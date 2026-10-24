#!/usr/bin/env python3
"""Publish a SHA-bound draft without replacing completed release artifacts."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess


TARGETS = (
    "aarch64-apple-darwin",
    "aarch64-unknown-linux-musl",
    "x86_64-apple-darwin",
    "x86_64-pc-windows-msvc",
    "x86_64-unknown-linux-musl",
)
SEMVER = re.compile(r"v(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?\Z")


def run(arguments, **kwargs):
    result = subprocess.run(arguments, text=True, capture_output=True, **kwargs)
    if result.returncode:
        raise RuntimeError(f"{arguments[0]} failed: {result.stderr.strip()}")
    return result.stdout


def api(repo, path, method="GET", payload=None, paginate=False):
    arguments = ["gh", "api", f"repos/{repo}/{path}", "--method", method]
    if paginate:
        arguments.extend(["--paginate", "--slurp"])
    if payload is not None:
        arguments.extend(["--input", "-"])
    result = subprocess.run(arguments, input=json.dumps(payload) if payload is not None else None,
                            text=True, capture_output=True)
    if result.returncode:
        raise RuntimeError(f"GitHub {method} {path} failed: {result.stderr.strip()}")
    return json.loads(result.stdout) if result.stdout.strip() else None


def output(name, value):
    with open(os.environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as destination:
        destination.write(f"{name}={value}\n")


def get_release(options):
    # The tag endpoint promises published releases; listing includes authorised drafts.
    pages = api(options.repo, "releases?per_page=100", paginate=True)
    release = next((item for page in pages for item in page if item["tag_name"] == options.tag), None)
    if release and release["draft"] and release["target_commitish"] != options.sha:
        raise RuntimeError("The existing draft belongs to another commit; do not move release tags.")
    return release


def prepare(options):
    release = get_release(options)
    if release and not release["draft"]:
        print(f"{options.tag} is already published; leaving its artifacts unchanged.")
        output("pending", "false")
        return
    if release is None:
        api(options.repo, "releases", "POST", {
            "tag_name": options.tag,
            "target_commitish": options.sha,
            "name": options.tag,
            "draft": True,
            "prerelease": bool(SEMVER.fullmatch(options.tag)[4]),
            "make_latest": "false",
        })
    output("pending", "true")


def inspect_image(reference, field):
    result = subprocess.run([
        "docker", "buildx", "imagetools", "inspect", reference,
        "--format", "{{json ." + field + "}}",
    ], text=True, capture_output=True)
    if result.returncode:
        error = result.stderr.lower()
        if ("not found" in error or "manifest unknown" in error) and not any(
            text in error for text in ("unauthorized", "denied", "authentication")
        ):
            return None
        raise RuntimeError(f"Could not inspect {reference}: {result.stderr.strip()}")
    return json.loads(result.stdout)


def image_revisions(configs):
    # Buildx .Image is an OCI config for one platform, or a platform -> config map.
    if not isinstance(configs, dict):
        return {}
    images = [configs] if "architecture" in configs else configs.values()
    return {(image.get("os"), image.get("architecture")):
            image.get("config", {}).get("Labels", {}).get("org.opencontainers.image.revision")
            for image in images}


def manifest(options):
    release = get_release(options)
    if release is None or not release["draft"]:
        raise RuntimeError("Container publication requires the matching draft release.")
    reference = f"{options.image}:{options.tag[1:]}"
    existing = inspect_image(reference, "Manifest")
    if existing is not None:
        revisions = image_revisions(inspect_image(reference, "Image"))
        if any(revisions.get(("linux", architecture)) != options.sha for architecture in ("amd64", "arm64")):
            raise RuntimeError("Existing version image does not identify this commit on both platforms.")
        print(f"Reusing {reference}; version images are never overwritten.")
    else:
        digests = sorted(path.name for path in Path(options.digests).iterdir())
        if len(digests) != 2 or any(not re.fullmatch(r"[0-9a-f]{64}", digest) for digest in digests):
            raise RuntimeError("Expected exactly two verified platform image digests.")
        run(["docker", "buildx", "imagetools", "create", "--tag", reference,
             *[f"{options.image}@sha256:{digest}" for digest in digests]])
        existing = inspect_image(reference, "Manifest")
    platforms = {(item.get("platform", {}).get("os"), item.get("platform", {}).get("architecture"))
                 for item in existing.get("manifests", [])}
    if not {("linux", "amd64"), ("linux", "arm64")} <= platforms:
        raise RuntimeError("Version image must contain both amd64 and arm64.")
    if not re.fullmatch(r"sha256:[0-9a-f]{64}", existing["digest"]):
        raise RuntimeError("Registry returned an invalid image digest.")
    output("digest", existing["digest"])


def stable_version(tag):
    match = SEMVER.fullmatch(tag)
    return tuple(map(int, match.group(1, 2, 3))) if match and not match[4] else None


def rolling_tags(tag, releases):
    version = stable_version(tag)
    if version is None:
        return []
    versions = [stable_version(item["tag_name"]) for item in releases
                if not item["draft"] and not item["prerelease"]]
    versions = [item for item in versions if item is not None] + [version]
    tags = []
    if version == max(item for item in versions if item[:2] == version[:2]):
        tags.append(f"{version[0]}.{version[1]}")
    if version == max(versions):
        tags.append("latest")
    return tags


def checksum(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def publish(options):
    release = get_release(options)
    if release is None or release["target_commitish"] != options.sha:
        raise RuntimeError("Expected a release bound to this commit.")
    releases = [item for page in api(options.repo, "releases?per_page=100", paginate=True) for item in page]
    tags = rolling_tags(options.tag, releases)
    immutable = f"{options.image}@{options.digest}"
    if release["draft"]:
        directory = Path(options.assets)
        names = [f"keepsake-{options.tag}-{target}.{'zip' if 'windows' in target else 'tar.gz'}" for target in TARGETS]
        existing = {item["name"]: item for item in release["assets"]}
        for name, asset in list(existing.items()):
            if name in [*names, "SHA256SUMS"] and asset["state"] != "uploaded":
                api(options.repo, f"releases/assets/{asset['id']}", "DELETE")
                del existing[name]
        for name in names:
            path = directory / name
            if name in existing:
                # Keep previously uploaded bytes when resuming, even after a full rebuild.
                run(["gh", "release", "download", options.tag, "--repo", options.repo,
                     "--pattern", name, "--dir", str(directory), "--clobber"])
                digest = existing[name].get("digest")
                if digest and digest != f"sha256:{checksum(path)}":
                    raise RuntimeError(f"Downloaded draft asset failed its digest check: {name}")
            elif path.is_file():
                run(["gh", "release", "upload", options.tag, str(path), "--repo", options.repo])
            else:
                raise RuntimeError(f"Missing release archive: {name}")
        sums = "".join(f"{checksum(directory / name)}  {name}\n" for name in sorted(names))
        sums_path = directory / "SHA256SUMS"
        if "SHA256SUMS" in existing:
            run(["gh", "release", "download", options.tag, "--repo", options.repo,
                 "--pattern", "SHA256SUMS", "--dir", str(directory), "--clobber"])
            if sums_path.read_text() != sums:
                raise RuntimeError("Existing checksums differ; refusing to replace completed assets.")
        else:
            sums_path.write_text(sums)
            run(["gh", "release", "upload", options.tag, str(sums_path), "--repo", options.repo])
        body = (f"Container: {options.image}:{options.tag[1:]} (Linux amd64 and arm64).\n\n"
                f"Immutable container: {immutable}.\n")
        api(options.repo, f"releases/{release['id']}", "PATCH", {
            "draft": False, "body": body, "make_latest": "true" if "latest" in tags else "false",
        })
    elif immutable not in (release.get("body") or ""):
        raise RuntimeError("Published release identifies another image; refusing to change it.")
    # A failed promotion can be retried without editing the now-published release.
    if tags:
        run(["docker", "buildx", "imagetools", "create",
             *[argument for tag in tags for argument in ("--tag", f"{options.image}:{tag}")], immutable])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("prepare", "manifest", "publish"))
    parser.add_argument("--repo", default=os.environ.get("GITHUB_REPOSITORY"), required="GITHUB_REPOSITORY" not in os.environ)
    parser.add_argument("--tag", default=os.environ.get("GITHUB_REF_NAME"), required="GITHUB_REF_NAME" not in os.environ)
    parser.add_argument("--sha", default=os.environ.get("GITHUB_SHA"), required="GITHUB_SHA" not in os.environ)
    parser.add_argument("--image")
    parser.add_argument("--digests")
    parser.add_argument("--digest")
    parser.add_argument("--assets")
    options = parser.parse_args()
    if not SEMVER.fullmatch(options.tag):
        parser.error("release tags must be semantic versions prefixed by v")
    if options.command != "prepare" and not options.image:
        parser.error("--image is required for manifest and publish")
    if options.command == "manifest" and not options.digests:
        parser.error("--digests is required for manifest")
    if options.command == "publish" and (not options.digest or not options.assets):
        parser.error("--digest and --assets are required for publish")
    try:
        {"prepare": prepare, "manifest": manifest, "publish": publish}[options.command](options)
    except (RuntimeError, OSError) as error:
        parser.exit(1, f"Release failed: {error}\n")


if __name__ == "__main__":
    main()
