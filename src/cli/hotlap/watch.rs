//! Compatibility entry point for the background worker.

pub(super) async fn run(args: &crate::cli::Args) -> anyhow::Result<()> {
    crate::cli::worker::run(args).await
}
