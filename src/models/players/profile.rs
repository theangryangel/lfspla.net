use std::collections::HashMap;

use sea_orm::{DatabaseConnection, EntityTrait, QueryOrder};

use crate::models::{
    badges::PlayerBadge,
    eras::{EraColumn, EraEntity},
    players::PlayerChartResult,
};

use super::PlayerModel;

const HIGHLIGHT_LIMIT: usize = 50;

#[derive(Debug)]
pub(crate) struct PlayerProfileStats {
    pub(crate) hotlaps: i64,
    pub(crate) personal_bests: i64,
    pub(crate) world_records: i64,
    pub(crate) podiums: i64,
    pub(crate) eras: usize,
    pub(crate) first_hotlap_at: Option<time::OffsetDateTime>,
    pub(crate) latest_hotlap_at: Option<time::OffsetDateTime>,
}

#[derive(Debug)]
pub(crate) struct PlayerEraProfile {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) hotlaps: i64,
    pub(crate) personal_bests: i64,
    pub(crate) firsts: i64,
    pub(crate) seconds: i64,
    pub(crate) thirds: i64,
    pub(crate) first_hotlap_at: time::OffsetDateTime,
    pub(crate) latest_hotlap_at: time::OffsetDateTime,
    pub(crate) badges: Vec<PlayerBadge>,
}

#[derive(Debug)]
pub(crate) struct PlayerProfile {
    pub(crate) player: PlayerModel,
    pub(crate) stats: PlayerProfileStats,
    pub(crate) eras: Vec<PlayerEraProfile>,
    pub(crate) highlights: Vec<PlayerChartResult>,
}

impl PlayerModel {
    pub(crate) async fn profile(
        self,
        database: &DatabaseConnection,
    ) -> Result<PlayerProfile, sea_orm::DbErr> {
        let era_titles = EraEntity::find()
            .order_by_asc(EraColumn::Id)
            .all(database)
            .await?
            .into_iter()
            .map(|era| (era.id, (era.slug, era.title)))
            .collect::<HashMap<_, _>>();
        let (era_rows, chart_results, mut badges_by_era) = tokio::try_join!(
            self.list_era_stats(database),
            self.list_chart_results(database, None, None, None),
            self.list_badges_by_era(database),
        )?;

        let mut podiums_by_era = HashMap::<i64, [i64; 3]>::new();
        for result in &chart_results {
            let podium = match result.position {
                1 => 0,
                2 => 1,
                3 => 2,
                _ => continue,
            };
            podiums_by_era.entry(result.era_id).or_default()[podium] += 1;
        }

        let mut hotlaps = 0;
        let mut personal_bests = 0;
        let mut world_records = 0;
        let mut podiums = 0;
        let mut first_hotlap_at: Option<time::OffsetDateTime> = None;
        let mut latest_hotlap_at: Option<time::OffsetDateTime> = None;
        let mut eras = Vec::with_capacity(era_rows.len());

        for row in era_rows {
            let (slug, title) = era_titles.get(&row.era_id).ok_or_else(|| {
                sea_orm::DbErr::Type(format!("player activity names missing era {}", row.era_id))
            })?;
            let [firsts, seconds, thirds] = podiums_by_era.remove(&row.era_id).unwrap_or_default();
            hotlaps += row.hotlaps;
            personal_bests += row.personal_bests;
            world_records += firsts;
            podiums += firsts + seconds + thirds;
            first_hotlap_at = Some(first_hotlap_at.map_or(row.first_hotlap_at, |current| {
                current.min(row.first_hotlap_at)
            }));
            latest_hotlap_at = Some(latest_hotlap_at.map_or(row.latest_hotlap_at, |current| {
                current.max(row.latest_hotlap_at)
            }));
            eras.push(PlayerEraProfile {
                title: title.clone(),
                badges: badges_by_era.remove(&row.era_id).unwrap_or_default(),
                id: slug.clone(),
                hotlaps: row.hotlaps,
                personal_bests: row.personal_bests,
                firsts,
                seconds,
                thirds,
                first_hotlap_at: row.first_hotlap_at,
                latest_hotlap_at: row.latest_hotlap_at,
            });
        }

        Ok(PlayerProfile {
            player: self,
            stats: PlayerProfileStats {
                hotlaps,
                personal_bests,
                world_records,
                podiums,
                eras: eras.len(),
                first_hotlap_at,
                latest_hotlap_at,
            },
            eras,
            highlights: chart_results.into_iter().take(HIGHLIGHT_LIMIT).collect(),
        })
    }
}
