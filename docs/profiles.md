# Household profiles

Profiles separate libraries and learning histories in one SQLite database. They are
household names without passwords, so they prevent accidental mixing rather than
restrict access to other household members.

## Browser views

![Household profile chooser](screenshots/profile-chooser.png)

![Question-only incoming shares with explicit acceptance](screenshots/shared-inbox.png)

## Upgrade an existing library

Before upgrading, create a whole-household backup with `keepsake backup PATH`.
Startup migrations assign existing facts, practice, schedules, settings, reviews,
and evidence to the initial profile (`default`). IDs and existing progress remain
intact. Name this profile in the browser or use:

```sh
keepsake profiles rename default 'Josiah'
keepsake profiles create 'Partner' --handle partner
```

The new profile starts empty with default study settings. Handles and numeric IDs
stay stable when display names change. Profile deletion is not supported.

## Select a library

The browser carries the selected profile in its URL, so switching one tab does not
change another. It clears loaded facts, practice, matching, and answer state when
switching. Unsaved answers need confirmation before being discarded. Writes and
failed retries remain tied to their original profile.

The Rust CLI accepts `--profile ID_OR_HANDLE` globally. `KEEPSAKE_PROFILE` supplies
the default; an explicit flag takes precedence. Run `keepsake profiles list` to see
IDs and handles. A sole profile can be selected automatically. If several exist,
an unscoped content or study command fails with a profile-selection error. This
happens before URL fetching or external agent work. Binding the owner before such
work also keeps capture in the intended library if another profile is later created.

## Share facts

Select facts in your library and choose the recipient. The offer contains a frozen
copy of their question, answer, context, source, tags, and practice content. The
recipient’s inbox shows questions with answers hidden until accepted. They can
accept one or several offers, or decline. Senders can withdraw pending offers.

Acceptance creates an independent owned fact with a fresh schedule. It copies no
reviews, successful recall history, cooldowns, or generated-prompt approvals. A retry
of an accepted offer returns the same copy. Later edits and new offers cannot replace
an existing accepted copy or its progress. Decline and withdrawal leave libraries
unchanged. There is no live synchronization or automatic merging of duplicate facts.

CLI equivalents:

```sh
keepsake --profile default share 12 13 --to partner --submission-id UUID
keepsake --profile partner shares list
keepsake --profile partner shares accept-batch SHARE_ID OTHER_SHARE_ID
keepsake --profile partner shares decline SHARE_ID
keepsake --profile default shares list --sent
keepsake --profile default shares withdraw SHARE_ID
```

Keep the same UUID and fact IDs to retry an offer. Each accepted share is idempotent,
so repeating acceptance does not create another fact.

## Back up or downgrade

Backup and restore include the entire household: profiles, libraries, learning
history, settings, pending offers, and acceptance provenance. They never filter by
`--profile`. There is no per-profile backup/export-and-restore operation.

To downgrade, restore the pre-upgrade backup to a new database path and stop the
server before switching `DATABASE_URL`. Keep the upgraded database because restoring
an older backup omits subsequent changes. Do not open a migrated database with an
older binary.
