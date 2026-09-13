//! Bounded stdout preparation. A document that would exceed the 8 MiB cap is
//! rejected before a single byte reaches the real stdout. The same bound is
//! enforced *during* construction: renderers write into the bounded sink
//! piece by piece, so no complete body is ever allocated past the cap.

use std::fmt;
use std::io::Write;

use serde_json::Value;

pub(crate) const LIMIT: usize = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TooLarge;

/// The shared preparation sink: every document byte passes through `push`
/// (or the `io::Write`/`fmt::Write` adapters over it) and the append fails
/// before the buffer grows past the bound.
pub(crate) struct Bounded {
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

    /// Append one piece; `Err(TooLarge)` appends nothing.
    pub(crate) fn push(&mut self, text: &str) -> Result<(), TooLarge> {
        if text.len() > self.remaining {
            return Err(TooLarge);
        }
        self.bytes.extend_from_slice(text.as_bytes());
        self.remaining -= text.len();
        Ok(())
    }

    /// Formatted output appended piecewise under the bound.
    pub(crate) fn write(&mut self, arguments: fmt::Arguments<'_>) -> Result<(), TooLarge> {
        fmt::Write::write_fmt(self, arguments).map_err(|_| TooLarge)
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
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

impl fmt::Write for Bounded {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.push(text).map_err(|_| fmt::Error)
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

/// Run a text renderer against the bounded sink: the bound is enforced while
/// the document is being built, so preparation aborts at the cap instead of
/// measuring an already complete oversized body. `Err(TooLarge)` means no
/// result document was produced.
pub(crate) fn render_text(
    render: impl FnOnce(&mut Bounded) -> Result<(), TooLarge>,
) -> Result<Vec<u8>, TooLarge> {
    let mut bounded = Bounded::new(LIMIT);
    render(&mut bounded)?;
    Ok(bounded.finish())
}

/// Bounded finalization for fixed text bodies (help, version, completion
/// scripts): the caller supplies a complete, finite body and the bound is
/// checked before the first stdout write.
pub(crate) fn encode_text(body: &str) -> Result<Vec<u8>, TooLarge> {
    if body.len() > LIMIT {
        return Err(TooLarge);
    }
    Ok(body.as_bytes().to_vec())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{Bounded, LIMIT, TooLarge, encode_document, encode_text, render_text};

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

    #[test]
    fn text_at_the_bound_succeeds() {
        let body = "x".repeat(LIMIT);
        let encoded = encode_text(&body).expect("text at the bound");
        assert_eq!(encoded.len(), LIMIT);
    }

    #[test]
    fn text_beyond_the_bound_is_rejected_before_any_bytes() {
        let body = "x".repeat(LIMIT + 1);
        assert_eq!(encode_text(&body), Err(TooLarge));
    }

    #[test]
    fn renderer_aborts_at_the_bound_during_construction() {
        // The sink never holds more than the bound, and a renderer that keeps
        // writing fails on the piece that would cross it — the oversized body
        // is never assembled.
        let mut bounded = Bounded::new(LIMIT);
        bounded.push(&"x".repeat(LIMIT - 8)).expect("under bound");
        assert_eq!(bounded.push(&"y".repeat(9)), Err(TooLarge));
        // A failed push appends nothing: the remaining capacity is intact.
        bounded
            .push(&"z".repeat(8))
            .expect("remaining capacity usable");
        assert_eq!(bounded.push("!"), Err(TooLarge), "no bytes fit past it");
        assert_eq!(bounded.finish().len(), LIMIT);

        let rendered = render_text(|out| {
            out.push(&"x".repeat(LIMIT))?;
            out.push("!")
        });
        assert_eq!(rendered, Err(TooLarge));
    }

    #[test]
    fn renderer_at_exactly_the_bound_succeeds() {
        let rendered =
            render_text(|out| out.push(&"x".repeat(LIMIT))).expect("exactly at the bound");
        assert_eq!(rendered.len(), LIMIT);
    }
}
