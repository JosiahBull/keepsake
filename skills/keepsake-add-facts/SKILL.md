---
name: keepsake-add-facts
description: Add source-linked facts to Keepsake from text, reading notes, or a URL with a learning prompt. Use when asked to remember or learn information in Keepsake.
---

# Add facts to Keepsake

Save knowledge the user wants to remember through the local CLI. The skill's
`scripts/keepsake.mjs` launcher works from any directory, without a running server.
It locates the repository from `KEEPSAKE_REPO`, its own location when in the repo,
or the checkout path saved by the installer. Set `KEEPSAKE_REPO` if the checkout moves.

## Preserve the knowledge

Use the supplied material as the source of truth. Write an assessable prompt with
a clear answer; retain conditions, exceptions, the surrounding explanation in
`--context`, and a URL, document path, or citation in `--source` when provided.
Do not invent a source or silently resolve conflicting claims. If accuracy depends
on a missing distinction, resolve it from the supplied context or ask narrowly.
For a supported, unambiguous fact, proceed with saving when the user asks to add it.

Split independent claims into facts that can be scored, while preserving the
relationships needed to understand them. Keep the user's useful tags. A long
answer can remain a fact when the prompt specifies the essential points.

Read the repository's `docs/research/learning-and-retention-2026-10-24.md` when
recommending learning methods and `docs/learning-design.md` for the implementation
boundary. Attribute learning recommendations to the saved research. Varied methods
are product proposals; no particular mix is a proven optimum.

## Save and inspect

Resolve `scripts/keepsake.mjs` relative to this skill file, then run its absolute path.
For example, from the skill directory:

```sh
node ./scripts/keepsake.mjs add \
  --prompt 'What distinguishes spaced practice from massed practice?' \
  --answer 'Spaced practice distributes learning across separate occasions; massed practice groups it together.' \
  --context 'Meaningful gaps depend on the retention goal and the material.' \
  --source 'docs/research/learning-and-retention-2026-10-24.md' \
  --tag learning
```

The same launcher is installed in Claude and Codex. Use process
arguments or proper shell quoting for user text; do not interpolate raw source
material into executable shell syntax. Run `--help` for current flags.

- `add --prompt ... --answer ...` saves one fact; repeat `--tag` for multiple tags
- `add ... --dry-run` previews without saving; `add --file reviewed.json` saves
  that exact reviewed JSON without regenerating it
- `list --search '...' --limit 25` finds a page of saved knowledge; `show ID`
  includes its practice variants; `methods` describes practice types
- `--json list` returns `{items, next_cursor, total}`. Continue with
  `list --before CURSOR` and the same search until `next_cursor` is null;
  the first page is not the whole library. `--limit` accepts 1–100
- `--database sqlite:PATH` selects another database; the launcher defaults
  `DATABASE_URL` to the repository's `data/keepsake.db` when it is unset
- `--json` gives machine-readable output

The launcher builds the `keepsake-cli` package through Cargo if necessary. Node and Rust are
needed. A command failure is not a saved fact; report it and preserve the proposed
content. Confirm successful saves using the returned fact IDs and storage path.
For an uncertain result, inspect `list`/`show` before repeating the save.

## Add from a URL

Pass the source URL and the user's learning focus directly:

```sh
keepsake add --url 'https://example.com/article' \
  --prompt 'Remember the main definitions and their exceptions' --tag reading
```

Use the skill launcher to target the repository's library. The standalone binary
defaults to `data/keepsake.db` in its current directory unless `--database` or
`DATABASE_URL` selects another file.
`--prompt` supplies instructions; omit `--answer`. URL capture defaults to Claude Haiku;
`--agent codex` uses Luna. `--limit` caps extraction at 1–20 facts (default 5).

The command fetches UTF-8 HTML, plain text, or Markdown without rendering JavaScript or
signing in. It passes readable text to the selected provider. Preserve access errors;
do not substitute invented content. PDFs and oversized sources require another input.

`--dry-run` returns a JSON array. Check the proposed answers and supporting excerpts,
then save that exact array with `add --file reviewed.json`. The source URL and supplied
tags are retained. Excerpt matching checks provenance, not whether an answer follows
from the excerpt. A direct save is authorised when the user asks to add the material.

## Generate practice when useful

`add ... --agent claude` uses `claude -p` with Haiku; `--agent codex` uses Codex
with Luna. `generate ID --agent claude|codex` adds practice to an existing fact.
Use `--model` only when the user requests another supported model. These commands
send the fact, context, and source field (or fetched page text for URL capture) to the selected installed agent.

Use `--dry-run` to capture generated JSON in a temporary file and inspect it.
For a new fact, save the reviewed JSON with `add --file /absolute/path/reviewed.json`.
For an existing fact, use `generate ID --file /absolute/path/reviewed.json` so the
original fact is checked and only the reviewed practice is added. No second agent
call is needed. Keep the JSON fields intact when importing. Relative file and
database paths resolve from the calling directory.

Generation without `--dry-run` saves variants immediately. Treat saved generated
practice as unverified drafts until checked against the source, and identify this
status in the report. The database does not track a separate approval state.
Validation checks structure, rather than proving factual correctness. Inspect the
saved result with `show ID`.

Check drafts against the source: one unambiguous correct option, plausible but
incorrect alternatives, no duplicate options or answer clues, and an explanation
of the distinction. Never turn a generated distractor into a canonical fact.
Use free recall when the learner needs unaided production; use choice or matching
to practise discrimination; add explanation or application only where the source
supports the expected response. Unsupported extensions stay explicit drafts.

Summarise the facts saved, their IDs, and any draft material needing source review.
Do not add an approval step to an ordinary authorised fact save.
