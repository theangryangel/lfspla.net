//! Fixed staff roles, stored as their serialized names.
use clap::ValueEnum;
use sea_orm::{DeriveActiveEnum, EnumIter, sea_query::StringLen};
use serde::{Deserialize, Serialize};

#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    EnumIter,
    DeriveActiveEnum,
    Serialize,
    Deserialize,
    ValueEnum,
    utoipa::ToSchema,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(16))")]
#[serde(rename_all = "snake_case")]
pub enum Role {
    #[default]
    #[sea_orm(string_value = "player")]
    Player,
    #[sea_orm(string_value = "moderator")]
    Moderator,
}

impl std::fmt::Display for Role {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Player => "player",
            Self::Moderator => "moderator",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::ActiveEnum;

    #[test]
    fn storage_serde_and_cli_share_names_and_reject_unknown_roles() {
        for role in [Role::Player, Role::Moderator] {
            let name = role.to_string();
            assert_eq!(role.to_value(), name);
            assert_eq!(Role::try_from_value(&name).unwrap(), role);
            assert_eq!(serde_json::to_value(role).unwrap(), name);
            assert_eq!(
                serde_json::from_value::<Role>(name.clone().into()).unwrap(),
                role
            );
            assert_eq!(<Role as ValueEnum>::from_str(&name, false).unwrap(), role);
        }
        assert!(Role::try_from_value(&"admin".to_owned()).is_err());
        assert!(serde_json::from_str::<Role>("\"admin\"").is_err());
        assert_eq!(Role::default(), Role::Player);
    }
}
