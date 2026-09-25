//! Sandboxed LFS hot-lap validation.

mod result;
mod sandbox;

pub use result::HlvcResult;
pub(crate) use sandbox::{diagnose, validate};
