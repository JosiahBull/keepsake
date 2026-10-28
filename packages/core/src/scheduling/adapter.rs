//! Pure scheduling adapter for the pinned FSRS release.
//!
//! FSRS 6.6.2 accepts elapsed whole days. Preserve exact elapsed seconds in the
//! event log, and pass completed 24-hour periods to its public transition API.
//! Learning-step overrides belong to the caller, separate from model output.

use anyhow::{Context, ensure};
use fsrs::{FSRS, ItemState, MemoryState};
use serde::{Deserialize, Serialize};

pub const ALGORITHM: &str = "fsrs-6.6.2";
const SECONDS_PER_DAY: i64 = 86_400;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct Memory {
    pub stability: f32,
    pub difficulty: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transition {
    pub memory: Memory,
    pub model_interval_days: f32,
    pub interval_seconds: i64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transitions {
    pub again: Transition,
    pub hard: Transition,
    pub good: Transition,
    pub easy: Transition,
}

pub fn default_parameters() -> Vec<f32> {
    fsrs::DEFAULT_PARAMETERS.to_vec()
}

pub fn next(
    memory: Option<Memory>,
    elapsed_seconds: i64,
    retention: f32,
    parameters: &[f32],
) -> anyhow::Result<Transitions> {
    ensure!(elapsed_seconds >= 0, "elapsed time must not be negative");
    let elapsed_days = u32::try_from(elapsed_seconds / SECONDS_PER_DAY)
        .context("elapsed time exceeds the pinned model's supported range")?;
    ensure!(
        retention.is_finite() && retention > 0.0 && retention < 1.0,
        "desired retention must be finite and strictly between zero and one"
    );
    ensure!(
        parameters.len() == fsrs::DEFAULT_PARAMETERS.len()
            && parameters.iter().all(|parameter| parameter.is_finite()),
        "FSRS 6 parameters must contain exactly 21 finite numbers"
    );

    if let Some(memory) = memory {
        ensure!(
            memory.stability.is_finite() && memory.stability > 0.0,
            "memory stability must be finite and positive"
        );
        ensure!(
            memory.difficulty.is_finite() && (1.0..=10.0).contains(&memory.difficulty),
            "memory difficulty must be finite and between one and ten"
        );
    }

    // The library owns parameter bounds and its documented clipping behavior.
    // Requiring a full vector avoids implicit conversion of older FSRS versions.
    let model = FSRS::new(parameters).context("construct pinned FSRS model")?;
    let states = model
        .next_states(
            memory.map(
                |Memory {
                     stability,
                     difficulty,
                 }| MemoryState {
                    stability,
                    difficulty,
                },
            ),
            retention,
            elapsed_days,
        )
        .context("calculate FSRS transitions")?;

    Ok(Transitions {
        again: transition(states.again)?,
        hard: transition(states.hard)?,
        good: transition(states.good)?,
        easy: transition(states.easy)?,
    })
}

fn transition(state: ItemState) -> anyhow::Result<Transition> {
    ensure!(
        state.memory.stability.is_finite()
            && state.memory.stability > 0.0
            && state.memory.difficulty.is_finite()
            && (1.0..=10.0).contains(&state.memory.difficulty)
            && state.interval.is_finite()
            && state.interval >= 0.0,
        "FSRS returned an invalid state or interval"
    );

    // Follow the release's scheduling example: round to whole days, minimum one.
    // Keep the unrounded model interval for auditing and later policy comparison.
    let interval_days = f64::from(state.interval).round().max(1.0);
    ensure!(
        interval_days <= (i64::MAX / SECONDS_PER_DAY) as f64,
        "FSRS interval exceeds the supported timestamp range"
    );
    let interval_seconds = (interval_days as i64)
        .checked_mul(SECONDS_PER_DAY)
        .context("FSRS interval exceeds the supported timestamp range")?;

    Ok(Transition {
        memory: Memory {
            stability: state.memory.stability,
            difficulty: state.memory.difficulty,
        },
        model_interval_days: state.interval,
        interval_seconds,
    })
}

#[cfg(test)]
mod tests {
    use super::{Memory, SECONDS_PER_DAY, default_parameters, next};

    fn close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 0.0001,
            "expected {expected}, received {actual}"
        );
    }

    #[test]
    fn new_cards_match_the_released_reference_and_keep_policy_separate() {
        // fsrs 6.6.2 src/inference.rs next_states documentation fixture:
        // https://docs.rs/fsrs/6.6.2/fsrs/struct.FSRS.html#method.next_states
        let states = next(None, 0, 0.9, &default_parameters()).unwrap();
        for (actual, stability, difficulty, days) in [
            (states.again, 0.212, 6.4133, 1),
            (states.hard, 1.2931, 5.1121707, 1),
            (states.good, 2.3065, 2.118104, 2),
            (states.easy, 8.2956, 1.0, 8),
        ] {
            close(actual.memory.stability, stability);
            close(actual.memory.difficulty, difficulty);
            close(actual.model_interval_days, stability);
            assert_eq!(actual.interval_seconds, days * SECONDS_PER_DAY);
        }
        assert_ne!(states.again.interval_seconds, 600);
    }

    #[test]
    fn completed_elapsed_days_preserve_the_librarys_same_day_branch() {
        let parameters = default_parameters();
        let memory = Memory {
            stability: 10.0,
            difficulty: 5.0,
        };
        let immediately = next(Some(memory), 0, 0.9, &parameters).unwrap();
        let after_ten_minutes = next(Some(memory), 600, 0.9, &parameters).unwrap();
        let before_one_day = next(Some(memory), SECONDS_PER_DAY - 1, 0.9, &parameters).unwrap();
        let after_one_day = next(Some(memory), SECONDS_PER_DAY, 0.9, &parameters).unwrap();

        assert_eq!(immediately, after_ten_minutes);
        assert_eq!(immediately, before_one_day);
        assert!(after_one_day.good.memory.stability > immediately.good.memory.stability);
        assert!(after_ten_minutes.again.memory.stability < memory.stability);
    }

    #[test]
    fn overdue_reviews_use_actual_elapsed_time() {
        let parameters = default_parameters();
        let memory = Some(Memory {
            stability: 7.0,
            difficulty: 5.0,
        });
        let on_time = next(memory, 7 * SECONDS_PER_DAY, 0.9, &parameters).unwrap();
        let late = next(memory, 45 * SECONDS_PER_DAY + 600, 0.9, &parameters).unwrap();
        let expected = fsrs::FSRS::default()
            .next_states(
                Some(fsrs::MemoryState {
                    stability: 7.0,
                    difficulty: 5.0,
                }),
                0.9,
                45,
            )
            .unwrap();

        close(late.good.memory.stability, expected.good.memory.stability);
        close(late.again.memory.stability, expected.again.memory.stability);
        close(late.good.model_interval_days, expected.good.interval);
        assert!(late.good.memory.stability > on_time.good.memory.stability);
    }

    #[test]
    fn review_history_matches_the_released_default_parameter_reference() {
        // fsrs 6.6.2 src/inference.rs test_memory_state reference history.
        let parameters = default_parameters();
        let mut memory = None;
        for (rating, elapsed_days) in [(1, 0), (3, 0), (3, 1), (3, 3), (3, 8), (3, 21)] {
            let states = next(memory, elapsed_days * SECONDS_PER_DAY, 0.9, &parameters).unwrap();
            memory = Some(if rating == 1 {
                states.again.memory
            } else {
                states.good.memory
            });
        }

        let memory = memory.unwrap();
        close(memory.stability, 53.62691);
        close(memory.difficulty, 6.3574867);
    }

    #[test]
    fn retention_changes_intervals_without_changing_observed_memory() {
        let parameters = default_parameters();
        let usual = next(None, 0, 0.9, &parameters).unwrap();
        let higher_retention = next(None, 0, 0.95, &parameters).unwrap();

        assert_eq!(usual.good.memory, higher_retention.good.memory);
        assert!(higher_retention.good.model_interval_days < usual.good.model_interval_days);
    }

    #[test]
    fn invalid_time_retention_parameters_and_memory_are_rejected() {
        let parameters = default_parameters();
        assert!(next(None, -1, 0.9, &parameters).is_err());
        assert!(next(None, i64::MAX, 0.9, &parameters).is_err());
        for retention in [0.0, 1.0, -0.1, 1.1, f32::NAN, f32::INFINITY] {
            assert!(next(None, 0, retention, &parameters).is_err());
        }
        for length in [0, 17, 19, 20, 22] {
            assert!(next(None, 0, 0.9, &vec![1.0; length]).is_err());
        }
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut invalid = parameters.clone();
            invalid[0] = value;
            assert!(next(None, 0, 0.9, &invalid).is_err());
        }
        for (stability, difficulty) in [
            (0.0, 5.0),
            (-1.0, 5.0),
            (f32::NAN, 5.0),
            (10.0, 0.0),
            (10.0, 11.0),
            (10.0, f32::INFINITY),
        ] {
            assert!(
                next(
                    Some(Memory {
                        stability,
                        difficulty,
                    }),
                    0,
                    0.9,
                    &parameters,
                )
                .is_err()
            );
        }
    }
}
