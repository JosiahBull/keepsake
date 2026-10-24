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
   git tag -a v0.1.0 -m 'Keepsake 0.1.0'
   git push origin v0.1.0
   ```

Use the next version for later releases. Do not move an already published tag.

## Verify

Watch the Release workflow through completion. Check the GitHub release contains
CLI archives and SHA256 checksums, and verify the container manifest includes
`linux/amd64` and `linux/arm64`.

The image is `ghcr.io/josiahbull/keepsake:0.1.0`, also tagged `0.1` and `latest`.
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
