# Browser tests

The Playwright suite builds the SPA and both Rust executables once, then starts a
new server and temporary SQLite database for every test. Servers bind ephemeral
loopback ports and serve the production assets directly. Tests never reuse a
development server or read the checkout's database or `.env`.

```sh
pnpm --filter @keepsake/web exec playwright install chromium webkit
pnpm test:e2e
pnpm test:e2e --project=desktop-chromium
pnpm --filter @keepsake/web test:e2e:ui
```

`pnpm test` runs Rust and browser tests. `pnpm test:rust` runs Rust tests alone.
`pnpm check` also type-checks the browser harness and specifications.

The suite runs desktop Chromium, mobile Chromium and mobile WebKit. It covers
the web app manifest, Home Screen shortcuts, shared pages, reminder settings,
fact forms, question images and audio, agent access settings, CLI/browser persistence across restarts, literal and Māori macron
search, pagination and keyboard focus, every practice format and rating, mixed
sessions, matching, loading states, retries, background saves and their order, preloaded
rounds, stale search responses and interface sounds.
Success paths use the real API. Recovery tests intercept only the deliberate
failure or delayed response. Reminder settings tests replace only the browser's push
subscription, which needs a vendor push service; every request still reaches the real
API. The service worker test runs in Chromium's new headless mode, because the default
headless shell denies notifications. It delivers a push through the DevTools protocol,
then reloads offline to check the cached shell. WebKit cannot be muted, so test pages have
no `AudioContext`; they record which sounds the app asks for instead. Rust tests deliver
encrypted reminders to a local push service and decrypt them. Unexpected browser exceptions
and requests outside the test server fail the test. Most cases reset the migrated database
to an empty library and add synthetic facts. Household-default cases retain the bundled facts
and offers to check first startup, acceptance, and separate progress. Tests never
call an AI agent.

## Visual comparisons

Four views have screenshot baselines for each browser project: the empty library,
fact form, expanded fact and practice feedback. Baselines include the platform in
their names because macOS and Linux fonts differ. CI uses the pinned Playwright
image in `.github/workflows/checks.yml`; its version must match the exact
`@playwright/test` dependency.

After an intentional appearance change, update and inspect local baselines:

```sh
pnpm test:e2e visual --update-snapshots
pnpm test:e2e visual
```

For Linux updates, CI uploads expected, actual and diff images on failure. Download
the `browser-test-results` artifact, inspect the actual images, replace the matching
`*-linux.png` baselines and rerun CI. Missing baselines fail CI. Commit reviewed
baselines with the change; do not weaken the comparison threshold to accept a change.

## Diagnose a failure

Failures retain an HTML report, screenshot, video, trace and bounded server log.
CI uploads them for seven days. Open local reports with:

```sh
pnpm --filter @keepsake/web exec playwright show-report
pnpm --filter @keepsake/web exec playwright show-trace test-results/TEST/trace.zip
```

Retries are disabled so intermittent failures stay visible. The harness stops only
the child processes it owns and removes its temporary databases during teardown.
