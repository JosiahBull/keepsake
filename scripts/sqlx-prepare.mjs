import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const check = process.argv.includes("--check");
if (process.argv.slice(2).some(argument => argument !== "--check")) {
  throw new Error("Usage: node scripts/sqlx-prepare.mjs [--check]");
}
const lockedVersion = readFileSync(join(root, "Cargo.lock"), "utf8").match(/name = "sqlx"\nversion = "([^"]+)"/)?.[1];
const installed = spawnSync("sqlx", ["--version"], { cwd: root, encoding: "utf8" });
if (installed.status !== 0 || !installed.stdout.trim().endsWith(` ${lockedVersion}`)) {
  throw new Error(`Install the matching schema tool: cargo install sqlx-cli --version ${lockedVersion} --locked --no-default-features --features sqlite`);
}

// An explicit temporary URL and --no-dotenv keep developer databases out of this workflow.
const temporary = mkdtempSync(join(tmpdir(), "keepsake-sqlx-prepare-"));
const databaseUrl = `sqlite:${join(temporary, "schema.db")}`;
const env = { ...process.env, DATABASE_URL: databaseUrl, SQLX_OFFLINE: "false" };
delete env.SQLX_OFFLINE_DIR;

function run(command, arguments_) {
  const result = spawnSync(command, arguments_, { cwd: root, stdio: "inherit", env, timeout: 300_000 });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${command} failed with exit status ${result.status}`);
}

try {
  run("sqlx", ["database", "create", "--no-dotenv", "--database-url", databaseUrl]);
  run("sqlx", ["migrate", "run", "--no-dotenv", "--database-url", databaseUrl, "--source", "packages/core/migrations"]);
  run("cargo", ["sqlx", "prepare", "--workspace", "--no-dotenv", ...(check ? ["--check"] : []), "--", "--all-targets", "--locked"]);
  console.log(check ? "SQL query metadata matches the migration schema." : "SQL query metadata regenerated from the migration schema.");
} finally {
  rmSync(temporary, { recursive: true, force: true });
}
