//! Readiness is advisory: callers retain their absolute deadline and handle
//! WouldBlock after every wake. Windows write wakes are cadence limited.
#[cfg(windows)]
mod windows;
#[cfg(unix)]
pub use nix::poll::{PollFd, PollFlags, PollTimeout};
#[cfg(windows)]
pub use windows::{LocalIo, PollFd, PollFlags, PollTimeout, poll};
#[cfg(unix)]
pub fn poll(sources: &mut [PollFd<'_>], timeout: PollTimeout) -> std::io::Result<usize> {
    nix::poll::poll(sources, timeout)
        .map(|count| count as usize)
        .map_err(Into::into)
}
#[cfg(unix)]
pub type LocalIo<'a> = std::os::fd::BorrowedFd<'a>;

/// Borrows a transport for readiness observation without transferring ownership.
pub trait AsLocalIo {
    fn as_local_io(&self) -> LocalIo<'_>;
}

#[cfg(unix)]
impl<T: std::os::fd::AsFd> AsLocalIo for T {
    fn as_local_io(&self) -> LocalIo<'_> {
        self.as_fd()
    }
}

#[cfg(windows)]
impl AsLocalIo for crate::LocalStream {
    fn as_local_io(&self) -> LocalIo<'_> {
        LocalIo::Stream(self)
    }
}

#[cfg(windows)]
impl AsLocalIo for crate::LocalListener {
    fn as_local_io(&self) -> LocalIo<'_> {
        LocalIo::Listener(self)
    }
}
