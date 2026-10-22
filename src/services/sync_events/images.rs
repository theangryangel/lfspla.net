//! Optional league logos: download outside transactions and retain cached images on failure.

use object_store::ObjectStore;
use sea_orm::DatabaseConnection;

use super::{League, SyncReport};
use crate::models::{event::source::Change, league};
use lfsplanet_lfs_leagues::LogoClient;

pub(super) async fn sync(
    database: &DatabaseConnection,
    store: Option<&dyn ObjectStore>,
    sources: &[League],
    report: &mut SyncReport,
) {
    let mut fetch = match LogoClient::new() {
        Ok(fetch) => fetch,
        Err(error) => {
            report
                .warnings
                .push(format!("league logo client: {error:#}"));
            return;
        }
    };
    for applied in &report.leagues {
        let Some(source) = sources
            .iter()
            .find(|source| source.external_id.to_string() == applied.external_id)
        else {
            continue;
        };
        let Some(url) = &source.logo_url else {
            continue;
        };
        let result = async {
            let source = fetch.logo(url).await?;
            let logo = league::Logo {
                bytes: source.bytes,
                extension: source.extension,
                media_type: source.media_type,
            };
            // Rolled-back insert IDs from dry runs do not identify stored rows.
            if report.dry_run && matches!(applied.change, Change::Created) {
                return Ok(true);
            }
            league::Model::cache_logo(
                database,
                applied.local_id,
                &logo,
                store,
                time::OffsetDateTime::now_utc(),
            )
            .await
        }
        .await;
        match result {
            Ok(changed) => report.images_updated += usize::from(changed),
            Err(error) => report
                .warnings
                .push(format!("league {} logo: {error:#}", applied.external_id)),
        }
    }
}
