# Releases

CI and releases call the same reusable checks workflow, following Sure's release
structure. A version tag publishes the website and backend as one container image,
plus standalone CLI archives. A release is complete when both are available.

## Prepare

1. Update the workspace version in `Cargo.toml` and every Node package version.
2. Run `pnpm install` and `cargo check --workspace` to update lockfiles.
3. Run `pnpm gen:client`, `pnpm check`, `pnpm test`, and the frontend build.
4. Commit and push to `main`; wait for CI to pass.
5. Create an annotated tag matching the version, then push it:

   ```sh
   git tag -a v0.5.0 -m 'Keepsake 0.5.0'
   git push origin v0.5.0
   ```

Use the next version for later releases. Do not move an already published tag.

Release runs share a publication lock. GitHub keeps one pending run; if another
tag supersedes it, rerun the canceled tag when publication is idle.
A completed release is skipped; its assets
and version image are never replaced. An incomplete release stays as a draft bound
to the tagged commit. Retry failed jobs to resume it. Existing draft uploads and
the first version image are retained, checksums are uploaded after all archives,
and the draft is published last. Rolling container tags advance only for the
greatest published stable version in their series.

If container tag promotion fails after the GitHub release publishes, retry failed
jobs to finish promotion. Rerunning a completed workflow leaves the release alone.
Protect `v*` tags against updates/deletions, and enable GitHub release immutability
in repository settings to enforce these rules outside this workflow too.

## Verify

Watch the Release workflow through completion. Check the GitHub release contains
CLI archives and SHA256 checksums, and verify the container manifest includes
`linux/amd64` and `linux/arm64`.

The image is `ghcr.io/josiahbull/keepsake:0.5.0`, also tagged `0.5` and `latest`.
On the first release, set the package's visibility to Public in GitHub's package
settings, then verify the image is accessible without credentials. Public repository
visibility alone does not make its container package public.

CLI downloads are unsigned. Verify the archive against `SHA256SUMS` from the same
release before installing. SHA256 detects corruption; it is not an independent
signature or proof of publisher identity.

The server has no authentication. Bind it to loopback or put an authenticated
reverse proxy in front of it. Keep `/data` on persistent local storage and back up
SQLite using the CLI's `backup` command before upgrading. See the README's backup and
restore commands; restore into a new file and verify it before switching the server.

Compose runs with a read-only root filesystem, dropped capabilities, and no new
privileges. `/data` remains writable; `/tmp` is a 64 MiB temporary filesystem.
Its 30-second stop grace exceeds the default 10-second application shutdown budget
and one-second runtime cleanup. If you increase `SHUTDOWN_GRACE_SECS`, increase the
Compose grace too. The image's bounded health probe reports availability; Docker's
restart policy acts on process exit, not an unhealthy result alone.
