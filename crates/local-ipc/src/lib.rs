//! Provides local byte-stream and readiness adapters using Unix-domain sockets on Unix and
//! private named pipes on Windows. It does not select application endpoints, interpret
//! protocol bytes, or own caller process lifetimes.

#![deny(unsafe_code)]

#[cfg(windows)]
mod windows;
#[cfg(windows)]
#[allow(
    unsafe_code,
    reason = "reviewed Win32 name queries, pipe counters, read watches and waits are isolated here"
)]
mod windows_native;
#[cfg(windows)]
pub use windows::{
    ChildInputWriter, ChildOutputReader, ConnectAttemptError, LocalListener, LocalStream,
    Readiness, current_user_sid, endpoint_exists, endpoint_exists_before,
};

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use std::os::unix::net::{UnixListener as LocalListener, UnixStream as LocalStream};
#[cfg(unix)]
pub use unix::ChildOutputReader;

mod polling;
pub use polling::{AsLocalIo, LocalIo, PollFd, PollFlags, PollTimeout, poll};
