//! Bounded stdout preparation. A document that would exceed the 8 MiB cap is
//! rejected before a single byte reaches the real stdout.

use std::io::Write;

use serde_json::Value;

pub(crate) const LIMIT: usize = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TooLarge;

struct Bounded {
    bytes: Vec<u8>,
    remaining: usize,
}

impl Bounded {
    fn new(limit: usize) -> Self {
        Self {
            bytes: Vec::new(),
            remaining: limit,
        }
    }
}

impl Write for Bounded {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        if buffer.len() > self.remaining {
            return Err(std::io::Error::new(
                std::io::ErrorKind::QuotaExceeded,
                "CLI output document exceeds the 8 MiB preparation bound",
            ));
        }
        self.bytes.extend_from_slice(buffer);
        self.remaining -= buffer.len();
        Ok(buffer.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Two-space indented JSON with a trailing newline, built entirely in memory
/// under the preparation bound. `Err(TooLarge)` means nothing was written.
pub(crate) fn encode_document(document: &Value) -> Result<Vec<u8>, TooLarge> {
    let mut bounded = Bounded::new(LIMIT);
    serde_json::to_writer_pretty(&mut bounded, document).map_err(|_| TooLarge)?;
    bounded.write_all(b"\n").map_err(|_| TooLarge)?;
    Ok(bounded.bytes)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{LIMIT, TooLarge, encode_document};

    #[test]
    fn encodes_bounded_documents_with_a_trailing_newline() {
        let encoded = encode_document(&json!({"a": 1})).expect("small document");
        assert_eq!(encoded, b"{\n  \"a\": 1\n}\n");
    }

    #[test]
    fn rejects_documents_beyond_the_bound() {
        let document = json!({"blob": "x".repeat(LIMIT)});
        assert_eq!(encode_document(&document), Err(TooLarge));
    }

    #[test]
    fn boundary_documents_succeed() {
        // Find a payload that lands exactly inside the bound.
        let base = "{\n  \"blob\": \"\"\n}\n".len();
        let inside = LIMIT - base;
        let document = json!({"blob": "x".repeat(inside)});
        let encoded = encode_document(&document).expect("document inside bound");
        assert_eq!(encoded.len(), LIMIT);
    }
}
