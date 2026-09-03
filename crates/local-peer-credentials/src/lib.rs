#![cfg(unix)]

use std::{io, os::unix::net::UnixStream};

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

#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_vendor = "apple",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "netbsd",
    target_os = "openbsd"
)))]
fn peer_effective_user(_stream: &UnixStream) -> io::Result<u32> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "local peer credentials are unsupported on this Unix target",
    ))
}
