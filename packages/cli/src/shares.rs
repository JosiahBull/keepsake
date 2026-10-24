use clap::Subcommand;
use keepsake_core::{profiles, sharing};
use sqlx::SqlitePool;

#[derive(Subcommand)]
pub(crate) enum ShareCommand {
    /// List received offers, or use --sent for offers you sent.
    List {
        #[arg(long)]
        sent: bool,
        #[arg(long, default_value_t = 25, value_parser = clap::value_parser!(i64).range(1..=100))]
        limit: i64,
        /// Continue with the preceding page's next_cursor.
        #[arg(long, value_parser = clap::value_parser!(i64).range(1..))]
        before: Option<i64>,
        #[arg(long, value_parser = ["pending", "accepted", "declined", "withdrawn"])]
        status: Option<String>,
    },
    /// Accept a fact as an independent copy with a fresh recall schedule.
    Accept { share_id: i64 },
    /// Accept several offers atomically; retries reuse their accepted copies.
    AcceptBatch {
        #[arg(required = true)]
        share_ids: Vec<i64>,
    },
    /// Decline an incoming offer.
    Decline { share_id: i64 },
    /// Withdraw a pending offer you sent.
    Withdraw { share_id: i64 },
}

pub(crate) async fn send(
    pool: &SqlitePool,
    actor: i64,
    fact_ids: Vec<i64>,
    recipient: String,
    submission_id: Option<String>,
    json: bool,
) -> anyhow::Result<()> {
    let recipient = profiles::resolve(pool, &recipient).await?;
    let submission_id = submission_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let offers = sharing::send(
        pool,
        actor,
        sharing::ShareRequest {
            submission_id: submission_id.clone(),
            recipient_id: recipient.id,
            fact_ids,
        },
    )
    .await?;
    if !json {
        println!("Offer submission: {submission_id}");
    }
    output(&offers, json)
}

pub(crate) async fn run(
    pool: &SqlitePool,
    actor: i64,
    command: ShareCommand,
    json: bool,
) -> anyhow::Result<()> {
    if let ShareCommand::List {
        sent,
        limit,
        before,
        status,
    } = command
    {
        let page = sharing::page(
            pool,
            actor,
            &sharing::ShareQuery {
                limit,
                before_id: before,
                direction: Some(if sent { "sent" } else { "inbox" }.into()),
                status,
            },
        )
        .await?;
        if json {
            return crate::print_json(&page);
        }
        output(&page.items, false)?;
        if let Some(cursor) = page.next_cursor {
            println!("\nContinue with --before {cursor} and the same filters.");
        }
        return Ok(());
    }
    let offers = match command {
        ShareCommand::List { .. } => unreachable!(),
        ShareCommand::Accept { share_id } => vec![sharing::accept(pool, actor, share_id).await?],
        ShareCommand::AcceptBatch { share_ids } => {
            sharing::accept_batch(pool, actor, sharing::ShareBatch { share_ids }).await?
        }
        ShareCommand::Decline { share_id } => vec![sharing::decline(pool, actor, share_id).await?],
        ShareCommand::Withdraw { share_id } => {
            vec![sharing::withdraw(pool, actor, share_id).await?]
        }
    };
    output(&offers, json)
}

fn output(offers: &[sharing::Share], json: bool) -> anyhow::Result<()> {
    if json {
        return crate::print_json(&offers);
    }
    if offers.is_empty() {
        println!("No shared facts.");
    }
    for offer in offers {
        println!(
            "{}\t{} → {}\t{}\t{}",
            offer.id, offer.sender_name, offer.recipient_name, offer.status, offer.question
        );
        if let Some(id) = offer.accepted_fact_id {
            println!("Accepted copy: {id}");
        }
    }
    Ok(())
}
