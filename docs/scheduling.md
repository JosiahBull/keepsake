# Recall scheduling

Keepsake uses the pinned [Rust FSRS 6.6.2 implementation](https://docs.rs/fsrs/6.6.2/fsrs/)
through a pure adapter in shared Rust code. The server and independent CLI use the
same SQLite state, validation and transactions.

The [saved learning research](research/learning-and-retention-2026-10-24.md) supports
retrieval, corrective feedback and spacing more strongly than any particular scheduler
or mixture of formats. This implementation is a product proposal informed by that
research, not evidence that Keepsake improves long-term retention.

## What is scheduled

Each fact has one forward recall target. Its canonical flashcard is eligible immediately.
Other generated prompts are supplementary until explicitly checked against the source.
An approved flashcard or cloze must retain the canonical answer and shares the original
target. Five variants therefore still create one review obligation.

Multiple choice, matching, explanation and application remain supplementary. Recognising
an option cannot advance the unaided-recall schedule. Separate directions and abilities
can be introduced deliberately later; they are not inferred from generated questions.

## Serving order and defaults

1. Learning or relearning targets whose delay has elapsed.
2. Due reviews, oldest urgency bucket first, randomised within an hour.
3. New targets within the local day's intake allowance, when no due work is waiting.

The server selects at most 20 targets. New intake is stable across reloads; additional
tabs cannot bypass the allowance or introduce new material ahead of ready reviews.
No transaction stays open while a person answers.

| Setting | Default |
| --- | --- |
| Desired retention | 90%, using the release's default model parameters |
| New intake | Five targets per local calendar day |
| Daily study budget | Soft 15-minute stopping suggestion |
| Again | Return after ten minutes |
| Successful recall | FSRS interval, rounded to whole days with a one-day minimum |
| Timezone | Europe/London; configurable with `study settings` |

The intake, budget and ten-minute delay are defaults to evaluate, not scientific optima.
The budget never hides due work or changes due dates. It counts recorded response time
once per attempt, including ungraded feedback; it does not measure time spent preparing
or repairing facts. Long intervals do not retire a target.

All stored times are UTC Unix seconds. The configured IANA timezone defines daily
intake and summaries, including daylight-saving transitions. FSRS 6.6.2's public API
accepts completed 24-hour periods, including zero for same-day reviews. Events retain
the exact elapsed seconds and unrounded model interval separately from policy overrides.

## Honest feedback and grading

Typing an answer or confirming a spoken attempt records that an attempt preceded feedback.
The answer appears only after its exposure event is saved. An unattempted reveal has
no recall rating. Rating an attempt uses these meanings:

| Rating | Meaning |
| --- | --- |
| Again | Could not recall, or recalled incorrectly |
| Hard | Correct unaided recall with substantial difficulty |
| Good | Correct unaided recall |
| Easy | Correct, immediate and confident recall |

Response speed is recorded, not used to invent a grade. Assistance and prior exposure
are recorded separately and disqualify an attempt as unaided recall evidence.

Library browsing, matching and supplementary answers record exposure before showing
answers. They leave memory estimates and model due dates unchanged, while blocking
assessment of the exposed target for ten minutes. Browsing does not introduce new
review obligations. A reveal in the scheduled queue consumes that day's intake, even
when the learner did not attempt it.

A rating can refer to its own just-saved feedback event, using the same prompt, response
time and current state version. This permits grading after corrective feedback without
turning another exposure into unaided evidence. Another tab or exposure invalidates the
attempt. Feedback-linked ratings expire after 15 minutes; that expiry is a product policy.
The model uses the time of the attempted feedback, rather than the later grading time.

## Persistence and retries

The append-only migration creates targets, practice mappings, schedule state, one settings
record and study events. It preserves facts, prompts and reviews. Existing reviews remain
legacy evidence, with their cutoff recorded in settings; they do not initialise memory
estimates or turn the existing collection into an overdue backlog.

Each submission has a UUID. Validation, review-history insertion, event recording and
schedule updates commit together. Retrying the same UUID and payload returns the original
result. Reusing a UUID for a different payload or submitting an old state version returns
a conflict. Batch exposures have the same retry guarantees. The older `/api/reviews`
endpoint remains supplementary and cannot update FSRS state.

The browser sends saves one at a time, in the order the learner made them, so the server
applies the same sequence as an interface that waited for every response. Exposures still
save before their answers or options appear; ratings save while the next question is shown.
Each save is kept in the browser's IndexedDB until the server confirms or refuses it, and a
later visit sends any left behind. A supplementary exposure that cannot reach the server
counts as saved once it is kept, so practice goes on offline; scheduled feedback waits for
the server, because its rating refers to the recorded event. A failed save holds back later
saves until it is retried with the same UUID. A
refusal, such as a conflict, cannot succeed on retry; the browser drops that save and says
so. A due queue is read only after earlier saves finish. Extra practice and matching
samples ignore schedules, so the browser loads the next round before the current one ends.

Events retain the target revision, assessment conditions, algorithm and parameter version,
exact elapsed time, model interval, policy override and before/after state. They also retain
the canonical request and recorded result. Export uses bounded cursors:

```sh
keepsake --profile default study events --after 0 --limit 100
```

Continue with `next_cursor` until it is null. A single event's `request_json` can be
submitted with `study submit --file`; its original UUID returns its recorded result.
This is idempotent replay, not a command to reapply historical transitions.

## Editing a fact

Changing a fact's question or answer retires its recall target and starts a new revision
with a fresh schedule; the canonical flashcard follows the new wording, and other prompts
stop qualifying until approved again. Context, source and tags change without touching
the schedule. The [saved research](research/learning-and-retention-2026-10-24.md) treats
memory-model estimates as predictions about a particular item, so performance on the old
wording is kept as history rather than carried over. Whether a small correction, such as a
typo, deserves to keep its schedule is a product question the research does not settle.

## Offline recall

A device with offline use turned on downloads prepared study with its copy of the library:
work available within the next seven days, each with when it becomes available and its
rating intervals at that time, and the next week of new targets in intake order (at most
1,000 of each). Offline, it serves due work once available by its own clock, then new
targets within the local day's allowance, counted in the profile's study timezone. Each
target comes at most once until the next download; Again does not bring it back offline.

Offline events carry `occurred_at`, the device's time, and a rating names its feedback by
`feedback_submission_id`. The server applies its usual rules at that time: elapsed time
for the model, the 15-minute rating window, due times and the day's new-item allowance
all use when the attempt happened, not when it arrived. Times may be up to 30 days old; a
time later than the server's counts as now. The rule that keeps new targets behind
waiting reviews is not applied to offline events, because reviews may fall due on the
server while the device has none left to serve.

An offline event that no longer fits its target, because another device reviewed it or
its content changed, is kept with the reason in `ignored`. It leaves the schedule as it
was, adds no review, and is never recall evidence. The app says how many offline attempts
did not count. Events for a deleted fact cannot be kept and are refused.

The [saved research](research/learning-and-retention-2026-10-24.md) supports spacing and
measures it in elapsed time between occasions; using the time of the attempt keeps that
measurement honest after a long offline stretch. Offline feedback still follows a
recorded attempt, as retrieval before feedback requires. Offering each target once and
holding new items within the daily allowance keep offline sessions within the same
spacing and budget as online ones. The seven-day window and the 30-day limit are product
choices; the research does not study offline practice.

## Reminders

A profile can receive one Web Push reminder per local day on each browser where it
turned reminders on. The server checks each minute. It sends a reminder once the
profile's chosen time has passed, practice is ready and the daily budget is unspent.
Ready practice means due reviews or new targets within that day's intake allowance.
The window closes at local midnight in the study timezone. The day is claimed before
delivery, so a crash or push-service failure can skip a reminder but never repeat one.
A push service's 404 or 410 removes that subscription from every profile.

The [saved research](research/learning-and-retention-2026-10-24.md) does not study
notifications. The policy follows from it only indirectly:

- Spaced retrieval across separate sessions has strong support, so a reminder marks
  each day's practice and never pursues repeated sessions within one day.
- The research measures useful knowledge within a sustainable study budget. Reminders
  therefore stop once the soft budget is spent and never hide or reschedule due work.
- Its warning that app "retention" can mean engagement rather than remembered knowledge
  is why reminders carry no streaks or escalation. Opened notifications are not
  evidence of learning.
- A small study supports sleep between learning and relearning without establishing
  an ideal time of day, so the learner chooses the time.

The 08:00 default, the midnight cutoff and reminding about new intake are product
choices to evaluate, not findings. Any effect of reminders on long-term recall should
be judged with the delayed measures the research recommends, kept separate from how
often reminders are opened.

## Deferred work

Retention and model parameters cannot yet be changed through settings. Optimisation,
held-out validation, workload previews and bulk rescheduling are deferred until reliable
history exists. Richer revision editing, suspension, undo, repair flags, separate ability
targets and delayed audits also remain future work. If canonical content changes outside
the application, scheduling rejects the mismatch instead of treating old performance as
evidence about the replacement.

## Profiles and shared facts

All queue selection, intake counts, study time, settings, exposure cooldowns, reviews,
and event exports belong to the selected profile. CLI study commands accept the global
`--profile ID_OR_HANDLE` flag or `KEEPSAKE_PROFILE`. Another profile’s fact, practice,
target, feedback ID, or submission UUID cannot replay or change your schedule.

Sharing offers a frozen snapshot. Accepting creates an owned copy with a new recall
target and no inherited reviews, memory state, or generated-prompt approvals. Each
copy then follows its owner’s settings independently. Existing schedules are preserved
during the profile migration; it does not recalculate FSRS state.
