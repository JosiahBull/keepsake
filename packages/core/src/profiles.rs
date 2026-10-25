//! Household identities. Profiles separate content; they are not authentication.
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqliteConnection, SqlitePool};
use utoipa::ToSchema;

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema, FromRow)]
pub struct Profile {
    pub id: i64,
    pub name: String,
    pub handle: String,
    pub setup_complete: bool,
    pub created_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewProfile {
    pub name: String,
    pub handle: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateProfile {
    pub name: String,
}

#[derive(Debug)]
pub enum ProfileError {
    Required,
    Missing,
    Invalid(String),
    Conflict(String),
}

impl std::fmt::Display for ProfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Required => f.write_str("profile_required: choose a profile"),
            Self::Missing => f.write_str("Profile not found"),
            Self::Invalid(message) | Self::Conflict(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for ProfileError {}

fn validate_name(name: &str) -> anyhow::Result<()> {
    if name.trim().is_empty()
        || name.len() > 320
        || name.chars().count() > 80
        || name.chars().any(char::is_control)
    {
        return Err(ProfileError::Invalid(
            "Name must contain 1 to 80 characters and no control characters".into(),
        )
        .into());
    }
    Ok(())
}

fn validate_handle(handle: &str) -> anyhow::Result<()> {
    if !((1..=40).contains(&handle.len())
        && handle
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && handle.bytes().any(|byte| byte.is_ascii_alphabetic()))
    {
        return Err(ProfileError::Invalid(
            "Handle must be 1 to 40 lowercase letters, digits or hyphens, including a letter"
                .into(),
        )
        .into());
    }
    Ok(())
}

pub async fn list(pool: &SqlitePool) -> anyhow::Result<Vec<Profile>> {
    Ok(sqlx::query_as!(
        Profile,
        "SELECT id AS \"id!\", name, handle, setup_complete AS \"setup_complete!: bool\", created_at
         FROM profiles ORDER BY id"
    )
    .fetch_all(pool)
    .await?)
}

pub async fn get(pool: &SqlitePool, id: i64) -> anyhow::Result<Option<Profile>> {
    Ok(sqlx::query_as!(
        Profile,
        "SELECT id AS \"id!\", name, handle, setup_complete AS \"setup_complete!: bool\", created_at
         FROM profiles WHERE id=?",
        id
    )
    .fetch_optional(pool)
    .await?)
}

pub async fn sole(pool: &SqlitePool) -> anyhow::Result<Profile> {
    let profiles = list(pool).await?;
    if profiles.len() != 1 {
        return Err(ProfileError::Required.into());
    }
    Ok(profiles.into_iter().next().expect("one profile"))
}

pub async fn resolve(pool: &SqlitePool, selector: &str) -> anyhow::Result<Profile> {
    let row = if let Ok(id) = selector.parse::<i64>() {
        get(pool, id).await?
    } else {
        sqlx::query_as!(
            Profile,
            "SELECT id AS \"id!\", name, handle, setup_complete AS \"setup_complete!: bool\", created_at
             FROM profiles WHERE handle=? COLLATE NOCASE",
            selector
        )
        .fetch_optional(pool)
        .await?
    };
    row.ok_or_else(|| ProfileError::Missing.into())
}

pub(crate) async fn require(connection: &mut SqliteConnection, id: i64) -> anyhow::Result<()> {
    let exists = sqlx::query_scalar!(
        "SELECT EXISTS(SELECT 1 FROM profiles WHERE id=?) AS \"exists!: bool\"",
        id
    )
    .fetch_one(connection)
    .await?;
    if !exists {
        return Err(ProfileError::Missing.into());
    }
    Ok(())
}

pub(crate) async fn require_fact(
    connection: &mut SqliteConnection,
    profile_id: i64,
    fact_id: i64,
) -> anyhow::Result<()> {
    let exists = sqlx::query_scalar!(
        "SELECT EXISTS(SELECT 1 FROM facts WHERE id=? AND profile_id=?) AS \"exists!: bool\"",
        fact_id,
        profile_id
    )
    .fetch_one(connection)
    .await?;
    if !exists {
        return Err(crate::scheduling::StudyError::Missing("Fact not found".into()).into());
    }
    Ok(())
}

pub(crate) async fn require_target(
    connection: &mut SqliteConnection,
    profile_id: i64,
    target_id: i64,
) -> anyhow::Result<()> {
    let exists = sqlx::query_scalar!(
        "SELECT EXISTS(
           SELECT 1 FROM learning_targets t JOIN facts f ON f.id=t.fact_id
           WHERE t.id=? AND f.profile_id=?
         ) AS \"exists!: bool\"",
        target_id,
        profile_id
    )
    .fetch_one(connection)
    .await?;
    if !exists {
        return Err(
            crate::scheduling::StudyError::Missing("Learning target not found".into()).into(),
        );
    }
    Ok(())
}

pub async fn create(pool: &SqlitePool, input: NewProfile) -> anyhow::Result<Profile> {
    validate_name(&input.name)?;
    validate_handle(&input.handle)?;

    crate::db::write_transaction(pool, move |connection| {
        let input = input.clone();
        Box::pin(async move {
            let exists = sqlx::query_scalar!(
                "SELECT EXISTS(SELECT 1 FROM profiles WHERE handle=? COLLATE NOCASE) AS \"exists!: bool\"",
                input.handle
            )
            .fetch_one(&mut *connection)
            .await?;
            if exists {
                return Err(ProfileError::Conflict("Handle already exists".into()).into());
            }

            let name = input.name.trim();
            let saved = sqlx::query_as!(
                Profile,
                "INSERT INTO profiles(name, handle, setup_complete) VALUES(?, ?, 1)
                 RETURNING id AS \"id!\", name, handle, setup_complete AS \"setup_complete!: bool\", created_at",
                name,
                input.handle
            )
            .fetch_one(&mut *connection)
            .await?;
            let defaults = include_str!("default-settings.json").trim();
            sqlx::query!(
                "INSERT INTO scheduler_settings(profile_id, value_json, legacy_review_cutoff)
                 VALUES(?, ?, 0)",
                saved.id,
                defaults
            )
            .execute(connection)
            .await?;
            Ok(saved)
        })
    })
    .await
}

pub async fn rename(pool: &SqlitePool, id: i64, input: UpdateProfile) -> anyhow::Result<Profile> {
    validate_name(&input.name)?;
    let name = input.name.trim();
    sqlx::query_as!(
        Profile,
        "UPDATE profiles SET name=?, setup_complete=1 WHERE id=?
         RETURNING id AS \"id!\", name, handle, setup_complete AS \"setup_complete!: bool\", created_at",
        name,
        id
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ProfileError::Missing.into())
}
