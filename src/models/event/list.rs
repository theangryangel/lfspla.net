//! Public event reads and reports of listings missing from a successful sync.

use sea_orm::{ConnectionTrait, DbBackend, DbErr, FromQueryResult, Statement};
use time::OffsetDateTime;

#[derive(Debug, FromQueryResult)]
pub(crate) struct Listing {
    pub id: i64,
    pub league_id: i64,
    pub league_name: String,
    pub league_image_attachment_id: Option<i64>,
    pub season_id: Option<i64>,
    pub season_name: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub starts_at: OffsetDateTime,
    pub details_url: Option<String>,
    pub signup_url: Option<String>,
    pub track_label: Option<String>,
    pub cars_label: Option<String>,
}

pub(crate) struct Filter {
    pub starts_from: OffsetDateTime,
    pub starts_before: Option<OffsetDateTime>,
    pub league_id: Option<i64>,
    pub season_id: Option<i64>,
}

const PREDICATE: &str = "NOT event.hidden AND event.starts_at >= $1
    AND ($2::timestamptz IS NULL OR event.starts_at < $2)
    AND ($3::bigint IS NULL OR event.league_id = $3)
    AND ($4::bigint IS NULL OR event.season_id = $4)";

#[derive(FromQueryResult)]
struct Count {
    total: i64,
}

pub(crate) async fn page(
    database: &impl ConnectionTrait,
    filter: &Filter,
    offset: u64,
    limit: u64,
) -> Result<(Vec<Listing>, u64), DbErr> {
    let values = vec![
        filter.starts_from.into(),
        filter.starts_before.into(),
        filter.league_id.into(),
        filter.season_id.into(),
    ];
    let count = Count::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        format!("SELECT count(*)::bigint AS total FROM event WHERE {PREDICATE}"),
        values.clone(),
    ))
    .one(database)
    .await?
    .ok_or_else(|| DbErr::Custom("event count missing".into()))?;
    let mut values = values;
    values.push(
        i64::try_from(offset)
            .map_err(|_| DbErr::Custom("event offset overflow".into()))?
            .into(),
    );
    values.push(
        i64::try_from(limit)
            .map_err(|_| DbErr::Custom("event limit overflow".into()))?
            .into(),
    );
    let items = Listing::find_by_statement(Statement::from_sql_and_values(DbBackend::Postgres,
        format!("SELECT event.*, league.name AS league_name, league.image_attachment_id AS league_image_attachment_id, season.name AS season_name
            FROM event JOIN league ON league.id = event.league_id LEFT JOIN season ON season.id = event.season_id
            WHERE {PREDICATE} ORDER BY event.starts_at, event.id OFFSET $5 LIMIT $6"), values)).all(database).await?;
    Ok((
        items,
        u64::try_from(count.total).map_err(|_| DbErr::Custom("negative event count".into()))?,
    ))
}

#[derive(FromQueryResult)]
struct Id {
    id: i64,
}

pub(crate) async fn stale(
    database: &impl ConnectionTrait,
    league_id: i64,
    now: OffsetDateTime,
) -> Result<Vec<i64>, DbErr> {
    Ok(Id::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT event.id FROM event
        WHERE event.league_id = $1 AND event.starts_at >= $2 AND event.source = 'lfs_net'
        AND event.last_synced_at < $2 ORDER BY event.id",
        [league_id.into(), now.into()],
    ))
    .all(database)
    .await?
    .into_iter()
    .map(|row| row.id)
    .collect())
}
