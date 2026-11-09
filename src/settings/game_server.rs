//! One application-managed LFS connection.
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, num::NonZeroU64};
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GameServerSettings {
    pub insim_address: SocketAddr,
    pub admin_password: Option<String>,
    #[serde(default = "default_interval")]
    pub poll_interval_seconds: NonZeroU64,
}
fn default_interval() -> NonZeroU64 {
    NonZeroU64::new(2).unwrap()
}
