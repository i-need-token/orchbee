//! Shared bounded-input provenance and failures. Native acquisition supplies bytes and cancellation.

use crate::cli::{Failure, FailureKind};
use crate::invocation::{SendSourceCause, UsageReason};
use serde_json::{Map, Value, json};
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
        display: None,
        client_refusal: None,
        stale_view: false,
        usage: Some(Box::new(UsageReason::SendSource {
            option,
            value,
            cause,
        })),
    })
}
