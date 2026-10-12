//! Read-only structural admission for automatic PowerShell candidates; never loads an image.
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

pub(super) fn compatible_image(path: &Path, host: u16) -> bool {
    let Ok(mut file) = File::open(path) else {
        return false;
    };
    let Ok(metadata) = file.metadata() else {
        return false;
    };
    if !metadata.is_file() || metadata.len() < 64 {
        return false;
    }
    let mut dos = [0; 64];
    if file.read_exact(&mut dos).is_err() || &dos[..2] != b"MZ" {
        return false;
    }
    let offset = u64::from(u32::from_le_bytes(
        dos[60..64].try_into().expect("fixed header field"),
    ));
    if offset < 64 || offset + 24 > metadata.len() || file.seek(SeekFrom::Start(offset)).is_err() {
        return false;
    }
    let mut header = [0; 24];
    if file.read_exact(&mut header).is_err() || &header[..4] != b"PE\0\0" {
        return false;
    }
    let word = |index| u16::from_le_bytes([header[index], header[index + 1]]);
    let (machine, sections, optional, flags) = (word(4), word(6), word(20), word(22));
    let supported = match host {
        0xaa64 => matches!(machine, 0x014c | 0x8664 | 0xaa64),
        0x014c => machine == 0x014c,
        _ => matches!(machine, 0x014c | 0x8664),
    };
    if !supported
        || sections == 0
        || optional < 2
        || flags & 2 == 0
        || flags & 0x2000 != 0
        || offset + 24 + u64::from(optional) + u64::from(sections) * 40 > metadata.len()
    {
        return false;
    }
    let mut magic = [0; 2];
    file.read_exact(&mut magic).is_ok() && matches!(u16::from_le_bytes(magic), 0x010b | 0x020b)
}
