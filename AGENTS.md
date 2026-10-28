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

## Rust

Match exhaustively, so a new variant or field is a compile error to resolve rather than
a case silently mishandled:

- Every `match` on an enum this workspace defines names every variant. Do not use a
  `_` arm, `matches!` or `==` against a single variant to classify one: give the enum a
  method with an exhaustive `match`, and call that.
- When converting one struct into another, destructure the source and name every field,
  binding the ones the target does not need to `_`. Do not use `..` to skip fields.
- Enums from other crates follow the same rule, except those marked
  `#[non_exhaustive]`, which require a wildcard. Wildcards are fine for strings, numbers
  and bytes.

Iterate over an enum's variants with strum's `EnumIter` (`Kind::iter()`), never a
hand-written list, so a new variant joins every loop and test that covers them all.

Model a closed set of values as an enum, not a string. Parse text into the enum where it
enters (an HTTP header, a SQL column, a file name) and turn it back into text only where
it leaves. Likewise hold lengths of time as `std::time::Duration`, converting to
milliseconds or seconds only at the API or database.

