//! Fixtures the file engine's suites share, and the file edits' suites with them.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

/// A private directory for one test, removed when the test ends.
pub(crate) struct Scratch(PathBuf);

impl Scratch {
    pub(crate) fn new(purpose: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        // macOS reaches its temporary folder through a link, which FSEvents resolves in the
        // paths it reports.
        #[cfg(target_os = "macos")]
        let base = PathBuf::from("/private/tmp");
        #[cfg(not(target_os = "macos"))]
        let base = std::env::temp_dir();
        let path = base.join(format!(
            "orchbee-files-{purpose}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        // Best effort: a test that failed may leave a handle open on Windows, and a leftover
        // directory under the system's temporary folder harms nothing.
        let _ = fs::remove_dir_all(&self.0);
    }
}
