//! Obtains a fresh nonzero 16-byte identity from the operating-system random source and
//! reports source failures, and reads one written as hexadecimal on a command line. It neither
//! registers that identity nor establishes uniqueness across retained operation state.

use std::io::{self};

pub(crate) fn fresh_identity() -> io::Result<[u8; 16]> {
    let mut bytes = [0; 16];
    getrandom::fill(&mut bytes).map_err(|error| std::io::Error::other(error.to_string()))?;
    if bytes == [0; 16] {
        return Err(io::Error::other(
            "Worker identity source returned a zero identity",
        ));
    }
    Ok(bytes)
}

/// Sixteen bytes as 32 lowercase hexadecimal digits, not all zero: a Worker id, a restart's ticket
/// or a Workspace files service's ticket.
pub(crate) fn parse_hex_identity(text: &std::ffi::OsStr) -> Option<[u8; 16]> {
    let text = text.to_str()?.as_bytes();
    if text.len() != 32 {
        return None;
    }
    let digit = |byte: u8| match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    };
    let mut bytes = [0_u8; 16];
    for (byte, pair) in bytes.iter_mut().zip(text.chunks_exact(2)) {
        *byte = (digit(pair[0])? << 4) | digit(pair[1])?;
    }
    bytes.iter().any(|byte| *byte != 0).then_some(bytes)
}
