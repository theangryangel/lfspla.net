//! Runs the application's background record processors.

use crate::{cli::Args, jobs::hlvc::HotlapValidation, settings::Settings, startup, storage};
use lfsplanet_jobs::{Runner, WorkerConfig};

pub(crate) async fn run(args: &Args) -> anyhow::Result<()> {
    let settings = Settings::load(&args.config)?;
    let database = startup::connect(&settings.database, 4).await?;
    let object_store = storage::build(&settings.storage)?;
    let config = WorkerConfig {
        workers: 1,
        poll_interval: settings.worker.hlvc.poll_interval.duration(),
    };
    let notifications = crate::jobs::webhooks::WebhookNotifications::new(
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
        runtime: settings.lfs_runtime,
        settings: settings.worker.hlvc,
    };
    tracing::info!("background worker started");
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    let mut signal_result = Ok(());
    Runner::new()
        .register(processor, config)?
        .register(notifications, webhook_config)?
        .run(async {
            tokio::select! {
                result = tokio::signal::ctrl_c() => signal_result = result,
                _ = terminate.recv() => {}
            }
        })
        .await?;
    signal_result?;
    tracing::info!("background worker stopped");
    Ok(())
}
