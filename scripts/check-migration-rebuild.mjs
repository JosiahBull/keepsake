import { cpSync, existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const temporary = mkdtempSync(join(tmpdir(), "keepsake-migration-rebuild-"));

try {
  for (const name of ["Cargo.toml", "Cargo.lock", ".cargo", ".sqlx", "packages/core", "packages/server", "packages/mcp", "packages/cli"]) {
    if (existsSync(join(root, name))) cpSync(join(root, name), join(temporary, name), { recursive: true });
  }
  const examples = join(temporary, "packages/core/examples");
  mkdirSync(examples, { recursive: true });
  writeFileSync(join(examples, "migration_probe.rs"), `
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let path = std::env::args().nth(1).expect("temporary database path");
    let pool = keepsake_core::db::connect(&format!("sqlite:{path}")).await?;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM sqlite_master WHERE name = 'migration_rebuild_probe'").fetch_one(&pool).await?;
    println!("{count}");
    pool.close().await;
    Ok(())
}
`);
  const migration = join(temporary, "packages/core/migrations/999999_rebuild_probe.sql");
  for (const [index, expected] of [0, 1, 0].entries()) {
    if (index === 1) writeFileSync(migration, "CREATE TABLE migration_rebuild_probe (id INTEGER PRIMARY KEY);\n");
    if (index === 2) rmSync(migration);
    const result = spawnSync("cargo", ["run", "--locked", "--offline", "--quiet", "--package", "keepsake-core", "--example", "migration_probe", "--", join(temporary, `probe-${index}.db`)], {
      cwd: temporary,
      encoding: "utf8",
      env: { ...process.env, SQLX_OFFLINE: "true", DATABASE_URL: "", CARGO_TARGET_DIR: process.env.CARGO_TARGET_DIR || join(root, "target/migration-rebuild-check") },
      timeout: 180_000,
    });
    if (result.error || result.status !== 0 || result.stdout.trim() !== String(expected)) {
      throw new Error(`Migration ${index === 0 ? "baseline" : index === 1 ? "addition" : "removal"} was not rebuilt: ${result.error?.message || result.stderr || result.stdout}`);
    }
  }
  console.log("Migration additions and removals rebuild the embedded migrator.");
} finally {
  rmSync(temporary, { recursive: true, force: true });
}
