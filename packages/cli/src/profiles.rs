//! Profile selection for direct SQLite commands, bound before capture work starts.
use anyhow::{Context, bail};
use clap::Subcommand;
use keepsake_core::{db, profiles};
use sqlx::{SqlitePool, sqlite::SqliteConnectOptions};
use std::str::FromStr;

#[derive(Subcommand)]
pub(crate) enum ProfileCommand {
    /// List stable IDs, handles, and display names.
    List,
    /// Create an empty library with its own study settings.
    Create {
        name: String,
        #[arg(long)]
        handle: String,
    },
    /// Change a display name without changing its ID or handle.
    Rename { target: String, name: String },
}

pub(crate) async fn run(database: &str, command: ProfileCommand, json: bool) -> anyhow::Result<()> {
    let pool = db::connect(database).await?;
    match command {
        ProfileCommand::List => {
            let profiles = profiles::list(&pool).await?;
            if json {
                crate::print_json(&profiles)?;
            } else {
                for profile in profiles {
                    println!("{}\t{}\t{}", profile.id, profile.handle, profile.name);
                }
            }
        }
        ProfileCommand::Create { name, handle } => {
            let profile = profiles::create(&pool, profiles::NewProfile { name, handle }).await?;
            if json {
                crate::print_json(&profile)?;
            } else {
                println!("{}\t{}\t{}", profile.id, profile.handle, profile.name);
            }
        }
        ProfileCommand::Rename { target, name } => {
            let actor = profiles::resolve(&pool, &target).await?;
            let profile =
                profiles::rename(&pool, actor.id, profiles::UpdateProfile { name }).await?;
            if json {
                crate::print_json(&profile)?;
            } else {
                println!("{}\t{}\t{}", profile.id, profile.handle, profile.name);
            }
        }
    }
    pool.close().await;
    Ok(())
}

fn selector(explicit: Option<String>) -> Option<String> {
    explicit.or_else(|| std::env::var("KEEPSAKE_PROFILE").ok())
}

pub(crate) async fn resolve(pool: &SqlitePool, explicit: Option<String>) -> anyhow::Result<i64> {
    let profile = if let Some(selector) = selector(explicit) {
        profiles::resolve(pool, &selector).await?
    } else {
        profiles::sole(pool).await.context(
            "choose a library with --profile ID_OR_HANDLE or KEEPSAKE_PROFILE; run keepsake profiles list",
        )?
    };
    Ok(profile.id)
}

pub(crate) async fn capture_profile(
    database: &str,
    explicit: Option<String>,
) -> anyhow::Result<i64> {
    let options = SqliteConnectOptions::from_str(database)?;
    if !options.get_filename().try_exists()? {
        // Bind the fresh household owner before fetching a URL or calling an
        // agent, without creating files for previews or rejected captures.
        return match selector(explicit).as_deref() {
            Some("1") => Ok(1),
            Some("2") => Ok(2),
            Some(handle) if handle.eq_ignore_ascii_case("default") => Ok(1),
            Some(handle) if handle.eq_ignore_ascii_case("ansam") => Ok(2),
            Some(selector) => bail!(
                "profile {selector:?} does not exist: run keepsake profiles list to see available libraries"
            ),
            None => bail!(
                "profile_required: choose --profile default (Josiah) or --profile ansam before adding facts"
            ),
        };
    }
    let pool = db::connect(database).await?;
    let actor = resolve(&pool, explicit).await?;
    pool.close().await;
    Ok(actor)
}
