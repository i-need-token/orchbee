//! Acquires one declared send-input source under its exact byte bound, deadline, and signal
//! watch, preserving typed source failures and provenance. It does not submit input or
//! reinterpret acquired bytes as executable syntax.

use super::{SignalWatch, acquire_file, acquire_stdin, limit_for};
use crate::cli::input_source::usage;
pub(crate) use crate::cli::input_source::{InputSource, SourceFailure, source_details};
use crate::invocation::{SendSourceCause, TerminalSendInput};

use std::{borrow::Cow, time::Instant};
/// Materialize the parsed input declaration into the records the send path
/// submits. `Text`/`Keys` pass through untouched; `File`/`Stdin` are
/// acquired once, bounded to the declared form, and become the literal
/// `Bytes` record — or the opaque `Paste` content under `--paste`. The
/// bytes are verbatim: UTF-8 validity is never required and NUL bytes pass
/// through; `--enter` on `Bytes` folds one CR into the record's bound,
/// while on `Paste` it stays a separate flag the executor turns into a
/// trailing Enter record.
pub(crate) fn materialize<'a>(
    input: &'a TerminalSendInput,
    deadline: Instant,
    signals: &mut dyn SignalWatch,
) -> Result<Cow<'a, TerminalSendInput>, SourceFailure> {
    let (mut bytes, enter, paste, option, value) = match input {
        TerminalSendInput::File { path, enter, paste } => (
            acquire_file(path, *paste, deadline, signals)?,
            *enter,
            *paste,
            "--file",
            Some(path.clone()),
        ),
        TerminalSendInput::Stdin { enter, paste } => (
            acquire_stdin(*paste, deadline, signals)?,
            *enter,
            *paste,
            "--stdin",
            None,
        ),
        TerminalSendInput::Text { .. }
        | TerminalSendInput::Keys(_)
        | TerminalSendInput::Bytes { .. }
        | TerminalSendInput::Paste { .. } => {
            return Ok(Cow::Borrowed(input));
        }
    };
    if paste {
        if bytes.len() > limit_for(true) {
            return Err(usage(
                option,
                value,
                SendSourceCause::Oversize {
                    limit: crate::invocation::MAXIMUM_SEND_PASTE_BYTES,
                },
            ));
        }
        return Ok(Cow::Owned(TerminalSendInput::Paste {
            content: bytes,
            enter,
        }));
    }
    // Routing: content that fits the text record bound as valid UTF-8 keeps
    // the single `text` record path bit-identical; anything else becomes an
    // opaque `bytes` record under the larger literal bound.
    if bytes.len() + usize::from(enter) <= crate::invocation::MAXIMUM_SEND_TEXT_BYTES {
        match String::from_utf8(bytes) {
            Ok(text) => return Ok(Cow::Owned(TerminalSendInput::Text { text, enter })),
            Err(error) => bytes = error.into_bytes(),
        }
    }
    if bytes.len() + usize::from(enter) > limit_for(false) {
        return Err(usage(
            option,
            value,
            SendSourceCause::Oversize {
                limit: crate::invocation::MAXIMUM_SEND_BYTES_BYTES,
            },
        ));
    }
    Ok(Cow::Owned(TerminalSendInput::Bytes { bytes, enter }))
}
