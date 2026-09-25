//! Generate a basic application configuration.

use std::io::Write;

use anyhow::Context;

use crate::settings::Settings;

/// Prints a valid configuration populated with development defaults.
pub(crate) fn run() -> anyhow::Result<()> {
    let mut options = serde_saphyr::SerializerOptions::default();
    options.prefer_block_scalars = false;
    let yaml = serde_saphyr::to_string_with_options(&Settings::default(), options)
        .context("failed to serialize default settings as YAML")?;
    std::io::stdout()
        .write_all(yaml.as_bytes())
        .context("failed to write default settings to standard output")?;
    Ok(())
}
