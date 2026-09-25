//! Validation queries over era replay-version coverage.

use anyhow::ensure;
use sea_orm::{ConnectionTrait, EntityTrait, QueryOrder};

use super::{EraColumn, EraEntity};

/// Requires at least one era and rejects overlapping version ranges.
pub(crate) async fn validate_version_requirements<C>(database: &C) -> anyhow::Result<()>
where
    C: ConnectionTrait,
{
    let eras = EraEntity::find()
        .order_by_asc(EraColumn::Id)
        .all(database)
        .await?;
    ensure!(!eras.is_empty(), "at least one era must be configured");
    for (index, left) in eras.iter().enumerate() {
        for right in &eras[index + 1..] {
            ensure!(
                !left
                    .version_requirement
                    .intersects(&right.version_requirement),
                "version requirements overlap for eras {} and {}",
                left.id,
                right.id
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use insim_core::game_version::GameVersion;

    use crate::models::eras::EraModel;

    fn model(version_requirement: &str) -> EraModel {
        EraModel {
            id: 1,
            slug: "2007-12-21".to_owned(),
            title: "2007 physics".to_owned(),
            installation_id: "0.7".to_owned(),
            version_requirement: version_requirement
                .parse()
                .expect("the test version requirement is valid"),
            open: true,
        }
    }

    #[test]
    fn an_era_row_covers_only_its_own_replay_versions() {
        let era = model(">=0.5Y,<0.8");
        assert!(era.accepts_replay_version(&"0.7D41".parse::<GameVersion>().unwrap()));
        assert!(!era.accepts_replay_version(&"0.8A".parse::<GameVersion>().unwrap()));
    }
}
