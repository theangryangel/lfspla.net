mod generate;
mod persist;
#[cfg(test)]
mod tests;

use crate::{cli::Args, db, settings::Settings};
use anyhow::ensure;
use clap::Args as ClapArgs;
#[derive(Clone, Debug, Eq, PartialEq, ClapArgs)]
pub(crate) struct DemoArgs {
    /// Confirm database writes.
    #[arg(long, required = true)]
    pub yes: bool,
    /// Number of synthetic player profiles to generate this run.
    #[arg(long, default_value_t = 100, value_parser = clap::value_parser!(u32).range(1..=100_000))]
    pub players: u32,
    /// Percent of demo combinations to include.
    #[arg(long, default_value_t = 90, value_parser = clap::value_parser!(u8).range(1..=100))]
    pub coverage: u8,
    /// Seed for repeatable generation; randomly chosen when omitted.
    #[arg(long)]
    pub seed: Option<u64>,
}

pub(crate) async fn run(args: &Args, options: &DemoArgs) -> anyhow::Result<()> {
    ensure!(options.yes, "generating demo data requires --yes");
    let settings = Settings::load(&args.config)?;
    eprintln!("Connecting to the database for demo generation...");
    let database = db::connect(&settings.database, 2).await?;
    let count = persist::populate(&database, options).await?;
    println!(
        "Generated {} demo player profiles and inserted {count} new demo hotlaps.",
        options.players
    );
    Ok(())
}
