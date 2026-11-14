//! Bounded stdout preparation. A document that would exceed the 8 MiB cap is
//! rejected before a single byte reaches the real stdout. The same bound is
//! enforced *during* construction: renderers write into the bounded sink
//! piece by piece, so no complete body is ever allocated past the cap.

use std::fmt;
use std::io::Write;

use serde_json::Value;

mod vocabulary;
pub(crate) use vocabulary::public;

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
/// under the preparation bound and in the CLI's public words. `Err(TooLarge)`
/// means nothing was written.
pub(crate) fn encode_document(document: &Value) -> Result<Vec<u8>, TooLarge> {
    let mut bounded = Bounded::new(LIMIT);
    serde_json::to_writer_pretty(&mut bounded, &public(document)).map_err(|_| TooLarge)?;
    bounded.write_all(b"\n").map_err(|_| TooLarge)?;
    Ok(bounded.bytes)
}

/// One compact NDJSON record, completely prepared under the per-record bound.
pub(crate) fn encode_record(document: &Value) -> Result<Vec<u8>, TooLarge> {
    let mut bounded = Bounded::new(LIMIT);
    serde_json::to_writer(&mut bounded, &public(document)).map_err(|_| TooLarge)?;
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
mod tests;
