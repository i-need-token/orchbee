mod completion;
mod config;
pub(crate) mod escape;
mod mutation;
mod output;
mod present;
mod query;
pub(crate) mod results;
mod schema;
pub(crate) mod send;
mod site;
#[cfg(target_os = "linux")]
pub(crate) mod source;
mod text;

#[cfg(test)]
mod tests;

use serde_json::{Map, Value, json};

use crate::invocation::{
    CompletionShell, HelpTopic, MutationCommand, OutputFormat, ParseFailure, ParseFailureKind,
    QueryCommand,
};
use crate::localization::Locale;
use crate::presentation::ProcessResult;

/// One fully prepared process answer: status code plus already-encoded
/// streams. `finalization` is the deferred, noncloneable duty the common
/// `main` emit path must consume after its last write attempt — only the
/// Linux `terminal send` source signal guard ever produces one.
pub(crate) struct Output {
    pub(crate) status: u8,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: String,
    pub(crate) finalization: Option<SourceFinalization>,
}

/// A noncloneable finalization owed by `emit` after the last output attempt.
/// Only the Linux `terminal send` source guard creates one: the inherited
/// thread mask stays blocked while the process answer is written, and
/// `release` restores it exactly — a signal arriving after the final drain
/// still reaches its original disposition. It is never constructed on other
/// platforms, and no path may drop it without releasing it.
#[must_use = "the owed finalization must be released by the common emit finalizer"]
pub(crate) struct SourceFinalization {
    /// The thread mask captured before the source guard armed.
    #[cfg(target_os = "linux")]
    old_mask: nix::sys::signal::SigSet,
    /// Test-only mock seam: `release` reports a kernel failure so the emit
    /// finalizer's error routing is provable without a real mask fault.
    #[cfg(test)]
    fail_release: bool,
}

#[cfg(target_os = "linux")]
impl SourceFinalization {
    pub(crate) fn new(old_mask: nix::sys::signal::SigSet) -> Self {
        Self {
            old_mask,
            #[cfg(test)]
            fail_release: false,
        }
    }

    /// A token whose `release` always reports a kernel failure — mock
    /// evidence for the emit finalizer's restore-failure routing.
    #[cfg(test)]
    pub(crate) fn failing() -> Self {
        Self {
            old_mask: nix::sys::signal::SigSet::empty(),
            fail_release: true,
        }
    }

    /// Restore the captured thread mask exactly; a failure is a terminal
    /// internal failure — reported by the caller, never retried.
    pub(crate) fn release(self) -> Result<(), nix::errno::Errno> {
        #[cfg(test)]
        if self.fail_release {
            return Err(nix::errno::Errno::EIO);
        }
        self.old_mask
            .thread_swap_mask(nix::sys::signal::SigmaskHow::SIG_SETMASK)
            .map(drop)
    }
}

#[cfg(not(target_os = "linux"))]
impl SourceFinalization {
    /// Unreachable on non-Linux: the token is never constructed there.
    pub(crate) fn release(self) -> Result<(), std::convert::Infallible> {
        Ok(())
    }
}

