use std::{
    fs::File,
    io::{self, Read},
};

pub(crate) fn fresh_identity() -> io::Result<[u8; 16]> {
    let mut bytes = [0; 16];
    File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    if bytes == [0; 16] {
        return Err(io::Error::other(
            "Worker identity source returned a zero identity",
        ));
    }
    Ok(bytes)
}
