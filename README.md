# Keepsake

A personal library for storing facts and practising recall, explanation, and application.
The TypeScript website builds to static files served by a Rust/Axum backend with SQLite.

## Install

Download the CLI for your platform from [Releases](https://github.com/JosiahBull/keepsake/releases/latest).
Verify its archive against the release's `SHA256SUMS`, extract it, and put `keepsake`
on your `PATH`. Linux, macOS, and Windows binaries are available.

The container image is `ghcr.io/josiahbull/keepsake:0.1.0` for Linux amd64 and arm64.
From a checkout, start it with persistent SQLite storage using:

```sh
docker compose pull
docker compose up -d
```

Open http://localhost:8080. The CLI can also be built independently from source:

```sh
cargo install --git https://github.com/JosiahBull/keepsake --tag v0.1.0 keepsake-cli --locked
```

## Capture a fact

From this repository:

```sh
pnpm keepsake add \
  --prompt 'What distinguishes spaced practice from massed practice?' \
  --answer 'Spaced practice distributes learning across separate occasions; massed practice groups it together.' \
  --context 'Choose meaningful gaps based on the material and retention goal.' \
  --source 'docs/research/learning-and-retention-2026-10-24.md' \
  --tag learning
pnpm keepsake list
pnpm keepsake show 1
```

Capture works without a running server. Every fact gets a flashcard, retains its source
and context, and appears in the website when the library loads. Reload the library after
adding facts from another process. `--json` prints machine-readable output.

Install the standalone command to use it from any directory:

```sh
cargo install --path packages/cli --bin keepsake --locked
keepsake add --prompt 'Your question' --answer 'Your answer'
```

The CLI is the `keepsake-cli` workspace package and builds independently with
`pnpm build:cli`. It shares fact types and SQLite storage with the server through
`keepsake-core`; neither executable depends on the other.

The standalone CLI defaults to `data/keepsake.db` in the current directory.
Use `--database sqlite:/absolute/path/keepsake.db` or `DATABASE_URL` to select a
SQLite file. The repository and skill launchers select this checkout's database
from any directory. Point the CLI and server at the same file to share a library.
There is no remote HTTP CLI mode.

## Back up and restore

Create a backup before upgrading. Choose an existing destination directory:

```sh
keepsake --database sqlite:/absolute/path/keepsake.db backup /backups/keepsake-2026-10-24.db
keepsake restore /backups/keepsake-2026-10-24.db --to /data/keepsake-restored.db
```

`backup` captures a consistent SQLite snapshot while the server is running, including
committed WAL data. Both commands check database integrity and foreign keys before
publishing the file. They preserve IDs, reviews and migration history, run no migrations,
and refuse existing destinations or SQLite sidecars. Snapshot files are private to the
current user on Unix. Keep copies on another volume with retention appropriate to your library.

Restore to a new path, inspect it with `keepsake --database sqlite:/data/keepsake-restored.db list`,
then stop the server before switching `DATABASE_URL` to that file and restarting.
Keep the original until you have verified the restored library. Do not copy only the live
database file: committed changes may still be in its WAL. Container users can run the CLI
inside the container with a writable backup directory mounted alongside `/data`.

## Add from a URL

```sh
keepsake add --url 'https://example.com/article' \
  --prompt 'Remember the main ideas, definitions, and important exceptions' \
  --tag reading
```

URL capture defaults to Claude Haiku. Add `--agent codex` for Luna. `--prompt` describes
what to remember; no `--answer` is needed. `--limit 5` sets the maximum number of facts
(default 5, maximum 20).

To inspect before saving:

```sh
keepsake add --url 'https://example.com/article' \
  --prompt 'Focus on the key mechanisms' --agent codex --dry-run > facts.json
keepsake add --file facts.json
```

Keepsake fetches readable page text and sends it with your prompt to the selected agent.
Each fact keeps the original URL, a supporting source excerpt, and your tags. The excerpt
must occur in the fetched text; this check does not prove the generated answer is accurate.
All facts are validated and saved in one transaction. A failed import saves none.

Supports UTF-8 HTML, plain text, and Markdown over HTTP/HTTPS. It does not render JavaScript,
sign in, or read PDFs. Fetches have a 30-second deadline, five-redirect limit, 2 MiB download
cap, and 120,000-byte readable-text cap. Oversized pages are rejected rather than truncated.

## Draft varied practice with an agent

Both integrations use the installed CLI's existing login. Claude uses the `haiku` alias;
Codex uses `gpt-6-luna`. Manual capture calls a model only when `--agent` is supplied.

```sh
keepsake add --prompt 'Your question' --answer 'Your answer' \
  --context 'The source material and qualifications' --source 'A citation' \
  --agent claude --dry-run > draft.json
# Inspect and correct draft.json against your source, then save that exact content:
keepsake add --file draft.json

# Draft variants for an existing fact with Codex Luna:
keepsake generate 1 --agent codex --dry-run > practice.json
keepsake generate 1 --file practice.json
```

Without `--dry-run`, `add --agent ...` saves the fact and generated variants together,
and `generate ID --agent ...` appends variants immediately. These variants are unverified
practice, available in the website; there is no approval queue. The original fact and
answer are preserved. Review new prompts, expected answers, explanations, and plausible
wrong choices against the source before relying on them.

`--model` overrides the selected agent's model. `--timeout-seconds` sets a 1–600 second
deadline (default 120); `--agent-command` selects an executable for custom installations.
On Windows, include the extension for npm shims, for example `--agent-command codex.cmd`.
Input is passed through stdin, with no shell interpolation. Agent stdout is capped; stderr is drained without retaining it.
Invalid output or generation failure does not partially save a new fact. A provider
call sends the supplied fact, context, and source to that provider.

`add --file -` and `generate ID --file -` accept JSON through stdin. `add` accepts one
`NewFact` or an array of up to 20; `generate` accepts one. The shape is in the OpenAPI schema. `keepsake methods` describes the supported modes.

## Practise

Practise shuffles all available flashcards, multiple choice, fill-the-gap, explanation,
and application prompts into one session. Each prompt appears once per session; starting
again reshuffles the queue. A matching game combines three or four facts with distinct questions
and answers. Additional methods can share the same source-linked knowledge units.

Attempt before revealing feedback, then record Again, Hard, Good, or Easy. Hard means
successful recall with difficulty; an incorrect answer means Again. Review history records
method, outcome, and response time separately, so matching and choice are not counted as
proof of unaided recall. A personalised spaced scheduler is not implemented yet.

[Learning design](docs/learning-design.md) explains the rationale and limitations, based on
the [saved research](docs/research/learning-and-retention-2026-10-24.md). The research supports
retrieval, feedback, and spacing; the particular mix of formats is a product proposal.

## Agent skill

The canonical [keepsake-add-facts skill](skills/keepsake-add-facts/SKILL.md) is stored here.
Install or update its copies in `~/.claude/skills` and `~/.codex/skills` with:

```sh
pnpm skills:install
```

The skill launcher works from any directory. Set `KEEPSAKE_REPO` if the checkout moves.

## Develop

Install Rust, Node 24 or newer, and pnpm 11. From the repository root:

```sh
pnpm install
pnpm gen:client
pnpm dev
```

Open http://localhost:5173. Vite proxies `/api` to the backend on port 8080.
The backend rebuilds on Rust and migration changes; the frontend reloads through Vite.

Builds check static SQL against committed `.sqlx` metadata, without opening a database.
After changing queries or migrations, regenerate metadata against a temporary schema:

```sh
cargo install sqlx-cli --version 0.9.0 --locked --no-default-features --features sqlite
pnpm sqlx:prepare
pnpm sqlx:check
```

The preparation script ignores `DATABASE_URL` and `.env` settings and deletes its temporary
database afterwards. Commit the updated `.sqlx` files with the query or migration change.

## Build and run

```sh
pnpm build
WEB_DIR=packages/web/dist ./target/release/keepsake-server
```

Open http://localhost:8080. The Rust process serves both the static website and the API.
Node is only needed to build the frontend.

| Variable | Default | Purpose |
| --- | --- | --- |
| `DATABASE_URL` | `sqlite:data/keepsake.db` for the server | SQLite file location |
| `BIND_ADDR` | `127.0.0.1:8080` | HTTP listening address |
| `WEB_DIR` | `packages/web/dist` | Built frontend directory |
| `RUST_LOG` | `info` | Server log filter |
| `SHUTDOWN_GRACE_SECS` | `10` | Shutdown budget for HTTP draining and database closure |
| `HEADER_READ_TIMEOUT_SECS` | `15` | Deadline for receiving an HTTP request head |
| `CONNECTION_TIMEOUT_SECS` | `120` | Maximum connection lifetime, including keep-alive and blocked writes |
| `REQUEST_TIMEOUT_SECS` | `30` | Request deadline after headers, including body receipt and handler work |
| `MAX_CONNECTIONS` | `128` | Concurrent TCP connections |
| `MAX_IN_FLIGHT` | `32` | Concurrent requests; excess requests receive 503 with `Retry-After` |
| `MAX_BODY_BYTES` | `524288` | Request-body size limit in bytes |

Run the server from the repository root with its defaults. `.env` files are not loaded
automatically. Embedded, append-only migrations run on startup. SQLite uses WAL, foreign
keys, and a four-connection pool. Keep the database on local persistent storage.

An explicitly configured `WEB_DIR` must contain a readable `index.html`; startup fails
before opening SQLite when it is missing. With `WEB_DIR` unset, the development API can
start before the frontend is built. [`.env.example`](.env.example) lists the defaults.
Duration settings accept 1–3600 seconds; connection and request counts accept 1–16384;
the body-size setting accepts 1–16777216 bytes. Invalid settings fail startup.

SIGINT or SIGTERM starts graceful shutdown. A second signal forces shutdown. The grace
budget includes closing SQLite; runtime cleanup can take up to one additional second.
Give the container or service manager a longer stop grace than this combined budget.

```sh
keepsake-server --health-check
```

The probe asks the running `/api/health` endpoint, with a two-second deadline and bounded
response headers. It bypasses startup configuration, never opens SQLite, and maps wildcard
bind addresses to loopback. The endpoint itself checks the running server's database.

API failures use JSON, with `X-Request-ID` for log correlation. Request summaries record
method, route template, status and duration without query strings, headers or fact content.
API responses are `no-store`; fingerprinted frontend assets are immutable and the shell
revalidates. Browser security headers apply to ordinary responses and rejections. These
controls do not add authentication or cross-origin access.

The Dockerfile packages both Rust binaries and the website into one image. `compose.yaml`
mounts `/data` for persistence. CI builds and smoke-tests the container before release.
Agent executables are not installed in the image. There is no authentication yet; the
provided host binding is loopback. Use SQLite's backup API or stop the process before
copying its data directory: WAL files can contain committed data outside the main file.

## Project layout and checks

- `packages/web`: Svelte 5 and TypeScript, built with Vite
- `packages/client`: generated API types and an `openapi-fetch` client
- `packages/server`: HTTP routes and static website serving
- `packages/cli`: standalone `keepsake-cli` package, source imports and agent integration
- `packages/core`: shared fact types, validation and SQLite repositories
- `packages/core/migrations`: append-only SQL migrations embedded in both binaries
- `skills/keepsake-add-facts`: reusable capture workflow

`pnpm gen:client` generates OpenAPI from Rust and then the TypeScript contract. Commit the
schema and generated types together. The API exposes facts, practice variants, reviews,
health, and OpenAPI. Unknown API routes return 404 rather than the SPA. SQL queries use
runtime validation; Sure's offline SQLx query metadata workflow has not been added.

```sh
pnpm check
pnpm test
```

Tests use temporary databases and fake agent executables to cover persistence, validation,
API boundaries, exact draft import, subprocess failures, deadlines, and output limits.
The supplied research is preserved with its original citations; saving it did not
independently verify its claims.

See [Releases](docs/releasing.md) for the versioning and publication workflow.
