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
