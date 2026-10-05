//! Application workflows, including their background worker implementations.

pub(crate) mod apply_eras;
pub(crate) mod deliver_webhook;
pub(crate) mod manage_lfs;
pub(crate) mod repair_eras;
pub(crate) mod storage_gc;
pub(crate) mod submit_hotlap;
pub(crate) mod sync_catalogue;
pub(crate) mod validate_hotlap;
