//! Application models, business rules, and database operations, grouped by feature.
//!
//! HTTP and CLI concerns remain in their respective entrypoint modules.

pub(crate) mod badge;
pub(crate) mod country;
mod ordering;
pub use ordering::Ordering;
pub(crate) mod era;
pub(crate) mod hotlap;
pub(crate) mod personal_access_token;
pub(crate) mod player;
pub(crate) mod ranking;
pub(crate) mod site_stats;
pub(crate) mod track;
pub(crate) mod vehicle;

pub(crate) mod webhook;

pub(crate) mod ranking_chart;
pub(crate) mod webhook_notification;

pub(crate) mod leaderboard;
pub(crate) mod personal_best;

pub(crate) use era::Model as Era;
pub(crate) use hotlap::Model as Hotlap;
pub(crate) use personal_access_token::Model as PersonalAccessToken;
pub(crate) use player::Model as Player;
pub(crate) use ranking::Model as Ranking;
pub(crate) use ranking_chart::Model as RankingChart;
pub(crate) use track::Model as Track;
pub(crate) use vehicle::Model as Vehicle;
pub(crate) use webhook::Model as Webhook;
pub(crate) use webhook_notification::Model as WebhookNotification;
