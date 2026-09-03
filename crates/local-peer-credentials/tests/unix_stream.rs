#![cfg(unix)]

use std::os::unix::net::UnixStream;

use orchbee_local_peer_credentials::require_same_effective_user;

#[test]
fn authenticates_both_ends_of_a_same_user_socket_pair() -> std::io::Result<()> {
    let (left, right) = UnixStream::pair()?;
    require_same_effective_user(&left)?;
    require_same_effective_user(&right)?;
    Ok(())
}
