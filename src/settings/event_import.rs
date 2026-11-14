//! Exclusions prevent our own upstream advertisements being imported twice.
use serde::{Deserialize, Serialize};
#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct EventImportSettings {
    pub ignore_league_ids: Vec<u64>,
    pub ignore_title_regex: Option<String>,
}
impl EventImportSettings {
    pub(crate) fn title_regex(&self) -> anyhow::Result<Option<regex::Regex>> {
        self.ignore_title_regex
            .as_deref()
            .map(regex::Regex::new)
            .transpose()
            .map_err(Into::into)
    }
    pub(crate) fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.ignore_league_ids.iter().all(|id| *id > 0),
            "ignored league IDs must be positive"
        );
        self.title_regex()?;
        Ok(())
    }
}
