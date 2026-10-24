use keepsake_core::{
    db,
    facts::{self, NewFact, NewReview, ReviewMethod, ReviewOutcome},
    paging::{self, FactQuery, PracticeQuery, ReviewQuery, SampleQuery},
};
use std::collections::HashSet;

fn fact(index: usize) -> NewFact {
    NewFact {
        prompt: format!("Question {index}"),
        answer: format!("Answer {index}"),
        context: format!("Context {index}"),
        source: format!("https://example.org/{index}"),
        tags: vec![format!("tag-{index}")],
        practices: vec![],
    }
}
async fn pool() -> (tempfile::TempDir, sqlx::SqlitePool) {
    let directory = tempfile::tempdir().unwrap();
    let pool = db::connect(&format!(
        "sqlite:{}",
        directory.path().join("paging.db").display()
    ))
    .await
    .unwrap();
    (directory, pool)
}
#[tokio::test]
async fn bounded_keyset_pages_visit_every_fact_without_duplicates() {
    let (_directory, pool) = pool().await;
    for batch in 0..6 {
        facts::create_many(&pool, (batch * 20..batch * 20 + 20).map(fact).collect())
            .await
            .unwrap();
    }
    let mut cursor = None;
    let mut seen = HashSet::new();
    loop {
        let page = paging::facts(
            &pool,
            &FactQuery {
                limit: 17,
                before_id: cursor,
                search: String::new(),
            },
        )
        .await
        .unwrap();
        assert!(page.items.len() <= 17);
        assert_eq!(page.total, 120);
        for item in page.items {
            assert!(seen.insert(item.id));
            assert_eq!(item.context, format!("Context {}", item.id - 1));
        }
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(seen.len(), 120);
    let first = facts::list(&pool).await.unwrap();
    assert_eq!(first.items.len(), 25);
    let visible: HashSet<_> = first.items.into_iter().map(|fact| fact.id).collect();
    let session = paging::practice(&pool, &PracticeQuery { limit: 50 })
        .await
        .unwrap();
    assert_eq!(session.total, 120);
    assert_eq!(session.items.len(), 50);
    assert_eq!(
        session
            .items
            .iter()
            .map(|item| item.practice.id)
            .collect::<HashSet<_>>()
            .len(),
        50
    );
    assert!(
        session
            .items
            .iter()
            .any(|item| !visible.contains(&item.fact.id)),
        "a50prompt sample cannot fit inside the25fact visible page"
    );
    let matching = paging::sample(&pool, &SampleQuery { limit: 4 })
        .await
        .unwrap();
    assert_eq!(matching.items.len(), 4);
    pool.close().await;
}
#[tokio::test]
async fn search_is_server_side_and_treats_wildcards_and_escape_literally() {
    let (_directory, pool) = pool().await;
    let mut literal = fact(1);
    literal.prompt = "Rate 100%_\\ complete".into();
    facts::create_many(&pool, vec![literal, fact(2)])
        .await
        .unwrap();
    for search in ["%_\\", "CONTEXT 1", "tag-1", "Answer 1"] {
        let page = paging::facts(
            &pool,
            &FactQuery {
                search: search.into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(page.total, 1, "{search}");
        assert_eq!(page.items.len(), 1);
    }
    assert!(
        paging::facts(
            &pool,
            &FactQuery {
                search: "missing".into(),
                ..Default::default()
            }
        )
        .await
        .unwrap()
        .items
        .is_empty()
    );
    pool.close().await;
}
#[tokio::test]
async fn review_history_pages_are_bounded_and_preserve_every_review() {
    let (_directory, pool) = pool().await;
    let fact = facts::create(&pool, fact(1)).await.unwrap();
    for index in 0..105 {
        facts::record_review(
            &pool,
            NewReview {
                fact_id: fact.id,
                practice_id: Some(fact.practices[0].id),
                method: ReviewMethod::Flashcard,
                outcome: ReviewOutcome::Good,
                response_ms: index,
            },
        )
        .await
        .unwrap();
    }
    let mut after_id = None;
    let mut ids = Vec::new();
    loop {
        let page = paging::reviews(
            &pool,
            fact.id,
            &ReviewQuery {
                limit: 37,
                after_id,
            },
        )
        .await
        .unwrap();
        assert!(page.items.len() <= 37);
        assert_eq!(page.total, 105);
        ids.extend(page.items.into_iter().map(|review| review.id));
        after_id = page.next_cursor;
        if after_id.is_none() {
            break;
        }
    }
    assert_eq!(ids, (1..=105).collect::<Vec<_>>());
    assert_eq!(
        facts::reviews(&pool, fact.id).await.unwrap().items.len(),
        50
    );
    pool.close().await;
}
#[tokio::test]
async fn matching_excludes_ambiguous_questions_and_duplicate_answers() {
    let (_directory, pool) = pool().await;
    let first = fact(0);
    let mut duplicate = fact(1);
    duplicate.answer = first.answer.clone();
    let conflicting = NewFact {
        prompt: first.prompt.clone(),
        answer: "Conflicting answer".into(),
        ..fact(2)
    };
    facts::create_many(
        &pool,
        vec![first, duplicate, conflicting, fact(3), fact(4), fact(5)],
    )
    .await
    .unwrap();
    let sample = paging::sample(&pool, &SampleQuery::default())
        .await
        .unwrap();
    assert_eq!(sample.items.len(), 4);
    assert!(sample.items.iter().all(|fact| fact.prompt != "Question 0"));
    assert_eq!(
        sample
            .items
            .iter()
            .map(|fact| &fact.answer)
            .collect::<HashSet<_>>()
            .len(),
        4
    );
    pool.close().await;
}
#[tokio::test]
async fn rejects_out_of_range_bounds_without_running_unbounded_queries() {
    let (_directory, pool) = pool().await;
    for limit in [0, -1, 101] {
        assert!(
            paging::facts(
                &pool,
                &FactQuery {
                    limit,
                    ..Default::default()
                }
            )
            .await
            .is_err()
        );
        assert!(
            paging::reviews(
                &pool,
                1,
                &ReviewQuery {
                    limit,
                    ..Default::default()
                }
            )
            .await
            .is_err()
        );
    }
    assert!(
        paging::practice(&pool, &PracticeQuery { limit: 51 })
            .await
            .is_err()
    );
    assert!(
        paging::sample(&pool, &SampleQuery { limit: 5 })
            .await
            .is_err()
    );
    assert!(
        paging::facts(
            &pool,
            &FactQuery {
                before_id: Some(0),
                ..Default::default()
            }
        )
        .await
        .is_err()
    );
    pool.close().await;
}

#[tokio::test]
async fn search_folds_maori_macrons_in_every_field_without_losing_page_boundaries() {
    let (_directory, pool) = pool().await;
    for (index, field) in ["prompt", "answer", "context", "tags"]
        .into_iter()
        .enumerate()
    {
        for vowels in ["āēīōū", "ĀĒĪŌŪ"] {
            let mut input = fact(index);
            let value = format!("{field} {vowels}");
            match field {
                "prompt" => input.prompt = value,
                "answer" => input.answer = value,
                "context" => input.context = value,
                "tags" => input.tags = vec![value],
                _ => unreachable!(),
            }
            facts::create(&pool, input).await.unwrap();
        }
        let page = paging::facts(
            &pool,
            &FactQuery {
                search: format!("{field} ĀĒĪŌŪ"),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(page.total, 2, "{field}");
        assert_eq!(page.items.len(), 2, "{field}");
    }
    for vowels in ["ā", "Ā", "ē", "Ē", "ī", "Ī", "ō", "Ō", "ū", "Ū"] {
        assert_eq!(
            paging::facts(
                &pool,
                &FactQuery {
                    search: vowels.into(),
                    ..Default::default()
                }
            )
            .await
            .unwrap()
            .total,
            8,
            "{vowels}"
        );
    }
    let mut ids = HashSet::new();
    let mut cursor = None;
    for search in ["ĀĒĪŌŪ", "āēīōū", "ĀĒĪŌŪ"] {
        let page = paging::facts(
            &pool,
            &FactQuery {
                search: search.into(),
                limit: 3,
                before_id: cursor,
            },
        )
        .await
        .unwrap();
        assert_eq!(page.total, 8);
        assert!(page.items.len() <= 3);
        for item in page.items {
            assert!(ids.insert(item.id));
        }
        cursor = page.next_cursor;
    }
    assert!(cursor.is_none());
    assert_eq!(ids.len(), 8);

    let mut greek = fact(99);
    greek.prompt = "Σ".into();
    facts::create(&pool, greek).await.unwrap();
    for (search, expected) in [("Σ", 1), ("σ", 0)] {
        let page = paging::facts(
            &pool,
            &FactQuery {
                search: search.into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(page.total, expected, "{search}");
    }

    let mut literal = fact(100);
    literal.prompt = "Whānau 100%_#\\ Ā".into();
    facts::create(&pool, literal).await.unwrap();
    let page = paging::facts(
        &pool,
        &FactQuery {
            search: "WHĀNAU 100%_#\\ ā".into(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].prompt, "Whānau 100%_#\\ Ā");
    let mut separate = fact(101);
    separate.prompt = "adjoining".into();
    separate.answer = "separate".into();
    facts::create(&pool, separate).await.unwrap();
    assert_eq!(
        paging::facts(
            &pool,
            &FactQuery {
                search: "adjoining separate".into(),
                ..Default::default()
            }
        )
        .await
        .unwrap()
        .total,
        0
    );
    pool.close().await;
}
