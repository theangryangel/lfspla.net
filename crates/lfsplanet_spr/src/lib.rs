//! Parser for the documented 192-byte header of Live for Speed single-player
//! replay (`.spr`) files.
//!
//! The undocumented replay stream following the header is not read.

mod error;
mod filename;
mod header;
mod spr_vehicle;

pub use error::Error;
pub use filename::{ConventionalFilename, ConventionalFilenameError};
pub use header::{FileVersion, HEADER_LEN, HotlapMode, SkillLevel, SprHeader};
pub use insim::insim::{PlayerFlags, RaceLaps};
pub use spr_vehicle::SprVehicle;
