//! Resolve one named LFS installation to its configured filesystem path.

use crate::{cli::Args, lfs::installations::installation_path, settings::Settings};

pub(super) fn run(args: &Args, installation_id: &str) -> anyhow::Result<()> {
    let settings = Settings::load(&args.config)?;
    let path = installation_path(
        settings.lfs_runtime.installation_root.path(),
        installation_id,
    )?;
    println!("{}", path.display());
    Ok(())
}
