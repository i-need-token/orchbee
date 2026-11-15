//! The outcome record of one helper run: how the run ended and, from the helper's `orchbee.update`
//! document, the version, phase and error code, written to the diagnostic log as static tokens,
//! counts and flags. The document is the helper's untrusted output: only its closed members are
//! read, and a value outside the log's token grammar is never written.
use orchbee_local_runtime::{CommandFailure, CommandOutput, LogField, LogLevel, LogValue};
use std::sync::{Mutex, PoisonError};

/// The diagnostic record of one helper run.
const EVENT: &str = "automatic-update";
/// The most distinct document values a process writes; later ones read `other`.
const MAXIMUM_TOKENS: usize = 32;
const MAXIMUM_TOKEN_BYTES: usize = 64;

/// How one scheduled run ended.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Outcome {
    /// No helper started; the token names why.
    NotStarted(&'static str),
    /// The helper exited and answered its document.
    Answered {
        status: Option<i32>,
        version: Option<&'static str>,
        phase: &'static str,
        /// `none` when the run succeeded.
        error: &'static str,
        up_to_date: bool,
    },
    /// The helper exited without a readable `orchbee.update` document, such as the refusal of a
    /// version that does not know the helper.
    Unanswered { status: Option<i32> },
    /// The run did not end by itself; the token names how it ended: `cancelled` by the Server's
    /// stop, `timeout` at the run limit, `unsettled` when its Job's end is unknown, `failed` when it
    /// could not be watched.
    Ended(&'static str),
}

impl Outcome {
    /// The outcome of a helper whose bounded exchange ended with `output`.
    pub(super) fn from_output(output: &CommandOutput) -> Self {
        match output.failure {
            None | Some(CommandFailure::ExitFailure) => {}
            Some(CommandFailure::SpawnUnavailable | CommandFailure::SpawnFailed) => {
                return Self::NotStarted("spawn-failed");
            }
            Some(CommandFailure::Cancelled) => return Self::Ended("cancelled"),
            Some(CommandFailure::Timeout) => return Self::Ended("timeout"),
            Some(CommandFailure::PhysicalUnknown) => return Self::Ended("unsettled"),
            Some(CommandFailure::Io) => return Self::Ended("failed"),
        }
        let status = output.exit_code;
        if output.stdout_truncated {
            return Self::Unanswered { status };
        }
        let Ok(document) = serde_json::from_slice::<serde_json::Value>(&output.stdout) else {
            return Self::Unanswered { status };
        };
        let result = &document["result"];
        let (Some("orchbee.update"), Some(phase)) =
            (document["schema"].as_str(), result["phase"].as_str())
        else {
            return Self::Unanswered { status };
        };
        let error = match &result["error"] {
            serde_json::Value::Null => "none",
            serde_json::Value::String(code) => token(code),
            _ => return Self::Unanswered { status },
        };
        Self::Answered {
            status,
            version: result["releaseVersion"].as_str().map(token),
            phase: token(phase),
            error,
            up_to_date: result["upToDate"].as_bool().unwrap_or(false),
        }
    }

    /// The record's level and fields. A failed or unfinished run is a warning; a run that answered
    /// without an error is information.
    pub(super) fn fields(&self) -> (LogLevel, Vec<LogField>) {
        match *self {
            Self::NotStarted(reason) => (
                LogLevel::Warning,
                vec![
                    ("run", LogValue::Token("not-started")),
                    ("reason", LogValue::Token(reason)),
                ],
            ),
            Self::Answered {
                status,
                version,
                phase,
                error,
                up_to_date,
            } => {
                let mut fields = vec![("run", LogValue::Token("answered"))];
                fields.extend(status_field(status));
                fields.extend(version.map(|version| ("version", LogValue::Token(version))));
                fields.extend([
                    ("phase", LogValue::Token(phase)),
                    ("error", LogValue::Token(error)),
                    ("upToDate", LogValue::Flag(up_to_date)),
                ]);
                let level = if error == "none" {
                    LogLevel::Info
                } else {
                    LogLevel::Warning
                };
                (level, fields)
            }
            Self::Unanswered { status } => {
                let mut fields = vec![("run", LogValue::Token("unanswered"))];
                fields.extend(status_field(status));
                (LogLevel::Warning, fields)
            }
            Self::Ended(how) => (LogLevel::Warning, vec![("run", LogValue::Token(how))]),
        }
    }

    /// Writes this outcome's record to the diagnostic log.
    pub(super) fn record(&self) {
        let (level, fields) = self.fields();
        orchbee_local_runtime::log_event(level, EVENT, &fields);
    }
}

fn status_field(status: Option<i32>) -> Option<LogField> {
    status
        .and_then(|code| u64::try_from(code).ok())
        .map(|code| ("status", LogValue::Count(code)))
}

static TOKENS: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());

/// A static copy of a document value for the log. A value outside the log's token grammar is
/// `invalid`, and every distinct value after the first [`MAXIMUM_TOKENS`] is `other`, so a helper
/// cannot grow the table. The table's lock guards only this lookup.
fn token(text: &str) -> &'static str {
    let admitted = (1..=MAXIMUM_TOKEN_BYTES).contains(&text.len())
        && text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'));
    if !admitted {
        return "invalid";
    }
    let mut tokens = TOKENS.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(known) = tokens.iter().find(|known| **known == text) {
        return known;
    }
    if tokens.len() >= MAXIMUM_TOKENS {
        return "other";
    }
    // Bounded by MAXIMUM_TOKENS short values for the life of the process.
    let interned: &'static str = Box::leak(text.to_owned().into_boxed_str());
    tokens.push(interned);
    interned
}
