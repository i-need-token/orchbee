//! The first bytes a service process writes on the connection to the Server: the magic `OBWF`, a
//! big-endian `u16` version and the sixteen-byte ticket of the request that started it. The Server
//! admits the connection only for a ticket it is waiting for, from a peer of its own user; the
//! service's own lines follow at once. Nothing answers the hello: a refused connection is closed.

use std::io::{self, Read};

/// The hello's length in bytes.
pub const HELLO_BYTES: usize = 22;
const MAGIC: [u8; 4] = *b"OBWF";
/// Server and service come from one installation, so there is one version.
const VERSION: u16 = 1;

/// The hello a service started for `ticket` writes.
#[must_use]
pub fn hello(ticket: [u8; 16]) -> [u8; HELLO_BYTES] {
    let mut bytes = [0; HELLO_BYTES];
    bytes[..4].copy_from_slice(&MAGIC);
    bytes[4..6].copy_from_slice(&VERSION.to_be_bytes());
    bytes[6..].copy_from_slice(&ticket);
    bytes
}

/// Reads a hello from `input` and returns its ticket.
///
/// # Errors
///
/// Returns the read's error, or [`io::ErrorKind::InvalidData`] for another magic or version.
pub fn read_hello(input: &mut impl Read) -> io::Result<[u8; 16]> {
    let mut bytes = [0; HELLO_BYTES];
    input.read_exact(&mut bytes)?;
    if bytes[..4] != MAGIC || bytes[4..6] != VERSION.to_be_bytes() {
        return Err(io::ErrorKind::InvalidData.into());
    }
    let mut ticket = [0; 16];
    ticket.copy_from_slice(&bytes[6..]);
    Ok(ticket)
}
