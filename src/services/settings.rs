use sqlx::{any::AnyRow, FromRow, Row};

use crate::args::Args;
use crate::database::Database;
use crate::domain_error::{DomainError, DomainResult};
use crate::time::unix_timestamp;

#[derive(Clone, Debug, PartialEq)]
pub struct InstanceSettings {
    pub site_name: String,
    pub home_mode: String,
    pub public_explore_enabled: bool,
    pub invitations_enabled: bool,
    pub attachments_enabled: bool,
    pub qr_codes_enabled: bool,
    pub default_format: String,
    pub default_language: String,
    pub default_visibility: String,
    pub default_expiration_seconds: Option<i64>,
    pub updated_at: i64,
    pub updated_by_user_id: Option<i64>,
}

impl<'r> FromRow<'r, AnyRow> for InstanceSettings {
    fn from_row(row: &'r AnyRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            site_name: row.try_get("site_name")?,
            home_mode: row.try_get("home_mode")?,
            public_explore_enabled: row.try_get::<i64, _>("public_explore_enabled")? != 0,
            invitations_enabled: row.try_get::<i64, _>("invitations_enabled")? != 0,
            attachments_enabled: row.try_get::<i64, _>("attachments_enabled")? != 0,
            qr_codes_enabled: row.try_get::<i64, _>("qr_codes_enabled")? != 0,
            default_format: row.try_get("default_format")?,
            default_language: row.try_get("default_language")?,
            default_visibility: row.try_get("default_visibility")?,
            default_expiration_seconds: row.try_get("default_expiration_seconds")?,
            updated_at: row.try_get("updated_at")?,
            updated_by_user_id: row.try_get("updated_by_user_id")?,
        })
    }
}

pub async fn initialize(repo: &Database, args: &Args) -> DomainResult<InstanceSettings> {
    let now = unix_timestamp();
    sqlx::query(
        "INSERT INTO instance_settings(
           id,site_name,home_mode,public_explore_enabled,invitations_enabled,
           attachments_enabled,qr_codes_enabled,default_format,default_language,
           default_visibility,default_expiration_seconds,updated_at
         ) VALUES(1,$1,$2,1,1,$3,$4,'text','plaintext','unlisted',NULL,$5)
         ON CONFLICT(id) DO NOTHING",
    )
    .bind(args.site_name.as_deref().unwrap_or("Racebin"))
    .bind(if args.plain_home { "plain" } else { "standard" })
    .bind(i64::from(args.attachments_enabled))
    .bind(i64::from(args.qr_codes))
    .bind(now)
    .execute(repo.pool())
    .await
    .map_err(DomainError::from)?;
    get(repo).await
}

pub async fn get(repo: &Database) -> DomainResult<InstanceSettings> {
    sqlx::query_as(
        "SELECT site_name,home_mode,public_explore_enabled,invitations_enabled,
                attachments_enabled,qr_codes_enabled,default_format,default_language,
                default_visibility,default_expiration_seconds,updated_at,updated_by_user_id
         FROM instance_settings WHERE id=1",
    )
    .fetch_optional(repo.pool())
    .await
    .map_err(DomainError::from)?
    .ok_or_else(|| DomainError::internal("Instance settings are not initialized"))
}

pub async fn replace(
    repo: &Database,
    actor_id: i64,
    value: &InstanceSettings,
) -> DomainResult<InstanceSettings> {
    validate(value)?;
    let now = unix_timestamp();
    sqlx::query(
        "UPDATE instance_settings SET
           site_name=$1,home_mode=$2,public_explore_enabled=$3,invitations_enabled=$4,
           attachments_enabled=$5,qr_codes_enabled=$6,default_format=$7,
           default_language=$8,default_visibility=$9,default_expiration_seconds=$10,
           updated_at=$11,updated_by_user_id=$12
         WHERE id=1",
    )
    .bind(value.site_name.trim())
    .bind(&value.home_mode)
    .bind(i64::from(value.public_explore_enabled))
    .bind(i64::from(value.invitations_enabled))
    .bind(i64::from(value.attachments_enabled))
    .bind(i64::from(value.qr_codes_enabled))
    .bind(&value.default_format)
    .bind(&value.default_language)
    .bind(&value.default_visibility)
    .bind(value.default_expiration_seconds)
    .bind(now)
    .bind(actor_id)
    .execute(repo.pool())
    .await
    .map_err(DomainError::from)?;
    get(repo).await
}

fn validate(value: &InstanceSettings) -> DomainResult<()> {
    let site_name = value.site_name.trim();
    if site_name.is_empty() || site_name.chars().count() > 64 {
        return Err(DomainError::validation_code(
            "invalid_settings",
            "Site name must contain 1 to 64 characters",
        ));
    }
    if !matches!(value.home_mode.as_str(), "standard" | "plain")
        || !matches!(value.default_format.as_str(), "text" | "markdown")
        || !matches!(
            value.default_visibility.as_str(),
            "public" | "unlisted" | "private"
        )
        || value.default_language.trim().is_empty()
        || value
            .default_expiration_seconds
            .is_some_and(|seconds| seconds <= 0)
    {
        return Err(DomainError::validation_code(
            "invalid_settings",
            "Instance settings contain an unsupported value",
        ));
    }
    Ok(())
}
