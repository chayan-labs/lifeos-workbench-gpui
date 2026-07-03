//! ID + time helpers.
//!
//! The schema documents `id` as a ULID. We keep a short human-readable prefix
//! (`ent_`, `evt_`, ...) in front of the ULID so rows are debuggable at a glance;
//! the ULID body stays lexicographically sortable by creation time.

use std::sync::Mutex;
use ulid::{Generator, Ulid};

/// `Ulid::new()` fills the sub-millisecond bits with fresh randomness on every
/// call, so two ids minted in the same millisecond are NOT guaranteed to sort
/// in call order - only in *most* cases. reconcile.rs relies on `ORDER BY id
/// ASC` being true causal order (docs/DATA-MODEL.md §4.2); a same-millisecond
/// tie there silently replays events out of order. A shared monotonic
/// `Generator` closes that gap: within one millisecond it increments the
/// random tail instead of re-rolling it, so ids it produces are always
/// strictly increasing regardless of call rate.
static GENERATOR: Mutex<Generator> = Mutex::new(Generator::new());

pub fn new_id(prefix: &str) -> String {
    let ulid = GENERATOR
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .generate()
        .unwrap_or_else(|_| Ulid::new());
    format!("{prefix}_{ulid}")
}

/// Unix epoch seconds. The whole schema stores integer seconds in `*_at`/`ts`.
pub fn now_secs() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