/// Legacy process results (TUI launch, version, root help) also finish
/// through the preparation bound; their `&'static` bodies are far under it, so
/// the overflow arm is unreachable — but if it ever ran it still reports a
/// localized failure, never a hardcoded English string.
pub(crate) fn process_result(result: ProcessResult, locale: Locale) -> Output {
    match output::encode_text(result.stdout) {
        Ok(stdout) => Output {
            status: result.status,
            stdout,
            stderr: result.stderr.to_owned(),
            finalization: None,
        },
        Err(output::TooLarge) => failure_output(
            Failure::new(FailureKind::OutputTooLarge).details(Map::from_iter([(
                "limitBytes".to_owned(),
                json!(output::LIMIT),
            )])),
            None,
            locale,
        ),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FailureKind {
    Ambiguous,
    Cancelled,
    CheckFailed,
    Denied,
    Incompatible,
    InternalFailure,
    InvalidIdentifier,
    InvalidUsage,
    NotFound,
    OutcomeUnknown,
    OutputTooLarge,
    ProtocolInvalid,
    Rejected,
    ServiceUnavailable,
    SessionCapacity,
    Timeout,
    TransportLoss,
}

impl FailureKind {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Ambiguous => "ambiguous",
            Self::Cancelled => "cancelled",
            Self::CheckFailed => "check-failed",
            Self::Denied => "denied",
            Self::Incompatible => "incompatible",
            Self::InternalFailure => "internal-failure",
            Self::InvalidIdentifier => "invalid-identifier",
            Self::InvalidUsage => "invalid-usage",
            Self::NotFound => "not-found",
            Self::OutcomeUnknown => "outcome-unknown",
            Self::OutputTooLarge => "output-too-large",
            Self::ProtocolInvalid => "protocol-invalid",
            Self::Rejected => "rejected",
            Self::ServiceUnavailable => "service-unavailable",
            Self::SessionCapacity => "session-capacity",
            Self::Timeout => "timeout",
            Self::TransportLoss => "transport-loss",
        }
    }

    pub(crate) const fn exit_code(self) -> u8 {
        match self {
            Self::InvalidUsage | Self::InvalidIdentifier => 2,
            Self::NotFound | Self::Ambiguous | Self::Denied | Self::Rejected => 3,
            Self::ServiceUnavailable | Self::SessionCapacity => 4,
            Self::TransportLoss | Self::ProtocolInvalid => 5,
            Self::Incompatible => 6,
            Self::Timeout => 7,
            Self::OutcomeUnknown => 8,
            Self::Cancelled => 130,
            Self::CheckFailed | Self::InternalFailure | Self::OutputTooLarge => 1,
        }
    }
}

/// One typed CLI failure; `details` stays absent when the matrix omits it.
#[derive(Debug)]
pub(crate) struct Failure {
    pub(crate) kind: FailureKind,
    pub(crate) details: Option<Map<String, Value>>,
    /// An authoritative source only when one genuinely exists.
    pub(crate) source: Option<Value>,
    /// The parser's concrete usage classification for `InvalidUsage`; carried
    /// to the localized stderr diagnostic only and never serialized. Boxed so
    /// the rarely-reached reason does not enlarge every `Result<_, Failure>`.
    pub(crate) usage: Option<Box<crate::invocation::UsageReason>>,
}

impl Failure {
    pub(crate) const fn new(kind: FailureKind) -> Self {
        Self {
            kind,
            details: None,
            source: None,
            usage: None,
        }
    }

    pub(crate) fn details(mut self, details: Map<String, Value>) -> Self {
        self.details = Some(details);
        self
    }

    pub(crate) fn source(mut self, source: Value) -> Self {
        self.source = Some(source);
        self
    }
}

fn target_details(resource: crate::invocation::Resource, target: &str) -> Map<String, Value> {
    Map::from_iter([
        ("resource".to_owned(), json!(resource.as_str())),
        ("target".to_owned(), json!(target)),
    ])
}

fn stage_details(stage: &str) -> Map<String, Value> {
    Map::from_iter([("stage".to_owned(), json!(stage))])
}

fn stage_reason_details(stage: &str, reason: &str) -> Map<String, Value> {
    Map::from_iter([
        ("stage".to_owned(), json!(stage)),
        ("reason".to_owned(), json!(reason)),
    ])
}

