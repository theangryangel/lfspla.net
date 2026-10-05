//! Runs the application's background record processors.

use crate::{
    cli::Args, db, services::validate_hotlap::HotlapValidation, settings::Settings, storage,
};
use lfsplanet_jobs::{Runner, WorkerConfig};
pub(crate) async fn run(args: &Args) -> anyhow::Result<()> {
    let settings = Settings::load(&args.config)?;
    let database = db::connect(&settings.database, 4).await?;
    let object_store = storage::build(&settings.storage)?;
    let config = WorkerConfig {
        workers: 1,
        poll_interval: settings.worker.hlvc.poll_interval.duration(),
    };
    let notifications = crate::services::deliver_webhook::WebhookNotifications::new(
        database.clone(),
        settings.web.public_base_url.url().clone(),
    )?;
    let webhook_config = WorkerConfig {
        workers: settings.worker.webhooks.workers.get(),
        ..WorkerConfig::default()
    };
    let processor = HotlapValidation {
        database,
        object_store,
        runtime: settings.lfs.runtime,
        installation_root: settings.lfs.installation_root.path().to_owned(),
        settings: settings.worker.hlvc,
    };
    tracing::info!("background worker started");
    #[cfg(unix)]
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    let mut signal_result = Ok(());
    Runner::new()
        .register(processor, config)?
        .register(notifications, webhook_config)?
        .run(async {
            #[cfg(unix)]
            tokio::select! {
                result = tokio::signal::ctrl_c() => signal_result = result,
                _ = terminate.recv() => {}
            }
            #[cfg(not(unix))]
            {
                signal_result = tokio::signal::ctrl_c().await;
            }
        })
        .await?;
    signal_result?;
    tracing::info!("background worker stopped");
    Ok(())
}
