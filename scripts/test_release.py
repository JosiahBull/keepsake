import hashlib
from pathlib import Path
import re
from types import SimpleNamespace
import tempfile
import unittest
from unittest.mock import patch

import release


class ReleaseTests(unittest.TestCase):
    def options(self, directory="unused"):
        return SimpleNamespace(repo="owner/keepsake", tag="v1.2.3", sha="a" * 40,
                               image="ghcr.io/owner/keepsake", digest="sha256:" + "b" * 64,
                               assets=directory, digests=directory)

    def draft(self):
        return {"id": 1, "tag_name": "v1.2.3", "target_commitish": "a" * 40,
                "draft": True, "prerelease": False, "assets": [], "body": ""}

    def test_completed_release_skips_without_mutation(self):
        completed = {**self.draft(), "draft": False}
        with patch.object(release, "api", return_value=[[completed]]) as api, patch.object(release, "output") as output:
            release.prepare(self.options())
        api.assert_called_once_with("owner/keepsake", "releases?per_page=100", paginate=True)
        output.assert_called_once_with("pending", "false")

    def test_manifest_stage_can_read_draft_releases(self):
        workflow = (Path(__file__).parents[1] / ".github/workflows/release.yml").read_text()
        job = workflow.split("\n  docker:\n", 1)[1].split("\n  github-release:", 1)[0]
        self.assertRegex(job, re.compile(r"    permissions:\n(?:      .*\n)*      contents: write\n"))

    def test_draft_cannot_resume_after_tag_moves(self):
        with patch.object(release, "api", return_value=[[{**self.draft(), "target_commitish": "another-sha"}]]):
            with self.assertRaisesRegex(RuntimeError, "another commit"):
                release.prepare(self.options())

    def test_existing_version_image_is_reused_without_registry_mutation(self):
        manifest = {"digest": "sha256:" + "b" * 64, "manifests": [
            {"platform": {"os": "linux", "architecture": architecture}} for architecture in ("amd64", "arm64")
        ]}
        configs = {architecture: {"os": "linux", "architecture": architecture,
                                  "config": {"Labels": {"org.opencontainers.image.revision": "a" * 40}}}
                   for architecture in ("amd64", "arm64")}
        with patch.object(release, "api", return_value=[[self.draft()]]), \
                patch.object(release, "inspect_image", side_effect=[manifest, configs]), \
                patch.object(release, "run") as run, patch.object(release, "output") as output:
            release.manifest(self.options())
        run.assert_not_called()
        output.assert_called_once_with("digest", manifest["digest"])

    def test_existing_image_from_another_commit_is_refused(self):
        with patch.object(release, "api", return_value=[[self.draft()]]), \
                patch.object(release, "inspect_image", side_effect=[{"digest": "old"}, {"os": "linux", "architecture": "amd64", "config": {"Labels": {"org.opencontainers.image.revision": "other"}}}]), \
                patch.object(release, "run") as run:
            with self.assertRaisesRegex(RuntimeError, "does not identify this commit"):
                release.manifest(self.options())
        run.assert_not_called()

    def test_rolling_tags_never_regress_and_prereleases_do_not_promote(self):
        releases = [{"tag_name": tag, "draft": False, "prerelease": False}
                    for tag in ("v2.0.0", "v1.2.4")]
        self.assertEqual(release.rolling_tags("v1.2.3", releases), [])
        self.assertEqual(release.rolling_tags("v1.2.5", releases), ["1.2"])
        self.assertEqual(release.rolling_tags("v2.1.0", releases), ["2.1", "latest"])
        self.assertEqual(release.rolling_tags("v3.0.0-beta.1", releases), [])

    def test_resuming_partial_upload_preserves_uploaded_bytes_and_retries_promotion(self):
        with tempfile.TemporaryDirectory() as directory:
            options = self.options(directory)
            draft = self.draft()
            remote = {}
            writes = []
            for target in release.TARGETS:
                name = f"keepsake-v1.2.3-{target}.{'zip' if 'windows' in target else 'tar.gz'}"
                (Path(directory) / name).write_bytes(b"new build bytes")
            original_name = next(Path(directory).iterdir()).name
            remote[original_name] = b"original completed archive"
            draft["assets"] = [{"id": 2, "name": original_name, "state": "uploaded",
                                "digest": "sha256:" + hashlib.sha256(remote[original_name]).hexdigest()}]

            def api(repo, path, method="GET", payload=None, **kwargs):
                if method == "PATCH":
                    writes.append(("publish", payload))
                    draft.update(payload)
                    return draft
                return [[draft]]

            def command(arguments):
                if arguments[:3] == ["gh", "release", "download"]:
                    name = arguments[arguments.index("--pattern") + 1]
                    (Path(directory) / name).write_bytes(remote[name])
                elif arguments[:3] == ["gh", "release", "upload"]:
                    path = Path(arguments[4])
                    self.assertNotIn(path.name, remote, "remote assets must not be replaced")
                    remote[path.name] = path.read_bytes()
                    writes.append(("upload", path.name))
                elif arguments[:3] == ["docker", "buildx", "imagetools"]:
                    writes.append(("promote", arguments))
                    if sum(item[0] == "promote" for item in writes) == 1:
                        raise RuntimeError("temporary registry outage")
                else:
                    self.fail(f"Unexpected command: {arguments}")
                return ""

            with patch.object(release, "api", side_effect=api), patch.object(release, "run", side_effect=command):
                with self.assertRaisesRegex(RuntimeError, "temporary registry outage"):
                    release.publish(options)
                saved = dict(remote)
                release.publish(options)
            self.assertEqual(remote, saved)
            self.assertEqual(remote[original_name], b"original completed archive")
            self.assertEqual(sum(item[0] == "publish" for item in writes), 1)
            self.assertEqual(sum(item[0] == "promote" for item in writes), 2)
            lines = remote["SHA256SUMS"].decode().splitlines()
            self.assertEqual(len(lines), 5)
            for line in lines:
                digest, name = line.split("  ")
                self.assertEqual(digest, hashlib.sha256(remote[name]).hexdigest())

    def test_service_failure_is_not_treated_as_absent_release(self):
        result = SimpleNamespace(returncode=1, stdout="", stderr="HTTP 503 unavailable")
        with patch.object(release.subprocess, "run", return_value=result):
            with self.assertRaisesRegex(RuntimeError, "503"):
                release.api("owner/keepsake", "releases?per_page=100", paginate=True)


if __name__ == "__main__":
    unittest.main()
