//! The shared wire corpus stores complete, contract-authored records as base64.
use std::io;

pub fn base64(value: &str) -> io::Result<Vec<u8>> {
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut bits = 0_u32;
    let mut count = 0;
    let mut out = Vec::new();
    for byte in value.bytes().take_while(|b| *b != b'=') {
        let value = alphabet
            .iter()
            .position(|x| *x == byte)
            .ok_or_else(|| io::Error::other("invalid fixture base64"))? as u32;
        bits = (bits << 6) | value;
        count += 6;
        if count >= 8 {
            count -= 8;
            out.push((bits >> count) as u8);
            bits &= (1 << count) - 1;
        }
    }
    Ok(out)
}
