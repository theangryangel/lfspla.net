//! Operator CLI for adding and maintaining named LFS installations.

mod args;

mod add;
mod installation_path;
mod trim;
mod update;

pub(crate) use args::LfsCommand;

use crate::cli::Args;

/// Runs one LFS installation command.
pub(crate) async fn run(args: &Args, action: &LfsCommand) -> anyhow::Result<()> {
    match action {
        LfsCommand::Path { installation_id } => installation_path::run(args, installation_id),
        LfsCommand::Add {
            installation_id,
            download_url,
            username,
            unlock_code,
            command_timeout_seconds,
        } => {
            add::run(
                args,
                installation_id,
                download_url,
                username,
                unlock_code,
                *command_timeout_seconds,
            )
            .await
        }
        LfsCommand::Update {
            installation_id,
            command_timeout_seconds,
        } => update::run(args, installation_id, *command_timeout_seconds).await,
        LfsCommand::Trim {
            installation_id,
            dds,
            lgh,
            yes,
        } => trim::run(args, installation_id, *dds, *lgh, *yes),
    }
}
