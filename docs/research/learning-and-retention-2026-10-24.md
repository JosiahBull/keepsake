**Research current to 24 October 2026**

**The best supported solution is personalised spaced retrieval, combined with practice that builds understanding and tests application.** Keep important knowledge eligible for future review, adapt the timing to your actual recall, and vary what you ask yourself to do with it.

Your suspicion is partly right: scheduling research has advanced well beyond fixed boxes and interval multipliers. The newest work includes richer forgetting models, neural predictors and policies that optimise study time. However, the evidence for spacing and retrieval is substantially stronger than the evidence that the newest scheduler produces better learning than every established alternative. [Carpenter, Pan and Butler, 2022](https://doi.org/10.1038/s44159-022-00089-1), [SRS benchmark](https://github.com/open-spaced-repetition/srs-benchmark).

The scope here is factual and conceptual knowledge retained over months and years, including vocabulary, technical knowledge and ideas from reading. Problem solving is included where it changes how practice should work. “Maximum retention” means the greatest amount of useful knowledge maintained within a sustainable study budget. The practical system proposed below is a synthesis of the evidence; that complete combination has not been validated as a single intervention.

# What back off then stop gets right and misses

Spacing reviews is sound. The questionable step is treating a successful answer, a fixed number of successes or a sufficiently long interval as proof that an item can be retired permanently.

There are three different policies hidden in “back off then stop”.

| Policy | Consequence for lasting retention |
| - | - |
| Increase intervals and eventually remove the item | Future forgetting becomes unobserved unless ordinary use or an audit catches it. |
| Increase intervals until a fixed maximum, then repeat at that interval | Maintenance continues, but the cap may cause unnecessary reviews of stable material. |
| Predict recall and review when it falls to a chosen target | Timing can change with the learner, item and review history. |

A model can distinguish **current accessibility** from **memory stability**. You may recall something easily today because you just saw it, while still having a fragile memory. Conversely, a well established memory can survive a long gap. FSRS represents item difficulty, stability and the probability of recall, and updates its estimates after reviews. These are useful predictive quantities, rather than direct measurements of a memory trace. [FSRS algorithm specification](https://github.com/open-spaced-repetition/awesome-fsrs/wiki/The-Algorithm).

**The evidence does not establish a general graduation rule that guarantees permanent recall.** A classic nine year vocabulary study found substantial benefits from widely spaced relearning and measured retention up to five years after training. It involved just four participants, so it supports the possibility of durable learning without establishing a universal stopping threshold. [Bahrick and colleagues, 1993](https://www.psychologicalscience.org/journals/psychological-science/j.1467-9280.1993.tb00571.x/).

My recommendation is to retain a maintenance path for knowledge you still value. An item can become very infrequent, receive practice through real use, or be sampled in occasional audits. Remove it deliberately when its value no longer justifies its cost.

# Techniques and the strength of the evidence

The confidence labels below are qualitative judgements about the stated use, rather than formal evidence ratings or claims about every application. Effect sizes such as *g* and *d* describe standardised differences; they are not percentage improvements and cannot be added together.

| Technique | Evidence assessment | Main use |
| - | - | - |
| Retrieval with corrective feedback | Strong overall | Remembering material rather than merely recognising it |
| Practice spread across time | Strong overall | Maintaining knowledge beyond the current session |
| Successive relearning | Strong for studied factual and conceptual tasks | Establishing reliable recall across separate sessions |
| Personalised scheduling | Supported, with algorithm specific uncertainty | Allocating a large review workload |
| Interleaving related categories | Supported under relevant conditions | Distinguishing similar concepts and choosing methods |
| Varied retrieval cues | Promising experimental evidence | Accessing knowledge from different contexts |
| Explanation and application practice | Supported, with task dependent results | Understanding relationships and using knowledge |
| Memory palaces and other mnemonics | Useful for selected material | Arbitrary associations and ordered information |
| Drawing and diagrams | Conditional | Representing spatial, structural and causal relationships |
| Pretesting | Useful adjunct | Directing attention during initial learning |
| Sleep between sessions | Supports learning and relearning | Allowing consolidation between practice occasions |

## Retrieve before revealing the answer

Attempt an answer before looking. A large classroom meta-analysis combined 222 studies and 48,478 students and found a medium average benefit from quizzing, *g* = 0.499. Its size depended on factors including the comparison activity, feedback and alignment between practice and assessment. This supports retrieval as a general method, without implying the same gain for every deck or subject. [Yang and colleagues, 2021](https://pubmed.ncbi.nlm.nih.gov/33683913/).

For personal study, say, type, sketch or execute an answer before revealing it. Use a response format that resembles the ability you want: producing a foreign phrase, explaining a mechanism or selecting and executing a procedure. Multiple choice can also support learning; choosing free response is especially useful when unaided production is the goal.

Check the answer against a clear standard. A familiar looking answer is weak evidence that you could have produced it. For complicated responses, specify the essential points that must be present.

## Distribute practice over meaningful gaps

The important contrast is spaced practice versus massing the same practice together. A meta-analysis of 29 studies found a substantial advantage for spaced retrieval, *g* = 0.74, but no overall reliable advantage for expanding over uniform intervals, *g* = 0.034. That does not show adaptive scheduling is useless; fixed schedule experiments answer a different question from allocating reviews across thousands of items. [Latimier, Peyre and Ramus, 2021](https://link.springer.com/article/10.1007/s10648-020-09572-8).

The right gap also depends on how long you need the knowledge. An experiment with more than 1,350 participants varied study gaps up to months and tested retention up to a year. The gap producing the best performance increased with the delay to the final test. [Cepeda and colleagues, 2008](https://digitalcommons.usf.edu/psy_facpub/1766/).

A universal sequence such as 1, 3, 7 and 30 days is therefore a workable heuristic, rather than a scientific optimum. Avoid interpreting “harder is better” as a reason to wait until most answers are forgotten.

## Relearn successfully across separate sessions

Successive relearning means practising to a recall criterion, returning after a delay, and reaching that criterion again. It combines successful retrieval with spacing.

Across three experiments involving 533 students, Rawson and Dunlosky varied initial recall criteria and subsequent relearning. They measured retention and relearning over one to four months. Their suggested balance was three correct initial recalls followed by three spaced relearning sessions; additional relearning reduced the importance of the initial criterion. [Rawson and Dunlosky, 2011](https://pubmed.ncbi.nlm.nih.gov/21707204/).

Treat that as evidence for establishing recall on multiple occasions, rather than a mandatory “three plus three” setting for every card. The authors later stressed that the useful number of sessions depends on the material, schedule and retention goal. [Rawson and Dunlosky, 2022](https://journals.sagepub.com/doi/abs/10.1177/09637214221100484).

A September 2026 grammar study found that multiple error free practice sessions predicted more durable performance at 56 days. Crucially, students were grouped by their achieved performance: this does not establish that forcing everyone through three perfect sessions causes the same outcome. It also supplies no lifetime retirement criterion. [Serfaty, 2026](https://onlinelibrary.wiley.com/doi/10.1002/acp.70290).

## Personalise which item receives practice

A semester long foreign language study compared review strategies with study time matched. Personalised review produced better retention after the semester than both massed review and a common spaced schedule. This is meaningful evidence for adapting practice to learners and items. It does not establish the superiority of today's FSRS or a particular commercial app. [Lindsey and colleagues, 2014](https://www.psychologicalscience.org/journals/psychological-science/0956797613504302/).

For a large knowledge collection, personalisation is valuable because material differs in difficulty and review history. A scheduler should update from actual success and failure, including the elapsed time when a review happens late.

## Interleave concepts that you need to distinguish

Interleaving mixes related categories or problem types so that you must decide which rule or concept applies. This is different from randomly switching between unrelated topics.

A classroom trial randomly assigned 54 mathematics classes, involving 787 students, to different practice orders while keeping the problems the same. It supported interleaved practice on a delayed test. [Rohrer and colleagues, 2020](https://ies.ed.gov/use-work/awards/efficacy-study-interleaved-mathematics-practice).

The broader literature shows that benefits depend on the material and its similarity. Interleaving is particularly relevant when categories are confusable; results for verbal materials are mixed. [Firth, Rivers and Boyle, 2021](https://doi.org/10.1002/rev3.3266).

For technical learning, mix examples requiring competing methods and ask why one applies. For vocabulary, explicitly distinguish words you repeatedly confuse. Establish enough understanding to attempt the discrimination, then test it without announcing the category.

## Vary the routes by which you retrieve an idea

This is one of the most relevant recent findings for improving a flashcard experience.

In six experiments, participants learned foreign word meanings through contextual sentences. Changing those sentences between retrieval attempts improved learning compared with repeating the same sentence; benefits appeared immediately and after 24 hours and were larger with spaced practice. Learners nevertheless judged the constant cues more favourably. [Butowska-Buczyńska and colleagues, 2024](https://pmc.ncbi.nlm.nih.gov/articles/PMC11536137/).

This supports varying meaningful retrieval contexts. It does not establish that automatically paraphrasing every card will improve retention over years. The study also distinguishes meaningful cue variation from simply rewording a question.

A practical extension is to practise a concept through a definition, an example, a contrast and a short application. Keep the underlying answer consistent. If one cue makes the task substantially easier, record that distinction rather than treating all successes as interchangeable.

## Explain relationships and practise application

Understanding and retrieval both deserve study time. A 2025 meta-analysis of 44 studies found only a small overall advantage for retrieval over elaborative learning activities, *g* = 0.14. Feedback materially affected the comparison. This is a useful correction to treating every non-flashcard activity as wasted time. [Gonçalves, Muniz and Jaeger, 2025](https://link.springer.com/article/10.1007/s10648-025-10076-6).

Use questions such as “Why is this true?”, “How does this connect to something I know?” and “What would change this conclusion?” Check the explanation against a reliable source. A separate meta-analysis found benefits from prompting self-explanation across a range of learning tasks. [Bisra and colleagues, 2018](https://doi.org/10.1007/s10648-018-9434-x).

Application requires explicit attention. A September 2026 semester long classroom intervention improved performance on repeated questions, but found no overall transfer benefit on its related question measure; exploratory analyses found both positive and negative transfer. [Ramgopal and colleagues, 2026](https://pubmed.ncbi.nlm.nih.gov/42803776/).

For complex procedures, a promising format is to study an example, predict and execute its next step, then reveal feedback. A 2025 randomised study with 164 adults found better recall and problem solving after one week using this approach. The retrieval group studied for about 31 minutes versus 13 minutes, so the result does not establish greater learning per minute. [Van den Broek and colleagues, 2025](https://research-portal.uu.nl/ws/files/269108220/1-s2.0-S0959475225001203-main.pdf).

## Use mnemonics where the material benefits

A memory palace places vivid associations at familiar locations and retrieves them by mentally following a route. It is well suited to ordered lists and otherwise arbitrary information.

A 2025 systematic review found benefits, including a large pooled effect for immediate serial recall against rehearsal. However, it rated evidence quality low to very low because of bias and other limitations. The headline effect should not be read as proof of superior retention for every subject or timescale. [Ondřej, 2025](https://pmc.ncbi.nlm.nih.gov/articles/PMC12514325/).

Use a mnemonic when it makes an association easier to establish. Then practise retrieving the actual knowledge over time. For concepts that already have a meaningful causal structure, an accurate explanation may be more useful than inventing an elaborate image.

## Draw relationships and check the result

Drawing is useful when the representation helps you understand the material. Its benefits are conditional.

A study published online in December 2024, with 91 participants, found no recall advantage for drawing scientific terms and definitions over paraphrasing or writing them. [Mykoniatis, Smith and Drake, 2025](https://journals.sagepub.com/doi/pdf/10.1177/02762374241303266).

A 2025 meta-analysis compared supported and unsupported drawing. Support that helped learners integrate drawings with the material improved comprehension; support in general did not reliably help. This comparison concerns the kind of support given to drawing, rather than establishing drawing's superiority over all other strategies. [Leutner and Biele, 2025](https://link.springer.com/article/10.1007/s10648-025-10067-7).

For personal practice, reconstruct a mechanism, system or process from memory, compare it with a correct representation, and repair the missing relationships. Artistic detail adds little to this purpose.

## Pretest before initial study

Trying to answer a question before learning the answer can improve subsequent learning when followed by an opportunity to study the correct answer. Benefits have been found with texts, videos and lectures, although they vary with the procedure and assessment. [Pan and Carpenter, 2023](https://doi.org/10.1007/s10648-023-09814-5).

Use a few questions before reading a chapter or watching a lecture. Briefly attempt them, study, then answer again. This is an adjunct to the main learning process; prolonged uninformed guessing is unnecessary.

## Allow sleep between learning occasions

In a vocabulary experiment with 40 participants, learning and relearning separated by sleep required less relearning and supported better later retention than sessions separated by daytime wakefulness. Follow-up extended to six months. The small study supports allowing sleep between sessions, without establishing one ideal study time for everyone. [Mazza and colleagues, 2016](https://pubmed.ncbi.nlm.nih.gov/27530500/).

The practical implication is to spread learning across days and preserve sleep rather than extending a session to accumulate more immediate successes.

# How scheduling research has advanced

A modern system separates two problems: estimating what you will remember, and deciding how to spend the next minute of study. Better predictions help, but the policy still needs an objective, such as readiness for an exam or knowledge maintained over years.

## From fixed rules to learned memory models

| Approach | What it adds | How to interpret it |
| - | - | - |
| Leitner style boxes and SM-2 style rules | Simple schedules that react to performance | Useful baselines with limited flexibility |
| Half-life regression | Fits memory decay from learner and item features | An early large scale demonstration of learned prediction |
| FSRS | Estimates difficulty, stability and recall probability from review history | A practical open approach with personal optimisation |
| SSP-MMC and related optimisation | Explicitly optimises a review policy under a memory model | Advances the allocation problem as well as prediction |
| Newer neural predictors | Uses richer histories and contextual information | Promising prediction results requiring learning outcome validation |

Duolingo's 2016 half-life regression paper reported substantially improved prediction over its baselines. Its operational experiments also measured engagement. “Retention” in an app experiment can mean users returning to the app, so engagement improvements should be kept separate from remembered knowledge. [Settles and Meeder, 2016](https://aclanthology.org/P16-1174.pdf).

Research on SSP-MMC explicitly links memory prediction and schedule optimisation. The 2023 paper reported prediction and cost improvements using real review data and simulated environments. Those percentages are specific to its models and baselines; they are not established savings for an individual learner. [Ye and colleagues, 2023](https://ieeexplore.ieee.org/abstract/document/10059206/).

Similarly, the MEMORIZE work derived an optimal policy under specified memory models and objectives and evaluated historical Duolingo data. Mathematical optimality is conditional on those assumptions. Its natural experiment was not a randomised trial of a newly deployed scheduler. [Tabibian and colleagues, 2019](https://doi.org/10.1073/pnas.1815156116).

## The frontier in October 2026

**FSRS-7 has reached the upstream implementation.** The integration was merged on 8 September 2026. It extends modelling to fractional intervals and richer short term behaviour. The stable Anki release checked for this report, 26.09.3, still uses FSRS-6 through library version 6.6.2. An upstream merge and an app release are distinct milestones. [FSRS-7 integration](https://github.com/open-spaced-repetition/fsrs-rs/pull/426), [Anki releases](https://github.com/ankitects/anki/releases), [released Anki dependency](https://github.com/ankitects/anki/blob/26.09.3/Cargo.toml).

**Neural predictors are ahead on the current open benchmark.** Its evaluation includes roughly 350 million reviews from 9,999 Anki collections. In the table excluding same day outcomes, log loss is approximately 0.346 for FSRS-6, 0.337 for FSRS-7 with recency weighting, and 0.319 for RWKV-Curve; lower is better. RWKV uses richer inputs and a different training design, so this is not an isolated comparison of model architecture. These results measure prediction on logged reviews. They do not establish how much more a learner retains when assigned to each scheduler. [Benchmark methods and results](https://github.com/open-spaced-repetition/srs-benchmark).

**Study cost is becoming part of the policy.** The current reference implementation includes Cost ADR, which learns a policy for varying desired retention and evaluates memory and time tradeoffs in simulation. This is directly relevant to maximising knowledge per unit of effort. Treat it as an emerging engineering direction, with gains conditional on the simulated memory environment. [FSRS reference implementation](https://github.com/open-spaced-repetition/fsrs-rs/blob/main/README.md).

**Commercial algorithms have also continued evolving.** SuperMemo now advertises an SM-20 API with personalised review scheduling. Its product page establishes the offering, but does not establish superiority over FSRS in independent controlled learning comparisons. [SuperMemo API](https://api.supermemo.com/).

The practical choice is to use a maintained implementation with transparent behaviour and accessible review history, while evaluating newer policies against actual delayed learning.

# A system to use now

**My recommendation is Anki with its supported FSRS scheduler, paired with a deliberate method for creating questions and practising applications.** This choice combines an available implementation, personal optimisation and inspectable review history. The evidence supports those capabilities; it does not identify Anki as a universal winner in head to head learning trials.

## Choose a sustainable retention target

Start near **90 percent desired retention** and use the workload simulator to judge whether the review burden fits your available time. Increase the target for particularly important material if the extra workload is worthwhile. A higher target schedules more reviews, and workload rises sharply near 100 percent. Refit parameters as review history accumulates; Anki's documentation suggests monthly optimisation is sufficient. Grade honestly: “Hard” means successful recall with difficulty, while forgetting requires “Again”. [Anki deck options](https://docs.ankiweb.net/manual/deck-options).

Desired retention concerns predicted success when a card is due under the model and review conditions. It does not mean that 90 percent of everything you have ever learned is permanently available in every context.

For your stated goal, evaluate **how much useful knowledge remains accessible within your daily time budget**. A very high success rate on a small collection can maintain less total knowledge than a slightly lower rate on a larger collection. The preferred tradeoff depends on the value of the items and the cost of forgetting them.

## Build questions around knowledge you want to use

This is the proposed workflow.

1. **Understand the source.** Read or work through enough context to explain the idea accurately.

2. **Select valuable knowledge.** Prioritise facts, relationships and procedures likely to matter again.

3. **Write an assessable prompt.** Make the expected response clear and specify what counts as correct.

4. **Attempt and check.** Produce the answer before revealing feedback.

5. **Return through spaced practice.** Let review performance change the timing.

6. **Add another route to important ideas.** Use a contrast, explanation or application.

7. **Repair repeated failures.** Check for ambiguity, missing prerequisites or competing memories before simply adding repetitions.

A small response is easier to score reliably, but retain the surrounding explanation and links. For complex knowledge, a collection of tiny answers needs accompanying practice that reconnects the ideas.

For example, a topic such as memory research could have these distinct prompts.

| Goal | Example prompt |
| - | - |
| Recall a fact | What distinguishes spaced practice from massed practice? |
| Explain a relationship | Why can immediate practice performance mislead you about later retention? |
| Discriminate | Which part of this practice schedule is spacing and which part is interleaving? |
| Apply | Given a limited study budget and material of different importance, what would you change and why? |

These prompts are examples of the proposed system, rather than a tested optimum number or mix of cards.

## Keep maintenance and real use connected

Keep valuable knowledge available for future review even after intervals become long. Reduce new intake when maintenance consistently exceeds the budget. Treat repeated failures as diagnostic information and give particularly costly items a deliberate keep, rewrite or remove decision.

Real work can supply excellent practice when it requires unaided retrieval. Reading an answer or seeing a familiar passage does not demonstrate the same ability. If ordinary use is replacing app reviews, test a sample occasionally to see whether that assumption holds.

## Use AI to support the learning activity

AI can help draft prompts, generate contrasting examples and provide questions about an explanation. Check generated facts against the source and attempt the task before requesting the answer.

A 2025 randomised field experiment with nearly 1,000 mathematics students found that unrestricted AI assistance improved assisted practice performance but could harm subsequent unaided performance. A tutor designed to provide hints and protect learning largely mitigated that harm. The study assessed short term outcomes in one school, rather than long term flashcard retention. [Bastani and colleagues, 2025](https://doi.org/10.1073/pnas.2422633122).

My proposed use is therefore an AI questioner and feedback assistant, with the learner still doing the retrieval and reasoning.

# How to judge whether the system works for you

Run an initial **eight to twelve week evaluation**, then continue checking at longer delays. That duration is a practical proposal, not a scientific threshold.

Measure the outcomes separately.

| Measure | What it tells you |
| - | - |
| Daily study time, including preparing and repairing questions | Whether the system fits your budget |
| Number of valuable knowledge units maintained | Whether retained knowledge is growing |
| Recall on ordinary scheduled reviews | Whether scheduling predictions are reasonably calibrated |
| Delayed recall on a representative sample, including items not currently due | Whether performance extends beyond the review queue |
| Answers to new examples and application questions | Whether the knowledge is usable |
| Repeated failures and time spent per item | Where question design or initial understanding needs work |

If comparing approaches, assign reasonably matched material to each condition and compare delayed performance for a similar time budget. Keep initial familiarity, question quality and assessment format comparable. A personal comparison can guide your choices, but repeated testing itself changes memory and small samples create noisy results.

For broader assessments, use a fresh sample at each checkpoint where possible. Feed missed items back into learning after recording the result. Include questions that require production and application, rather than relying entirely on familiar wording.

For a future custom system, I would prioritise source linked knowledge units, several meaningful prompt types, honest outcome recording, response time, item importance and exportable history. I would evaluate a cost sensitive scheduler against supported FSRS before adopting it. These are design proposals based on the research, rather than individually proven product features.

# Limits that matter to the decision

**Evidence for a technique does not automatically validate an app.** A January 2026 review of Anki studies included 11 studies and 1,135 medical students. Findings were mixed, and use was not randomly assigned in the included studies. Motivation, prior attainment and study time can therefore contribute to associations between app use and exam scores. [Frappa and colleagues, 2026](https://link.springer.com/article/10.1007/s40670-026-02643-5).

**The material changes the answer.** A 2025 mathematics meta-analysis found a reliable spacing benefit across 27 studies, *g* = 0.28. Its smaller testing versus restudy literature comprised seven studies, with an estimate whose confidence interval crossed zero. Remembering definitions and learning to solve unfamiliar problems should therefore be assessed separately. [Murray, Horner and Göbel, 2025](https://link.springer.com/article/10.1007/s10648-025-10035-1).

**Recent findings often cover short delays.** Varied cue experiments tested up to 24 hours; the 2026 error free session study extended to 56 days. These are useful advances, but extending their exact gains to years would go beyond the measurements.

**There is no established universal optimum retention percentage, repetition count or terminal interval.** Your sustainable workload and actual delayed performance must guide the final settings.

This is a focused research synthesis drawing on published experiments, meta-analyses and current primary software sources, rather than a registered systematic review. Some papers were accessible only through their published abstract or author supplied summary. Software status is checked to 24 October 2026; benchmark rankings and release status can change.
