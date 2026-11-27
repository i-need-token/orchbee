//! The relay's address on this operating system: a named pipe with an unguessable name on
//! Windows, and elsewhere a socket in a folder only this user can enter, removed with it. The name
//! only keeps the endpoint out of sight; admission still proves each helper's user.

use orchbee_local_ipc::{LocalListener, LocalStream};
use std::{
    io,
    path::{Path, PathBuf},
};

/// A fresh address, removed when dropped.
pub(super) struct Endpoint {
    path: PathBuf,
    /// The private folder that holds the socket.
    #[cfg(unix)]
    folder: PathBuf,
}

impl Endpoint {
    /// An address named by `suffix`, unique to this process.
    #[cfg(windows)]
    pub(super) fn create(suffix: &str) -> io::Result<Self> {
        Ok(Self {
            path: PathBuf::from(format!(
                r"\\.\pipe\orchbee-files-relay-{}-{suffix}",
                std::process::id()
            )),
        })
    }

    /// An address named by `suffix`, in a folder only this user can enter.
    #[cfg(unix)]
    pub(super) fn create(suffix: &str) -> io::Result<Self> {
        use std::os::unix::fs::DirBuilderExt;
        let folder = std::env::temp_dir().join(format!("orchbee-relay-{suffix}"));
        std::fs::DirBuilder::new().mode(0o700).create(&folder)?;
        Ok(Self {
            path: folder.join("relay"),
            folder,
        })
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    /// Listens at the address; the listener's accept answers at once when no helper waits.
    pub(super) fn bind(&self) -> io::Result<LocalListener> {
        let listener = LocalListener::bind(&self.path)?;
        listener.set_nonblocking(true)?;
        Ok(listener)
    }
}

#[cfg(unix)]
impl Drop for Endpoint {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        let _ = std::fs::remove_dir(&self.folder);
    }
}

/// Makes an accepted helper's stream block: a socket accepted on macOS keeps the listener's
/// nonblocking mode.
#[cfg(unix)]
pub(super) fn blocking(stream: &LocalStream) -> io::Result<()> {
    stream.set_nonblocking(false)
}

/// A pipe accepted on Windows is admitted in blocking mode already.
#[cfg(windows)]
pub(super) const fn blocking(_: &LocalStream) -> io::Result<()> {
    Ok(())
}
