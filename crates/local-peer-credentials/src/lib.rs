//! Kernel-backed same-user proof for an already connected local stream. The caller retains the
//! stream and owns endpoint selection, application-role admission and all protocol I/O; this
//! adapter neither reads application bytes nor retains peer authority.

#[cfg(windows)]
use orchbee_local_ipc::LocalStream as UnixStream;
use std::io;
#[cfg(unix)]
use std::os::unix::net::UnixStream;

/// Requires the connected peer's kernel-reported effective user to match this process.
///
/// # Errors
/// Returns `PermissionDenied` for another user, `Unsupported` on unsupported Unix targets,
/// or the operating-system error if the peer identity cannot be obtained.
#[cfg(unix)]
pub fn require_same_effective_user(stream: &UnixStream) -> io::Result<()> {
    let current_user = nix::unistd::geteuid();
    let peer_user = peer_effective_user(stream)?;
    if peer_user == current_user.as_raw() {
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::PermissionDenied,
        "local peer has a different effective user",
    ))
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn peer_effective_user(stream: &UnixStream) -> io::Result<u32> {
    let credentials =
        nix::sys::socket::getsockopt(stream, nix::sys::socket::sockopt::PeerCredentials)
            .map_err(io::Error::from)?;
    Ok(credentials.uid())
}

#[cfg(any(
    target_vendor = "apple",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "netbsd",
    target_os = "openbsd"
))]
fn peer_effective_user(stream: &UnixStream) -> io::Result<u32> {
    let (user, _group) = nix::unistd::getpeereid(stream).map_err(io::Error::from)?;
    Ok(user.as_raw())
}

#[cfg(all(
    unix,
    not(any(
        target_os = "linux",
        target_os = "android",
        target_vendor = "apple",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "netbsd",
        target_os = "openbsd"
    ))
))]
fn peer_effective_user(_stream: &UnixStream) -> io::Result<u32> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "local peer credentials are unsupported on this Unix target",
    ))
}

/// Requires the named-pipe peer's kernel process-token user to match this process's user.
///
/// # Errors
/// Returns the transport adapter's identity-query or same-user admission failure.
#[cfg(windows)]
pub fn require_same_effective_user(stream: &UnixStream) -> io::Result<()> {
    stream.require_same_user()
}
