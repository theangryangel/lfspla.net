//! Sandboxed LFS hot-lap validation.

#![cfg(target_os = "linux")]

mod result;
mod sandbox;

pub use result::HlvcResult;
pub(crate) use sandbox::{diagnose, validate};
