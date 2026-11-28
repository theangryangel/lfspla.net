//! Hotlap feature records.

pub mod badge;
pub mod chart;
pub mod era;
pub mod hotlap;
pub mod player;
pub mod ranking;
pub mod ranking_chart_membership;
pub mod site_stats;
pub mod webhook;
pub mod webhook_notification;

pub use chart::Model as Chart;
pub use era::Model as Era;
pub use hotlap::Model as Hotlap;
pub use ranking::Model as Ranking;
pub use webhook::Model as Webhook;
pub use webhook_notification::Model as WebhookNotification;
