//! Obtains a fresh nonzero 16-byte identity from the operating-system random source and
//! reports source failures. It neither registers that identity nor establishes uniqueness
//! across retained operation state.

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
