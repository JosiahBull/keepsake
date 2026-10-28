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

## Ask with pictures and sound

A question can show images or play audio clips: name a bird from its photo or its call,
or recognise a te reo Māori word by ear. The saved research does not study images or
audio in prompts, so this is a design proposal. It draws on two points the research does
make. A response should resemble the ability you want, such as producing a foreign
phrase. Varying retrieval cues helps access knowledge from different contexts, which the
research rates as promising experimental evidence, measured over delays of up to 24 hours.
It rates diagrams as conditional: useful when the representation shows spatial,
structural or causal relationships.

Media belongs to the question, never the answer. The learner still attempts an answer
before revealing it, and scheduled recall records the same evidence as for a text
question. An image description is read to screen-reader users, so it should say what is
shown without naming the answer. Matching treats the same words asked of different
pictures as different questions, so a bank of pictures can be matched by sight.

The research does not establish that a picture or a sound improves retention of a fact,
or that recall from a picture transfers to recall from words. If you need both, keep a
text-only fact as well.

## Identify birds and plants

Josiah and Ansam each start with two lists asked through pictures and sound: New Zealand
native birds, by sight and by call, and plants common in New Zealand. Their design applies
the saved research where it can, and goes beyond it where noted.

- **Name it before seeing the answer.** Every card asks for the bird or plant by name, and
  that free recall is the scheduled target. This follows the research's strongest finding,
  retrieval with corrective feedback, and its advice to answer in the form you need:
  naming a bird you see or hear, not picking it from a list.
- **Feedback that separates look-alikes.** Each card's context gives the cues that tell
  the species apart, especially from those it is most often mistaken for, so a wrong
  answer is corrected with the distinction rather than only the name.
- **Practise confusable species together.** Multiple choice offers species a learner
  could confuse, and the lists tag groups such as forest birds, parrots or coastal birds,
  so matching draws its round from one group. The research supports interleaving related
  categories when they are confusable; its evidence is task dependent and does not
  include identifying species.
- **More than one route to a species.** Common birds and plants appear in more than one
  photo, and many birds also by call. This applies the research's promising evidence for
  varied retrieval cues, which it measured with word meanings over up to 24 hours, not with
  pictures or sounds. Each card keeps its own schedule, because recognising a bird by its
  call does not show you would know it by sight.
- **Sex and age where they differ.** When males and females, or young and adults, look or
  sound different, a card asks for the species and its sex or age. Both forms appear, so
  the answer is not always the unusual one. This is a design proposal; the research does
  not study it.

A wrong identification teaches the wrong thing, so the lists prefer photographs and
recordings whose identification could be checked against New Zealand references, such as
NZ Birds Online and the NZ Plant Conservation Network. An image made by a generative model
is used only where no suitable photograph was found. It is labelled as AI-generated
wherever it is shown, and carries a visible watermark, because it can be wrong in ways a
photograph is not.

## Keep sounds neutral about ratings

Every rating makes the same sound. A rewarding sound for Good or Easy would reward the
button rather than the recall, and the scheduler depends on honest ratings. Only multiple choice and matching sound right or wrong, because only
they have an answer Keepsake can mark; the learner judges recall formats. There are no
timers, streaks or combos: response time feeds the study budget, and the research
favours spacing and a sustainable budget over longer unbroken sessions.

The saved research does not study sound. These rules are inferences from its advice to
grade honestly, its evidence for corrective feedback, and its separation of engagement
from remembered knowledge.

## Keep AI output inspectable

The CLI can ask Claude Haiku or Codex Luna to draft alternative questions and
plausible wrong answers. Preserve the canonical fact, source, and context. Treat
generated practice as unverified until checked against that source. Saved variants
are available for supplementary practice. Only the canonical prompt initially supplies
scheduled recall evidence. Approve an equivalent flashcard or cloze with
`keepsake study approve PRACTICE_ID` after checking it; approval adds no schedule.
Explanation, application and recognition remain supplementary to this recall target.
To inspect before saving, capture `--dry-run` JSON and import
that exact content with `add --file` for a new fact or `generate ID --file` for an
existing fact.

For multiple choice, inspect the alternatives for accidental correctness,
ambiguity, duplicated meaning, length clues, and claims the source does not
establish. A structural validator can reject malformed choices; it cannot prove
their meaning or factual accuracy. AI should help the learner attempt retrieval
and understand feedback rather than supplying the answer before the attempt.

Agents connected over [MCP](mcp.md) follow the same rules. They can add facts and
variants, which arrive unverified, but cannot approve a variant or record a review: the
learner still attempts each answer and rates their own recall. This applies the research's
proposal of AI as questioner and feedback assistant; the research does not test an agent
integration.

## Assess retention across time

The [scheduler](scheduling.md) uses a pinned Rust FSRS implementation and one recall
target per fact. Qualifying unaided attempts update memory estimates; recognition,
assistance and answer exposure do not. Feedback is recorded before showing the answer,
including when the learner leaves without rating it. Legacy reviews remain available
without being reinterpreted as verified recall evidence.

Desired retention starts at 90%. Five new targets per day, a soft 15-minute budget
and ten-minute retries are product defaults to evaluate. Neither these settings nor
the combination of formats has been established as optimal. Event export supports
inspection; delayed audits, parameter fitting and held-out evaluation remain future work.

Keep valued knowledge eligible for future maintenance. Interpret a successful
round as evidence about that attempt. Assess learning with delayed unaided recall
and unfamiliar applications within a sustainable study budget. The saved research
does not establish a universal repetition count, ideal mix of methods, or terminal
interval that guarantees permanent recall.
