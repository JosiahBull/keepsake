import { execFileSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../..");

export default function setup() {
  execFileSync("cargo", ["build", "--locked", "-p", "keepsake-server", "-p", "keepsake-cli", "--bins"], {
    cwd: repoRoot,
    stdio: "inherit",
    env: { ...process.env, SQLX_OFFLINE: "true" },
  });
  execFileSync("pnpm", ["--filter", "@keepsake/web", "build"], {
    cwd: repoRoot,
    stdio: "inherit",
  });

  const metadata = JSON.parse(
    execFileSync("cargo", ["metadata", "--no-deps", "--format-version", "1"], {
      cwd: repoRoot,
      encoding: "utf8",
    }),
  ) as { target_directory: string };
  process.env.KEEPSAKE_E2E_BIN_DIR = path.join(metadata.target_directory, "debug");
}
