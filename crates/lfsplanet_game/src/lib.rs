#![allow(missing_docs, missing_debug_implementations)]
//! Shared game-runtime plumbing for lfspla.net's mini-game crates
//! (`lfsplanet_bomb`, `lfsplanet_shortcut`, `lfsplanet_metronome`): kitcar-driven HUD components, common
//! session setup, and common InSim connection args.
//! Depends only on `insim`/`kitcar`/`clap`/`tokio` — never on
//! anything DB- or web-related, so any game crate depending on this stays
//! persistence-agnostic.

mod args;
mod components;
mod layout_store;
mod leaderboard;
mod session_setup;
mod vehicle_restrictions;

pub use args::{InSimArgs, LayoutArgs, LayoutArgsError};
pub use components::{
    Dialog, DialogMsg, DialogProps, Marquee, MarqueeProps, hud_active, hud_muted, hud_text,
    hud_title, topbar,
};
pub use layout_store::{Storage, StorageConfig};
pub use leaderboard::{
    InMemoryLeaderboardStore, Leaderboard, LeaderboardError, LeaderboardResult, LeaderboardStore,
};
pub use session_setup::{GamePhase, LayoutSpec, SessionSetup, SetupReady, SetupStarted};
pub use vehicle_restrictions::{VehicleRestrictions, VehicleRule};
