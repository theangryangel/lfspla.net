//! Shared persisted records.

pub mod attachment;
pub mod badge;
pub mod player;
pub mod team;
pub mod track;
pub mod vehicle;

pub use player::Model as Player;
pub use track::{Model as Track, TrackLocation};
pub use vehicle::Model as Vehicle;
