# Keepsake contributor guidance

Keep the frontend statically buildable and serve it through the Rust backend in production.
Keep SQL in the database module or dedicated repositories as the application grows.
Use append-only migrations. Tests must use temporary databases, never `data/keepsake.db`.
Regenerate the TypeScript client after API changes with `pnpm gen:client`.
Run `pnpm check` and `pnpm test` before delivering changes.
Browser tests build and serve the production SPA through the Rust server, with a
temporary SQLite database per test. Keep happy paths on the real API; intercept
requests only for deliberate failures and response ordering. Review visual baseline
changes; never update screenshots just to make a failing comparison pass.
Keepsake is a personal flashcard library for storing and recalling detailed information.
When recommending solutions from a learning perspective, read and reference
`docs/research/learning-and-retention-2026-10-24.md`. This is user-supplied research,
dated 24 October 2026. Distinguish its empirical findings from its design proposals
and preserve its evidence limitations. Verify time-sensitive software and benchmark
claims before presenting them as current. Explain how learning recommendations follow
from the research; identify proposals that go beyond it.
Do not start Docker or OrbStack. Develop and verify directly on the host.
