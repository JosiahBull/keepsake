# Agent access (MCP)

Keepsake speaks the [Model Context Protocol](https://modelcontextprotocol.io), so an agent
such as Claude Code can search a profile's library and, with a second, higher setting, add
facts to it.

> **Turning this on sends that library to a model provider.** Everything a tool returns,
> including questions, answers, context, sources and question images and audio, goes to
> whichever model the connecting client runs. That is why it is off by default, takes two
> switches to turn on, and needs a third step before an agent can write.

## Turning it on

Two switches, and both have to allow it:

1. **`KEEPSAKE_MCP`**, an environment variable, is a ceiling: the most this server will
   ever serve. Unset, the default, means `off`, and `/mcp` is not a route at all.
2. **Profile settings → Agent access** is each profile's own choice: Off, Read only, or
   Read and add facts. What is served is that choice, limited to the ceiling.

```sh
KEEPSAKE_MCP=read keepsake-server    # profiles may choose off or read
KEEPSAKE_MCP=write keepsake-server   # profiles may choose off, read or write
keepsake-server                      # off; the settings control explains why
```

| Variable | Default | Meaning |
| --- | --- | --- |
| `KEEPSAKE_MCP` | `off` | `off`, `read` or `write`: the ceiling. Off means `/mcp` is not mounted. |
| `KEEPSAKE_MCP_ALLOWED_HOSTS` | none | Comma-separated host names accepted besides loopback |

The ceiling exists because Keepsake has no authentication: a setting alone would let
anyone who can reach the app grant agent access. Requiring the environment first leaves
"may this server do this at all" to whoever runs it. An app setting above the ceiling is
refused with a 422 naming the limit, not silently lowered, because a settings page showing
more access than is served would describe a server that does not exist. An unrecognised
`KEEPSAKE_MCP` value stops startup: a typo that served nothing would be confusing, and one
that fell back the other way would grant access nobody chose.

Each profile chooses separately because each library belongs to one person. Profiles have
no passwords, so this is a household agreement rather than a security boundary.

Changes apply to the next request. The served mode is read from the database on every
call, with nothing cached. Off means nothing is served, not read-only: an agent connected
when access is switched off finds every tool gone. The server's instructions tell agents
the tool list can change and to list the tools again.

## Connecting an agent

Each profile has its own endpoint, `/mcp/HANDLE`. Profile settings shows it once access is
on, with the command for Claude Code:

```sh
claude mcp add --transport http keepsake-ansam http://127.0.0.1:8080/mcp/ansam
```

An agent reaches only that profile's library; there is no profile argument to get wrong.
Handles never change, so the address stays valid.

### Reaching it from another machine

Requests are accepted only when their `Host` is loopback or listed in
`KEEPSAKE_MCP_ALLOWED_HOSTS`. A server running on your machine is otherwise a
DNS-rebinding target: a web page can point its own hostname at `127.0.0.1` and post to it
from your browser. Requests carrying a browser `Origin` are refused outright, so no web
page can drive the tools; MCP clients send none. Behind a reverse proxy on
`keepsake.example.com`, set `KEEPSAKE_MCP_ALLOWED_HOSTS=keepsake.example.com`.

There is still no authentication, here or on `/api`. MCP adds a convenient surface to the
same open server. Bind to loopback, or put access control in front.

## The tools

[`packages/mcp/tool-manifest.json`](../packages/mcp/tool-manifest.json) is the committed
contract: names, descriptions, arguments, annotations and tier. A test compares it with
the running code, so it cannot drift silently.

**Read** (served at `read` and `write`): `search_facts`, `get_fact`, `view_media`,
`list_tags`, `study_overview`.

**Write** (added at `write`): `add_facts` saves up to 20 facts together or not at all,
with practice variants and up to four images or clips each, sent as base64.
`add_practices` adds variants to an existing fact. Media goes through the same checks as
any upload: the declared type must match, and the whole file must decode within the limits
in the README. A request body may be up to 8 MiB, enough for one 5 MiB file.

Write tools are added to the tool list or left out entirely, never listed and then
checked, so a read-only connection cannot reach one by accident.

### Not exposed, at any level

- **Reviews and ratings.** The learner attempts each answer before seeing it and rates
  their own recall in the app or CLI.
- **Approving practice variants.** Variants an agent adds appear in extra practice; one
  counts towards scheduled recall only after the learner checks it against the source and
  runs `keepsake study approve`.
- **Sharing, profiles, settings, reminders, backups and restores.** These are household
  decisions.
- **Editing or removing anything.** Keepsake has no route for either.

The first two follow from the [saved research](research/learning-and-retention-2026-10-24.md).
A 2025 randomised field experiment found that unrestricted AI assistance improved assisted
practice but could harm later unaided performance, while a tutor designed to give hints
largely mitigated that harm. That study covered short-term mathematics outcomes in one
school, not flashcard retention. The research's proposals are to use AI as a questioner and
feedback assistant while the learner does the retrieval, and to check generated facts
against the source. Keeping reviews and approval with the learner is Keepsake's application
of those proposals; the research does not test an MCP integration.

## Conventions on the wire

- **Lists are pipe tables**, cheaper in tokens than JSON and read as tables. Cells are
  flattened to one line; questions and answers in a search are cut at 160 characters and
  marked with `…`. `get_fact` gives the full text.
- **Searches are paged**, 1 to 50 facts at a time, with the total and a `before_id` for the
  next page.
- **Media is returned as MCP image or audio content**, exactly as stored.
- **A refusal the agent can act on**, such as a missing fact or an invalid draft, returns
  as a tool error with its message. Anything else is logged and replaced with a fixed
  sentence, so no database path or SQL error reaches a transcript.

## How it fits

`packages/mcp` (`keepsake-mcp`) depends on `keepsake-core` and nothing else in the
workspace. `keepsake-server` mounts it at `/mcp/{profile}` inside the same middleware as
`/api`: request IDs, panic recovery, admission limits and the request deadline. The
transport is Streamable HTTP in stateless JSON mode: each call is one POST and one JSON
body, with no held-open stream for a graceful shutdown to wait on. MCP responses are
`no-store`.

## Testing

- `packages/mcp` unit tests: the tier split, that read access serves only read-only
  tools, that off serves nothing, error scrubbing, and the committed manifest. Regenerate
  it deliberately with `cargo test -p keepsake-mcp -- --ignored update_the_tool_manifest`
  and read the diff.
- `packages/server/tests/mcp.rs` drives the router with raw JSON-RPC. It covers no
  endpoint without a ceiling, per-profile modes changing without a restart, the 422 above
  the ceiling, host and origin refusal, saving facts with media, refusing a bad draft whole,
  and startup stopping on an unrecognised `KEEPSAKE_MCP`.
- `packages/web/tests/agent-access.spec.ts` uses the settings control against the real
  server and checks the endpoint's tool list after each change.
