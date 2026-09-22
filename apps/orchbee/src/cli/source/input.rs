use super::{SignalWatch, acquire_file, acquire_stdin};
use crate::cli::{Failure, FailureKind};
use crate::invocation::{SendSourceCause, TerminalSendInput, UsageReason};
use serde_json::{Map, Value, json};
use std::{borrow::Cow, time::Instant};
/// The declared form's acquisition bound: the literal `bytes` record bound
/// plus one spare byte to detect oversize, or the `paste` content bound.
/// The folded `--enter` CR counts toward the `bytes` record bound after
/// acquisition, so acquisition reads at most the plain bound.
pub(super) fn limit_for(paste: bool) -> usize {
    if paste {
        crate::invocation::MAXIMUM_SEND_PASTE_BYTES
    } else {
        crate::invocation::MAXIMUM_SEND_BYTES_BYTES
    }
}

/// Which declared source a source-stage failure names in its typed details.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InputSource {
    File,
    Stdin,
}

impl InputSource {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Stdin => "stdin",
        }
    }

    pub(super) const fn option(self) -> &'static str {
        match self {
            Self::File => "--file",
            Self::Stdin => "--stdin",
        }
    }
}

/// How a source stage ends before any runtime effect. `Standard` failures
/// keep the existing `orchbee.cli.failure` envelope (usage/validation/local
/// kinds); `Timeout`/`Cancelled` carry the `orchbee.cli.failure.source`
/// envelope through the explicit typed conversion in `send.rs`.
#[derive(Debug)]
pub(crate) enum SourceFailure {
    /// Validation or local failure on the standard failure path.
    Standard(Failure),
    /// The shared invocation deadline elapsed before the source completed.
    Timeout { source: InputSource },
    /// An eligible captured signal ended source acquisition.
    Cancelled { source: InputSource, signal: i32 },
}

/// The typed details object the `orchbee.cli.failure.source` envelope
/// carries: which declared source and, for a timeout, the configured shared
/// budget — never a measured duration, path, or payload byte.
pub(crate) fn source_details(source: InputSource, timeout_ms: Option<u64>) -> Map<String, Value> {
    let mut details = Map::from_iter([
        ("stage".to_owned(), json!("input")),
        ("inputSource".to_owned(), json!(source.as_str())),
    ]);
    if let Some(timeout_ms) = timeout_ms {
        details.insert("timeoutMs".to_owned(), json!(timeout_ms));
    }
    details
}

/// A source-stage usage failure on the standard `invalid-usage` path: the
/// machine envelope carries no details; the localized stderr names the
/// declared option and its concrete local cause.
pub(super) fn usage(
    option: &'static str,
    value: Option<String>,
    cause: SendSourceCause,
) -> SourceFailure {
    SourceFailure::Standard(Failure {
        kind: FailureKind::InvalidUsage,
        details: None,
        source: None,
        failure_schema: None,
        io_diagnostic: None,
        usage: Some(Box::new(UsageReason::SendSource {
            option,
            value,
            cause,
        })),
    })
}

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
        if bytes.len() > crate::invocation::MAXIMUM_SEND_PASTE_BYTES {
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
    if bytes.len() + usize::from(enter) > crate::invocation::MAXIMUM_SEND_BYTES_BYTES {
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