/// Encode a failure into its process answer. `format` is the format selected by
/// a valid selector before the failure; `None` keeps stdout empty.
fn failure_output(failure: Failure, format: Option<OutputFormat>, locale: Locale) -> Output {
    let status = failure.kind.exit_code();
    let stderr = locale.cli_failure(&failure);
    match format {
        Some(OutputFormat::Json) => {
            let mut document = Map::from_iter([
                ("schema".to_owned(), json!(schema::FAILURE_ID)),
                ("version".to_owned(), json!(1)),
            ]);
            let mut failure_object =
                Map::from_iter([("kind".to_owned(), json!(failure.kind.as_str()))]);
            if let Some(details) = failure.details {
                failure_object.insert("details".to_owned(), Value::Object(details));
            }
            document.insert("failure".to_owned(), Value::Object(failure_object));
            if let Some(source) = failure.source {
                document.insert("source".to_owned(), source);
            }
            // Failure documents are small and bounded well below the output cap.
            let stdout =
                output::encode_document(&Value::Object(document)).unwrap_or_else(|_| Vec::new());
            Output {
                status,
                stdout,
                stderr,
                finalization: None,
            }
        }
        Some(OutputFormat::Text) | None => Output {
            status,
            stdout: Vec::new(),
            stderr,
            finalization: None,
        },
    }
}

/// `terminal send` source-stage failures carry the dedicated
/// `orchbee.cli.failure.source` envelope: the same failure object shape but
/// no authority source and no request identity, because nothing was admitted
/// yet. The caller's selected format still decides whether stdout carries
/// the document; the status is explicit since a handled source interruption
/// exits with the conventional `128 + signo`, not a kind-derived code.
#[cfg(target_os = "linux")]
pub(crate) fn source_failure_output(
    failure: Failure,
    status: u8,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    let stderr = locale.cli_failure(&failure);
    match format {
        OutputFormat::Json => {
            let mut failure_object =
                Map::from_iter([("kind".to_owned(), json!(failure.kind.as_str()))]);
            if let Some(details) = failure.details {
                failure_object.insert("details".to_owned(), Value::Object(details));
            }
            let document = json!({
                "schema": schema::SOURCE_FAILURE_ID,
                "version": 1,
                "failure": Value::Object(failure_object),
            });
            let stdout = output::encode_document(&document).unwrap_or_else(|_| Vec::new());
            Output {
                status,
                stdout,
                stderr,
                finalization: None,
            }
        }
        OutputFormat::Text => Output {
            status,
            stdout: Vec::new(),
            stderr,
            finalization: None,
        },
    }
}

/// Parse failures answer before any runtime work; envelope rules come from the
/// parser's recorded selection.
pub(crate) fn parse_failure(failure: ParseFailure, locale: Locale) -> Output {
    let (kind, details) = match &failure.kind {
        ParseFailureKind::InvalidUsage => (FailureKind::InvalidUsage, None),
        ParseFailureKind::InvalidIdentifier { resource, target } => (
            FailureKind::InvalidIdentifier,
            Some(target_details(*resource, target)),
        ),
    };
    let record = Failure {
        kind,
        details,
        source: None,
        usage: failure.usage.map(Box::new),
    };
    let format = match failure.envelope {
        crate::invocation::FailureEnvelope::Json => Some(OutputFormat::Json),
        crate::invocation::FailureEnvelope::Text | crate::invocation::FailureEnvelope::Silent => {
            None
        }
    };
    failure_output(record, format, locale)
}

/// Fixed text outputs (help, completion scripts) still finish through the
/// preparation bound; their size is a small constant asserted by tests.
fn fixed_text(body: &str, locale: Locale) -> Output {
    match output::encode_text(body) {
        Ok(stdout) => Output {
            status: 0,
            stdout,
            stderr: String::new(),
            finalization: None,
        },
        Err(output::TooLarge) => failure_output(
            Failure::new(FailureKind::OutputTooLarge).details(Map::from_iter([(
                "limitBytes".to_owned(),
                json!(output::LIMIT),
            )])),
            None,
            locale,
        ),
    }
}

pub(crate) fn help(topic: HelpTopic, locale: Locale) -> Output {
    fixed_text(text::help(topic, locale), locale)
}

pub(crate) fn completion(shell: CompletionShell, locale: Locale) -> Output {
    fixed_text(&completion::render(shell), locale)
}

