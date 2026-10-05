//! Persisted application concepts and their behavior, grouped by feature.
//!
//! HTTP and CLI concerns remain in their respective entrypoint modules.

pub(crate) mod badge;
pub(crate) mod era;
pub(crate) mod hotlap;
pub(crate) mod personal_access_token;
pub(crate) mod player;
pub(crate) mod ranking;
pub(crate) mod site_stats;
pub(crate) mod track;
pub(crate) mod vehicle;

pub(crate) mod webhook;

pub(crate) mod chart;
pub(crate) mod ranking_chart_membership;
pub(crate) mod webhook_notification;

pub(crate) use chart::Model as Chart;
pub(crate) use era::Model as Era;
pub(crate) use hotlap::Model as Hotlap;
pub(crate) use personal_access_token::Model as PersonalAccessToken;
pub(crate) use player::Model as Player;
pub(crate) use ranking::Model as Ranking;
pub(crate) use track::Model as Track;
pub(crate) use vehicle::Model as Vehicle;
pub(crate) use webhook::Model as Webhook;
pub(crate) use webhook_notification::Model as WebhookNotification;
