//! Ranking-era persistence entities, policies, and validation queries.

mod activity;
mod badges;
mod entity;
mod personal_bests;
mod queries;
mod rankings;

pub(crate) use activity::rank_world_record_holders;
#[cfg(test)]
pub(crate) use badges::qualifies_for_ranking_badge;
pub(crate) use badges::{rebuild_published_badges, request_badge_refresh, retry_badge_refreshes};
pub(crate) use entity::*;
pub use entity::{
    ActiveModel as EraMutation, Column as EraColumn, Entity as EraEntity, Model as EraModel,
};
pub(crate) use personal_bests::{RERANK_SQL, rerank_chart};
pub(crate) use queries::validate_version_requirements;
/// Serializes catalogue changes with admission and validation decisions.
pub(crate) const CATALOGUE_LOCK: i64 = 7_104_152_026;

pub(crate) async fn lock_admission<C: sea_orm::ConnectionTrait>(
    database: &C,
) -> Result<(), sea_orm::DbErr> {
    database
        .query_one_raw(sea_orm::Statement::from_sql_and_values(
            sea_orm::DbBackend::Postgres,
            "SELECT pg_advisory_xact_lock_shared($1)",
            [CATALOGUE_LOCK.into()],
        ))
        .await?;
    Ok(())
}

/// Excludes admissions and PB mutations during bulk repairs.
pub(crate) async fn lock_rebuild<C: sea_orm::ConnectionTrait>(
    database: &C,
) -> Result<(), sea_orm::DbErr> {
    database
        .query_one_raw(sea_orm::Statement::from_sql_and_values(
            sea_orm::DbBackend::Postgres,
            "SELECT pg_advisory_xact_lock($1)",
            [CATALOGUE_LOCK.into()],
        ))
        .await?;
    Ok(())
}

/// Resolves the stable YAML/API slug at an application boundary.
pub(crate) fn find_by_slug(slug: &str) -> sea_orm::Select<EraEntity> {
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    EraEntity::find().filter(EraColumn::Slug.eq(slug))
}
use validator::ValidationError;

/// Whether a value can safely be used as one era path component.
pub(crate) fn validate_era_slug(slug: &str) -> Result<(), ValidationError> {
    if !slug.is_empty()
        && std::path::Path::new(slug).components().count() == 1
        && !matches!(slug, "." | "..")
        && !slug.contains(std::path::MAIN_SEPARATOR)
    {
        return Ok(());
    }
    Err(ValidationError::new("invalid_era_slug"))
}

#[cfg(test)]
mod tests {
    use super::validate_era_slug;

    #[test]
    fn era_slug_must_be_one_safe_path_component() {
        for slug in ["2007-12-21", "s2", "current-era"] {
            assert!(validate_era_slug(slug).is_ok(), "{slug}");
        }
        for slug in ["", ".", "..", "era/other"] {
            assert!(validate_era_slug(slug).is_err(), "{slug}");
        }
    }
}