/// One effectful command: the mutation executor owns Workbench actions, while
/// `terminal send` takes its own input-record path on the same session
/// discipline — both admit once, submit once, and never replay.
pub(crate) fn mutation(command: MutationCommand, format: OutputFormat, locale: Locale) -> Output {
    if let MutationCommand::TerminalSend { target, input } = &command {
        return send::run(target, input, format, locale);
    }
    mutation::run(&command, format, locale)
}

/// `site start` is neither a query nor a workbench mutation: it runs the
/// shared connect-or-activate core once and reports the branch it took.
pub(crate) fn site_start(format: OutputFormat, locale: Locale) -> Output {
    site::start(format, locale)
}

pub(crate) fn query(command: QueryCommand, format: OutputFormat, locale: Locale) -> Output {
    match &command {
        QueryCommand::ApiSchema { schema_id } => {
            schema::answer(schema_id.as_deref(), format, locale)
        }
        QueryCommand::ConfigCheck => config::check(format, locale),
        // `terminal get` answers the Server's own launch-read join on the
        // admitted session; the snapshot supplies only the typed target id.
        QueryCommand::TerminalGet(target) => match query::terminal_get(target) {
            Err(failure) => failure_output(failure, Some(format), locale),
            Ok((source, projection)) => {
                success_output(projection, source, &command, format, locale)
            }
        },
        // `terminal read` couples the snapshot to one viewer observation on
        // the same session; its answer arrives already projected.
        QueryCommand::TerminalRead { target, history } => {
            match query::read_terminal(target, *history) {
                Err(failure) => failure_output(failure, Some(format), locale),
                Ok((source, projection)) => {
                    success_output(projection, source, &command, format, locale)
                }
            }
        }
        // `terminal wait-output` couples the snapshot to one bounded
        // read-only observation on the same session; its answer arrives
        // already projected, like `terminal read`.
        QueryCommand::TerminalWaitOutput {
            target,
            match_text,
            rows,
            timeout_ms,
        } => match query::wait_output(target, match_text, *rows, *timeout_ms) {
            Err(failure) => failure_output(failure, Some(format), locale),
            Ok((source, projection)) => {
                success_output(projection, source, &command, format, locale)
            }
        },
        _ => network_query(&command, format, locale),
    }
}

fn network_query(command: &QueryCommand, format: OutputFormat, locale: Locale) -> Output {
    match query::read_snapshot() {
        Err(failure) => failure_output(failure, Some(format), locale),
        Ok(snapshot) => {
            let source = results::snapshot_source(&snapshot);
            match results::project(command, &snapshot) {
                Err(failure) => failure_output(failure.source(source), Some(format), locale),
                Ok(projection) => success_output(projection, source, command, format, locale),
            }
        }
    }
}

fn success_output(
    projection: results::Projection,
    source: Value,
    command: &QueryCommand,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    let schema_id = results::schema_id(command);
    // A preparation overflow keeps the caller's selected format, and a text
    // failure never writes a stdout document.
    let too_large = |source: &Value| {
        failure_output(
            Failure::new(FailureKind::OutputTooLarge)
                .details(Map::from_iter([(
                    "limitBytes".to_owned(),
                    json!(output::LIMIT),
                )]))
                .source(source.clone()),
            Some(format),
            locale,
        )
    };
    // The api commands answer machine documents in every format.
    let document_format = match command {
        QueryCommand::ApiSnapshot => OutputFormat::Json,
        _ => format,
    };
    match document_format {
        OutputFormat::Json => {
            let document = json!({
                "schema": schema_id,
                "version": results::schema_version(command),
                "source": source,
                "result": projection.result(),
            });
            match output::encode_document(&document) {
                Ok(stdout) => Output {
                    status: 0,
                    stdout,
                    stderr: String::new(),
                    finalization: None,
                },
                Err(output::TooLarge) => too_large(&source),
            }
        }
        OutputFormat::Text => {
            match output::render_text(|out| present::render(out, &projection, command, locale)) {
                Ok(stdout) => Output {
                    status: 0,
                    stdout,
                    stderr: String::new(),
                    finalization: None,
                },
                Err(output::TooLarge) => too_large(&source),
            }
        }
    }
}
