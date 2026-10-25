use anyhow::{Context, bail};
use clap::{Args, Subcommand};
use keepsake_core::{
    db,
    scheduling::{self, QueueQuery, SettingsUpdate, StudySubmission},
};
use std::{
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Subcommand)]
pub(crate) enum StudyCommand {
    /// Submit a study attempt. Reuse its submission_id when retrying.
    Submit {
        /// StudySubmission JSON; '-' reads standard input (maximum 1 MiB).
        #[arg(long)]
        file: PathBuf,
    },

    /// Display settings, or update timezone, new intake, and daily budget.
    Settings(SettingsOptions),

    /// Approve a source-checked flashcard or cloze with the canonical answer.
    Approve {
        #[arg(value_parser = clap::value_parser!(i64).range(1..))]
        practice_id: i64,
    },

    /// Export one JSON page of replayable study events.
    Events {
        #[arg(long, default_value_t = 0, value_parser = clap::value_parser!(i64).range(0..))]
        after: i64,
        #[arg(long, default_value_t = 100, value_parser = clap::value_parser!(i64).range(1..=1000))]
        limit: i64,
    },
}

#[derive(Args)]
pub(crate) struct SettingsOptions {
    /// IANA timezone used to define study days, for example Europe/London.
    #[arg(long)]
    timezone: Option<String>,
    /// Maximum new recall targets introduced each local day (0–100).
    #[arg(long, value_parser = clap::value_parser!(i64).range(0..=100))]
    new_limit: Option<i64>,
    /// Soft daily study budget in seconds (60–14400).
    #[arg(long, value_parser = clap::value_parser!(i64).range(60..=14400))]
    daily_budget_seconds: Option<i64>,
}

pub(crate) async fn due(
    database: &str,
    profile: Option<String>,
    limit: i64,
    json: bool,
) -> anyhow::Result<()> {
    let pool = db::connect(database).await?;
    let actor = crate::profiles::resolve(&pool, profile).await?;
    let queue = scheduling::queue_for(&pool, actor, &QueueQuery { limit }).await?;
    pool.close().await;

    if json {
        return crate::print_json(&queue);
    }

    if queue.items.is_empty() {
        println!("No recall questions due.");
    } else {
        for question in &queue.items {
            println!(
                "{}\t{}\t{}",
                question.target_id, question.fact.id, question.practice.prompt
            );
        }
    }
    println!(
        "Due: {}. New remaining today: {}.",
        queue.total_due, queue.new_remaining
    );
    if let Some(at) = queue.next_due_at {
        println!("Next due: {at} (Unix seconds).");
    }
    if queue.budget_reached {
        println!("Daily study budget reached.");
    }
    Ok(())
}

pub(crate) async fn run(
    database: &str,
    profile: Option<String>,
    command: StudyCommand,
    json: bool,
) -> anyhow::Result<()> {
    // Parse input before opening SQLite so malformed files cannot create a library.
    let submission = match &command {
        StudyCommand::Submit { file } => Some(read_submission(file)?),
        _ => None,
    };
    let pool = db::connect(database).await?;
    let actor = crate::profiles::resolve(&pool, profile).await?;

    match command {
        StudyCommand::Submit { .. } => {
            let result =
                scheduling::submit_for(&pool, actor, submission.expect("parsed submission"))
                    .await?;
            if json {
                crate::print_json(&result)?;
            } else {
                println!(
                    "{}",
                    if result.qualifying {
                        "Recall review saved."
                    } else {
                        "Study activity saved."
                    }
                );
                if let Some(at) = result.next_due_at {
                    println!("Next due: {at} (Unix seconds).");
                }
            }
        }
        StudyCommand::Settings(options) => {
            let current = scheduling::settings_for(&pool, actor).await?;
            let updated = if options.timezone.is_some()
                || options.new_limit.is_some()
                || options.daily_budget_seconds.is_some()
            {
                scheduling::update_settings_for(
                    &pool,
                    actor,
                    SettingsUpdate {
                        timezone: options.timezone.unwrap_or(current.timezone),
                        new_limit: options.new_limit.unwrap_or(current.new_limit),
                        daily_budget_seconds: options
                            .daily_budget_seconds
                            .unwrap_or(current.daily_budget_seconds),
                        expected_version: current.version,
                    },
                )
                .await?
            } else {
                current
            };
            if json {
                crate::print_json(&updated)?;
            } else {
                println!("Timezone: {}", updated.timezone);
                println!("New targets per day: {}", updated.new_limit);
                println!("Daily budget: {} seconds", updated.daily_budget_seconds);
                println!("Desired retention: {}", updated.desired_retention);
                println!("Algorithm: {}", updated.algorithm);
            }
        }
        StudyCommand::Approve { practice_id } => {
            scheduling::approve_practice_for(&pool, actor, practice_id).await?;
            if json {
                crate::print_json(
                    &serde_json::json!({ "practice_id": practice_id, "approved": true }),
                )?;
            } else {
                println!("Practice {practice_id} approved.");
            }
        }
        StudyCommand::Events { after, limit } => {
            crate::print_json(&scheduling::export_events_for(&pool, actor, after, limit).await?)?;
        }
    }

    pool.close().await;
    Ok(())
}

fn read_submission(path: &Path) -> anyhow::Result<StudySubmission> {
    let reader: Box<dyn Read> = if path.as_os_str() == "-" {
        Box::new(std::io::stdin())
    } else {
        Box::new(std::fs::File::open(path).context("open study submission JSON")?)
    };
    let mut bytes = Vec::new();
    reader.take(1024 * 1024 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > 1024 * 1024 {
        bail!("study submission JSON exceeds 1 MiB");
    }
    serde_json::from_slice(&bytes).context("parse study submission JSON")
}
