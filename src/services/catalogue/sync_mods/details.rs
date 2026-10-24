//! Cache full API responses once per successfully observed mod revision.
use crate::models::vehicle::{Column, Entity};
use futures::stream::{self, StreamExt};
use lfsplanet_lfs_api::{LfsClient, RemoteVehicleMod, VehicleModMetadata};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, sea_query::Expr};
use std::collections::HashMap;

pub(super) async fn cache_details(
    database: &DatabaseConnection,
    client: &LfsClient,
    remote: &[RemoteVehicleMod],
) -> Result<usize, sea_orm::DbErr> {
    let cached = Entity::find()
        .filter(Column::Kind.eq("mod"))
        .all(database)
        .await?
        .into_iter()
        .map(|row| (row.id.to_string(), row.metadata))
        .collect::<HashMap<_, _>>();
    let pending = remote.iter().filter(|entry| {
        !cached
            .get(&entry.id)
            .and_then(Option::as_ref)
            .and_then(|value| serde_json::from_value::<VehicleModMetadata>(value.clone()).ok())
            .is_some_and(|details| {
                details.version == entry.version
                    && details.id.eq_ignore_ascii_case(&entry.id)
                    && details.vehicle.is_some()
            })
    });
    let count = stream::iter(pending).map(|entry| async move {
        let result = async {
            let details = client.vehicle_mod(&entry.id).await?;
            anyhow::ensure!(details.version == entry.version, "mod revision changed during synchronization");
            // A concurrent newer catalogue refresh must not receive older details.
            let updated = Entity::update_many()
                .filter(Column::Id.eq(&entry.id))
                .filter(Column::Version.eq(i16::try_from(entry.version)?))
                .filter(Column::Available.eq(true))
                .col_expr(Column::Metadata, Expr::value(details.metadata))
                .exec(database).await?;
            Ok::<_, anyhow::Error>(updated.rows_affected == 1)
        }.await;
        match result {
            Ok(updated) => usize::from(updated),
            Err(error) => {
                tracing::warn!(id = %entry.id, ?error, "Vehicle Mod detail fetch failed; will retry next sync");
                0
            }
        }
    }).buffer_unordered(4).fold(0, |total, count| async move { total + count }).await;
    Ok(count)
}
