//! Catalogue admission and rebuild locks. Call inside the owning transaction.

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
