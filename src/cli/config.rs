//! Generate a basic application configuration.

use std::io::Write;

use anyhow::Context;
use clap::{ArgGroup, Args};

use crate::settings::{PublicBaseUrl, Settings};
use lfsplanet_lfs_net_api::OAuthCredentials;

/// Selects the defaults used in a generated configuration.
#[derive(Clone, Debug, Eq, PartialEq, Args)]
#[command(group(ArgGroup::new("mode").required(true).multiple(false).args(["production", "development"])))]
pub struct GenerateConfigArgs {
    /// Use production defaults; edit the URLs for your deployment.
    #[arg(long)]
    pub production: bool,
    /// Use local development defaults, including validation bypass.
    #[arg(long = "no-production", visible_alias = "development")]
    pub development: bool,
}

/// Prints a configuration using the selected defaults.
pub(crate) fn run(args: &GenerateConfigArgs) -> anyhow::Result<()> {
    let mut settings = Settings::default();
    if !args.production {
        // Keep OAuth structurally configured while making setup's required edit explicit.
        settings.lfs_net.oauth = Some(OAuthCredentials::new("REPLACE_ME", "REPLACE_ME"));
        // Development services run on the host; PostgreSQL is published by Compose.
        // Browsers reach both the frontend and auth routes through Vite over HTTP.
        settings.web.public_base_url = PublicBaseUrl::try_from(
            url::Url::parse("http://localhost:5173/")
                .context("failed to parse the development public URL")?,
        )
        .context("invalid development public URL")?;
        settings.web.cookie_secure = false;
        settings.hotlaps.allow_test_validation = true;
    }
    let mut options = serde_saphyr::SerializerOptions::default();
    options.prefer_block_scalars = false;
    let yaml = serde_saphyr::to_string_with_options(&settings, options)
        .context("failed to serialize default settings as YAML")?;
    std::io::stdout()
        .write_all(yaml.as_bytes())
        .context("failed to write default settings to standard output")?;
    Ok(())
}
