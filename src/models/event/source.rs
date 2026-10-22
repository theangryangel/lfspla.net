//! Provenance shared by leagues, seasons and advertised events.

use sea_orm::{DeriveActiveEnum, EnumIter};
use serde::{Deserialize, Serialize};

/// Stored as ordinary text; adding a source does not require a PostgreSQL enum.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize,
)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
pub enum Source {
    #[default]
    #[sea_orm(string_value = "local")]
    Local,
    #[sea_orm(string_value = "lfs_net")]
    LfsNet,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Change {
    Created,
    Updated,
    Unchanged,
}

#[cfg(test)]
mod tests {
    use super::Source;

    #[test]
    fn event_sources_have_stable_text_values() {
        use sea_orm::ActiveEnum;
        for (source, value) in [(Source::Local, "local"), (Source::LfsNet, "lfs_net")] {
            assert_eq!(source.to_value(), value);
            assert_eq!(serde_json::to_value(source).unwrap(), value);
            assert_eq!(
                serde_json::from_value::<Source>(value.into()).unwrap(),
                source
            );
        }
        assert_eq!(Source::default(), Source::Local);
        assert!(serde_json::from_str::<Source>("\"unknown\"").is_err());
    }
}
