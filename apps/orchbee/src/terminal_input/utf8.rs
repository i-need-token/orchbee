/// Byte provenance for framed input, independent of text admission. Rejected
/// payloads still advance this tracker so a continuation cannot open a C1
/// control sequence, and a broken scalar cannot hide a later real terminator.
/// This recognizes UTF-8 byte shape; the text owner validates Unicode scalars.
#[derive(Debug, Default)]
pub(super) struct Continuation {
    remaining: u8,
}

impl Continuation {
    pub(super) fn push(&mut self, byte: u8) -> bool {
        if self.remaining > 0 && (0x80..=0xbf).contains(&byte) {
            self.remaining -= 1;
            true
        } else {
            self.remaining = continuation_count(byte).unwrap_or(0);
            false
        }
    }
}

pub(super) fn continuation_count(lead: u8) -> Option<u8> {
    match lead {
        0xc2..=0xdf => Some(1),
        0xe0..=0xef => Some(2),
        0xf0..=0xf4 => Some(3),
        _ => None,
    }
}
