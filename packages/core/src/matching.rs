//! Related, unambiguous matching rounds drawn from one topic tag.

use std::{cmp::Reverse, collections::HashSet};

use sqlx::SqlitePool;

use crate::{
    facts::FactRow,
    paging::{FactSample, FactSummary, SampleQuery},
};

pub(crate) async fn sample(
    pool: &SqlitePool,
    profile_id: i64,
    query: &SampleQuery,
) -> anyhow::Result<FactSample> {
    query.validate()?;

    // Count distinct, unambiguous pairs inside each bank before choosing a seed.
    // The seed's smallest eligible bank wins over broad tags such as "beginner".
    // db::connect supplies Unicode-aware, whitespace-trimming NOCASE comparisons.
    let rows = sqlx::query!(
        "WITH normalized AS (
           SELECT id, prompt COLLATE NOCASE AS prompt_key, answer COLLATE NOCASE AS answer_key
           FROM facts WHERE profile_id = ?
         ), tagged AS (
           SELECT DISTINCT n.id, n.prompt_key, n.answer_key,
             tag.value COLLATE NOCASE AS bank
           FROM normalized n JOIN facts f ON f.id = n.id, json_each(f.tags) tag
           WHERE tag.type = 'text' AND tag.value COLLATE NOCASE <> ''
         ), unambiguous AS (
           SELECT t.* FROM tagged t WHERE NOT EXISTS (
             SELECT 1 FROM tagged other
             WHERE other.bank = t.bank AND other.prompt_key = t.prompt_key
               AND other.answer_key <> t.answer_key
           )
         ), pairs AS (
           SELECT bank, answer_key, min(id) AS id
           FROM unambiguous GROUP BY bank, answer_key
         ), banks AS (
           SELECT bank, count(*) AS pair_count FROM pairs
           GROUP BY bank HAVING count(*) >= 3
         ), seed AS (
           SELECT p.id FROM pairs p JOIN banks b ON b.bank = p.bank
           GROUP BY p.id ORDER BY random() LIMIT 1
         ), chosen_bank AS (
           SELECT p.bank FROM pairs p JOIN banks b ON b.bank = p.bank
           WHERE p.id = (SELECT id FROM seed)
           ORDER BY b.pair_count, random() LIMIT 1
         ), ranked AS (
           SELECT p.id, count(*) AS shared_tags
           FROM pairs p JOIN chosen_bank b ON b.bank = p.bank
           JOIN tagged t ON t.id = p.id
           WHERE t.bank IN (SELECT bank FROM tagged WHERE id = (SELECT id FROM seed))
           GROUP BY p.id
         )
         SELECT f.id AS \"id!\", f.prompt, f.answer, f.context, f.source, f.tags, f.created_at,
           r.shared_tags AS \"shared_tags!: i64\"
         FROM ranked r JOIN facts f ON f.id = r.id
         ORDER BY (f.id = (SELECT id FROM seed)) DESC, r.shared_tags DESC, random()
         LIMIT 100",
        profile_id
    )
    .fetch_all(pool)
    .await?;

    let candidates = rows
        .into_iter()
        .map(|row| {
            let fact = FactRow {
                id: row.id,
                prompt: row.prompt,
                answer: row.answer,
                context: row.context,
                source: row.source,
                tags: row.tags,
                created_at: row.created_at,
            }
            .try_into()?;

            Ok(Candidate {
                fact,
                shared_tags: row.shared_tags,
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    let mut sample = select_pairs(candidates, query.limit as usize);
    let mut connection = pool.acquire().await?;
    for fact in &mut sample.items {
        crate::paging::enrich_summary(&mut connection, fact).await?;
    }
    Ok(sample)
}

fn select_pairs(mut candidates: Vec<Candidate>, limit: usize) -> FactSample {
    if candidates.is_empty() {
        return FactSample { items: vec![] };
    }

    // SQL keeps the seed first and caps materialized candidates at 100. Prefer
    // matching question patterns among candidates sharing the same topic tags.
    let seed = candidates.remove(0).fact;
    let words = question_words(&seed.prompt);
    candidates.sort_by_cached_key(|candidate| {
        Reverse((
            candidate.shared_tags,
            question_similarity(&words, &question_words(&candidate.fact.prompt)),
        ))
    });

    let mut items = vec![seed];
    items.extend(
        candidates
            .into_iter()
            .take(limit - 1)
            .map(|candidate| candidate.fact),
    );

    FactSample { items }
}

struct Candidate {
    fact: FactSummary,
    shared_tags: i64,
}

fn question_words(prompt: &str) -> HashSet<String> {
    prompt
        .split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .filter(|word| {
            !matches!(
                word.as_str(),
                "a" | "an" | "the" | "what" | "which" | "how" | "does" | "do" | "is" | "are"
            )
        })
        .collect()
}

fn question_similarity(left: &HashSet<String>, right: &HashSet<String>) -> usize {
    let overlap = left.intersection(right).count();
    let union = left.len() + right.len() - overlap;
    overlap * 1_000 / union.max(1)
}

#[cfg(test)]
mod tests {
    use super::{Candidate, FactSummary, select_pairs};

    #[test]
    fn prefers_similar_questions_within_a_bank() {
        let candidates = [
            (
                1,
                "For food or drink, what does ‘hanawiti’ mean in te reo Māori?",
            ),
            (2, "What does Cargo --locked do?"),
            (
                3,
                "For food or drink, what does ‘heihei’ mean in te reo Māori?",
            ),
            (
                4,
                "For food or drink, what does ‘wai’ mean in te reo Māori?",
            ),
            (
                5,
                "For food or drink, what does ‘miraka’ mean in te reo Māori?",
            ),
        ]
        .into_iter()
        .map(|(id, prompt)| Candidate {
            fact: FactSummary {
                id,
                shared_by: None,
                prompt: prompt.into(),
                answer: format!("Answer {id}"),
                context: String::new(),
                source: String::new(),
                tags: vec!["broad-topic".into()],
                created_at: String::new(),
            },
            shared_tags: 1,
        })
        .collect();

        let sample = select_pairs(candidates, 4);
        assert_eq!(
            sample.items.iter().map(|fact| fact.id).collect::<Vec<_>>(),
            [1, 3, 4, 5]
        );
    }
}
