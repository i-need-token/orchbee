//! Owned named-pipe addresses for native transport contract tests.
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
pub(crate) fn endpoint() -> PathBuf {
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    format!(
        r"\\.\pipe\orchbee-test-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    )
    .into()
}
