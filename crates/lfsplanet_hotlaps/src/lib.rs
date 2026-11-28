//! Hotlap records, catalogue workflows, validation, and rankings.

pub mod era_slug;
pub mod game_version;
pub mod hotlap_discipline;
pub mod installation_id;
pub mod models;
pub mod player_flags;
pub mod ranking_slug;
pub mod services;
pub mod settings;

#[cfg(any(test, feature = "test-support"))]
pub mod test_support;

mod relations;

#[cfg(test)]
mod tests;
