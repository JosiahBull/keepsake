# Learning methods in Keepsake

Keepsake stores a source-linked fact once and can practise it through several
methods. The answer and its context remain the reference point for every variant.
This makes varied practice possible without treating generated questions as new
evidence for the underlying claim.

The [saved learning research](research/learning-and-retention-2026-10-24.md),
supplied by the user and dated 24 October 2026, is the basis for these decisions.
Its strongest support concerns retrieval with feedback and spacing. Meaningful
cue variation, explanation, and application motivate additional methods, with
task-dependent evidence and shorter follow-up in some studies. The combination
below is a product design proposal, not a validated intervention.

## Choose a method for the ability being practised

| Method | What the learner does | Design constraint |
| --- | --- | --- |
| Flashcard | Attempts an answer before revealing it | Prompt a response that can be checked against the source |
| Multiple choice | Distinguishes the right answer from plausible alternatives | Exactly one option is correct under the stated conditions; explain the distinction |
| Cloze | Supplies a missing part of a statement | Leave enough context to identify one intended answer without leaking it |
| Explain | Reconstructs why or how a relationship holds | Supply essential answer points supported by the source |
| Apply | Uses knowledge in a short scenario | Check the scenario and expected reasoning against the source |
| Matching game | Connects related prompts and answers | Use similar questions from one topic tag, with distinct pairings; repeated exposure within a game does not establish delayed recall |

Choice and matching exercise recognition and discrimination. Their outcomes do
not by themselves show that a learner could produce the answer without options.
Keep the method visible when interpreting performance. Provide the correct answer
and context after an attempt, including after an error.

Matching chooses a narrow shared topic and favours similar question wording, so unrelated
subjects do not give away the pairings. This is a design choice informed by the saved
research's discussion of distinguishing related concepts. That evidence is task-dependent;
it does not establish that harder matching rounds improve long-term vocabulary recall.

## Extend interactivity around meaningful retrieval

Future methods can include putting a process in order, predicting the next step
of a worked example, comparing easily confused concepts, or reconstructing a
diagram from memory. Mnemonics can help with selected arbitrary associations and
ordered lists. Choose these methods when they fit the material and give feedback
against a reliable reference.

These are extensions to evaluate. The research does not establish that mini games,
points, speed, or random paraphrasing improve long-term retention. An enjoyable
activity can support engagement while leaving delayed recall unchanged. Measure
engagement, unaided recall, and application separately.

## Keep AI output inspectable

The CLI can ask Claude Haiku or Codex Luna to draft alternative questions and
plausible wrong answers. Preserve the canonical fact, source, and context. Treat
generated practice as unverified until checked against that source. Saved variants
are immediately available for practice; there is no separate approval state or
draft review queue. To inspect before saving, capture `--dry-run` JSON and import
that exact content with `add --file` for a new fact or `generate ID --file` for an
existing fact.

For multiple choice, inspect the alternatives for accidental correctness,
ambiguity, duplicated meaning, length clues, and claims the source does not
establish. A structural validator can reject malformed choices; it cannot prove
their meaning or factual accuracy. AI should help the learner attempt retrieval
and understand feedback rather than supplying the answer before the attempt.

## Assess retention across time

This implementation stores review history with the method, outcome, response
time, and timestamp. These records can support later evaluation, while the first
practice formats remain available on demand. Personalised spaced scheduling and
delayed assessment still need implementation and validation. A method catalogue
does not demonstrate retention benefits.

Keep valued knowledge eligible for future maintenance. Interpret a successful
round as evidence about that attempt. Assess learning with delayed unaided recall
and unfamiliar applications within a sustainable study budget. The saved research
does not establish a universal repetition count, ideal mix of methods, or terminal
interval that guarantees permanent recall.
