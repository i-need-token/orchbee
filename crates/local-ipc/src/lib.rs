//! Private same-user Windows byte transport; protocol and process owners remain callers.
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::{
    ChildInputWriter, ChildOutputReader, ConnectAttemptError, LocalListener, LocalStream,
    Readiness, current_user_sid, endpoint_exists, endpoint_exists_before,
};

#[cfg(unix)]
pub use std::os::unix::net::{UnixListener as LocalListener, UnixStream as LocalStream};

mod polling;
pub use polling::{AsLocalIo, LocalIo, PollFd, PollFlags, PollTimeout, poll};
