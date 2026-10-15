//! Explicit metadata-only definitions for database test setup.

use super::{tracks::TrackDefinition, vehicles::VehicleDefinition};
use sea_orm::{DatabaseConnection, TransactionTrait};
use std::path::Path;

pub(crate) async fn tracks(database: &DatabaseConnection) -> anyhow::Result<()> {
    let transaction = database.begin().await?;
    for track in insim_core::track::Track::ALL {
        TrackDefinition {
            id: *track,
            outline: None,
            image: None,
        }
        .apply(
            &transaction,
            None,
            false,
            Path::new("fixture"),
            crate::models::track::location::canonical_code(*track),
        )
        .await?;
    }
    transaction.commit().await?;
    Ok(())
}

pub(crate) async fn vehicles(database: &DatabaseConnection) -> anyhow::Result<()> {
    let transaction = database.begin().await?;
    for (vehicle, _) in crate::models::vehicle::STANDARD_VEHICLES {
        VehicleDefinition {
            id: *vehicle,
            image: None,
        }
        .apply(&transaction, None, false, Path::new("fixture"))
        .await?;
    }
    transaction.commit().await?;
    Ok(())
}
