//! Versioned Pubstat response types and Serde plain-text serialization.
//!
//! No HTTP or persistence dependencies. Version-aware Serde implementations
//! select fields in protocol order before the chosen output format sees them.
pub mod ch;
pub mod hl;
pub mod ser;
pub mod track;
mod version;
pub mod wr;
use insim_core::{track::Track, vehicle::Vehicle};
pub use version::{ParseVersionError, Version};

/// Complete data for a published hotlap, before selecting a wire layout.
/// Splits are cumulative checkpoints; finish values are excluded and unused slots are zero.
#[derive(Debug, Clone)]
pub struct Lap {
    pub id: i64,
    pub track: Track,
    pub car: Vehicle,
    pub split1: i64,
    pub split2: i64,
    pub split3: i64,
    pub laptime: i64,
    pub flags_hlaps: u16,
    pub racername: String,
    pub timestamp: i64,
}
