pub(crate) mod agent_names;
mod agent_prompt;
mod caller_context;
mod completion;
pub(crate) mod config;
pub(crate) mod escape;
pub(crate) mod integration;
mod mutation;
pub(crate) mod output;
pub(crate) mod present;
mod query;
pub(crate) mod results;
pub(crate) mod schema;
pub(crate) mod send;
mod site;
mod site_diagnose;
#[cfg(any(target_os = "linux", windows))]
#[cfg_attr(windows, path = "cli/source_windows.rs")]
pub(crate) mod source;
mod text;
#[cfg(any(target_os = "linux", windows))]
mod wait_deadline;

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
    #[cfg(windows)]
    interrupt: crate::windows_signal::ControlInterrupt,
    /// The thread mask captured before the source guard armed.
    #[cfg(target_os = "linux")]
    old_mask: nix::sys::signal::SigSet,
    /// Test-only mock seam: `release` reports a kernel failure so the emit
    /// finalizer's error routing is provable without a real mask fault.
    #[cfg(all(test, target_os = "linux"))]
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

#[cfg(not(any(target_os = "linux", windows)))]
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
    /// A dedicated failure envelope schema when the failure belongs to a
    /// bounded command family (`worktree list`); `None` is the shared
    /// `orchbee.cli.failure` document.
    pub(crate) failure_schema: Option<&'static str>,
    /// Observed I/O detail retained from the native cohort check: the
    /// technical `io::ErrorKind` plus the OS error number when one exists.
    /// Carried to the localized stderr diagnostic only and never serialized
    /// — same rule as `usage`; Boxed so it never enlarges the hot Result.
    pub(crate) io_diagnostic: Option<Box<IoDiagnostic>>,
}

/// The bounded technical detail a cohort-check failure observed: the std
/// `io::ErrorKind` and, when the OS reported one, its error number. This is
/// observed data, not a classified reason vocabulary.
#[derive(Debug)]
pub(crate) struct IoDiagnostic {
    pub(crate) kind: std::io::ErrorKind,
    pub(crate) raw_os_error: Option<i32>,
}

impl Failure {
    pub(crate) const fn new(kind: FailureKind) -> Self {
        Self {
            kind,
            details: None,
            source: None,
            usage: None,
            failure_schema: None,
            io_diagnostic: None,
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

    /// Mark the failure as belonging to a dedicated family envelope; only
    /// post-boundary failures carry it.
    pub(crate) fn failure_schema(mut self, schema_id: &'static str) -> Self {
        self.failure_schema = Some(schema_id);
        self
    }

    /// Retain the observed I/O kind (and OS error number when one exists) for
    /// the localized stderr diagnostic. Only the Linux native-cohort check
    /// produces this today.
    #[cfg(any(target_os = "linux", windows))]
    pub(crate) fn io_diagnostic(mut self, error: &std::io::Error) -> Self {
        self.io_diagnostic = Some(Box::new(IoDiagnostic {
            kind: error.kind(),
            raw_os_error: error.raw_os_error(),
        }));
        self
    }
}

fn target_details(resource: crate::invocation::Resource, target: &str) -> Map<String, Value> {
    Map::from_iter([
        ("resource".to_owned(), json!(resource.as_str())),
        ("target".to_owned(), json!(target)),
    ])
}

pub(crate) fn stage_details(stage: &str) -> Map<String, Value> {
    Map::from_iter([("stage".to_owned(), json!(stage))])
}

pub(crate) fn stage_reason_details(stage: &str, reason: &str) -> Map<String, Value> {
    Map::from_iter([
        ("stage".to_owned(), json!(stage)),
        ("reason".to_owned(), json!(reason)),
    ])
}

/// Encode a failure into its process answer. `format` is the format selected by
/// a valid selector before the failure; `None` keeps stdout empty.
pub(crate) fn failure_output(
    failure: Failure,
    format: Option<OutputFormat>,
    locale: Locale,
) -> Output {
    let status = failure.kind.exit_code();
    let stderr = locale.cli_failure(&failure);
    match format {
        Some(OutputFormat::Json) => {
            let version = if failure.failure_schema == Some(schema::AGENT_FAILURE_ID)
                && failure
                    .details
                    .as_ref()
                    .and_then(|details| details.get("timeoutMs"))
                    == Some(&Value::Null)
            {
                2
            } else {
                1
            };
            let mut document = Map::from_iter([
                (
                    "schema".to_owned(),
                    json!(failure.failure_schema.unwrap_or(schema::FAILURE_ID)),
                ),
                ("version".to_owned(), json!(version)),
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
#[cfg(any(target_os = "linux", windows))]
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
        failure_schema: None,
        io_diagnostic: None,
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

/// Print the same build's offline Agent instructions through the bounded sink.
pub(crate) fn skill(locale: Locale) -> Output {
    fixed_text(
        match locale {
            Locale::EnUs => include_str!("../docs/cli-agent-skill.md"),
            Locale::ZhCn => include_str!("../docs/cli-agent-skill.zh-CN.md"),
        },
        locale,
    )
}

/// One effectful command: the mutation executor owns Workbench actions, while
/// `terminal send` takes its own input-record path on the same session
/// discipline — both admit once, submit once, and never replay.
pub(crate) fn mutation(command: MutationCommand, format: OutputFormat, locale: Locale) -> Output {
    if let MutationCommand::TerminalSend { target, input } = &command {
        return send::run(target, input, format, locale);
    }
    if let MutationCommand::AgentSendKeys { target, keys } = &command {
        return send::run_agent(target, keys, format, locale);
    }
    if let MutationCommand::AgentPrompt { target, text, wait } = &command {
        return agent_prompt::run(target, text, wait.as_ref(), format, locale);
    }
    mutation::run(&command, format, locale)
}

/// `site start` is neither a query nor a workbench mutation: it runs the
/// shared connect-or-activate core once and reports the branch it took.
pub(crate) fn site_start(format: OutputFormat, locale: Locale) -> Output {
    site::start(format, locale)
}

/// `site diagnose` is neither a query nor a mutation: it reports bounded
/// local runtime facts and the limits of those observations — it never
/// connects, creates, repairs, or predicts a later start.
pub(crate) fn site_diagnose(format: OutputFormat, locale: Locale) -> Output {
    site_diagnose::run(format, locale)
}

/// `agent integration` is CLI-local provider onboarding: the bounded local
/// runner owns every provider command, and no Site is started or contacted.
pub(crate) fn agent_integration(
    command: crate::invocation::AgentIntegrationCommand,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    integration::run(&command, format, locale)
}

pub(crate) fn query(command: QueryCommand, format: OutputFormat, locale: Locale) -> Output {
    match &command {
        QueryCommand::ApiSchema { schema_id } => {
            schema::answer(schema_id.as_deref(), format, locale)
        }
        QueryCommand::ConfigCheck => config::check(format, locale),
        // `agent doctor` is fully offline: it answers bounded filesystem
        // facts itself and never opens a session or touches the network
        // query path.
        QueryCommand::AgentDoctor {
            providers,
            asset_roots,
        } => crate::agent_doctor::run(providers, asset_roots, format, locale),
        // `terminal get` answers the Server's own launch-read join on the
        // admitted session; the snapshot supplies only the typed target id.
        QueryCommand::TerminalGet(target) => match query::terminal_get(target) {
            Err(failure) => failure_output(failure, Some(format), locale),
            Ok((source, projection)) => {
                success_output(projection, source, &command, format, locale)
            }
        },
        // Agent reads exchange on their own admitted Client Session; answers
        // arrive already projected with the correlated request identity.
        QueryCommand::AgentList {
            include_ended,
            limit,
            cursor,
        } => agent_output(
            query::agent_list(*include_ended, *limit, cursor.clone()),
            &command,
            format,
            locale,
        ),
        QueryCommand::AgentRead {
            target,
            history,
            unwrap,
            lines,
        } => {
            match query::read_agent_terminal(
                target,
                *history,
                results::ReadPresentation {
                    unwrap: *unwrap,
                    tail: *lines,
                },
            ) {
                Err(failure) => failure_output(failure, Some(format), locale),
                Ok((source, projection)) => {
                    success_output(projection, source, &command, format, locale)
                }
            }
        }
        QueryCommand::AgentGet(session_id) => {
            agent_output(query::agent_get(session_id), &command, format, locale)
        }
        QueryCommand::AgentExplain(session_id) => {
            agent_output(query::agent_explain(session_id), &command, format, locale)
        }
        QueryCommand::AgentWait {
            session,
            until,
            after_revision,
            timeout_ms,
        } => agent_output(
            query::agent_wait(session, until, *after_revision, *timeout_ms),
            &command,
            format,
            locale,
        ),
        // `terminal read` couples the snapshot to one viewer observation on
        // the same session; its answer arrives already projected.
        QueryCommand::TerminalRead {
            target,
            history,
            unwrap,
            lines,
        } => {
            match query::read_terminal(
                target,
                *history,
                results::ReadPresentation {
                    unwrap: *unwrap,
                    tail: *lines,
                },
            ) {
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
            matcher,
            rows,
            timeout_ms,
        } => match query::wait_output(target, matcher, *rows, *timeout_ms) {
            Err(failure) => failure_output(failure, Some(format), locale),
            Ok((source, projection)) => {
                success_output(projection, source, &command, format, locale)
            }
        },
        // `workspace close-status` reads the retained operation record on the
        // admitted session; the stable `(project, workspace)` pair is the
        // whole selector — the record outlives the active projection.
        QueryCommand::WorkspaceCloseStatus { project, workspace } => {
            match query::workspace_close_status(project, workspace) {
                Err(failure) => failure_output(failure, Some(format), locale),
                Ok((source, projection)) => {
                    success_output(projection, source, &command, format, locale)
                }
            }
        }
        // `worktree list` couples the snapshot to one Workbench worktree-list
        // read on the same session; the directory is a strict typed ID and
        // membership is the Server's call, never the CLI's.
        QueryCommand::WorktreeList { project, directory } => {
            match query::worktree_list(project, directory) {
                Err(failure) => failure_output(failure, Some(format), locale),
                Ok((source, projection)) => {
                    success_output(projection, source, &command, format, locale)
                }
            }
        }
        // `launch-profile list`/`get` ride one typed catalog read on the
        // admitted session; selector resolution is the Server's call under
        // its lock — the CLI carries the intent verbatim.
        QueryCommand::LaunchProfileList => match query::launch_profile_catalog(None) {
            Err(failure) => failure_output(failure, Some(format), locale),
            Ok((source, projection)) => {
                success_output(projection, source, &command, format, locale)
            }
        },
        QueryCommand::LaunchProfileGet(selector) => {
            match query::launch_profile_catalog(Some(selector.clone())) {
                Err(failure) => failure_output(failure, Some(format), locale),
                Ok((source, projection)) => {
                    success_output(projection, source, &command, format, locale)
                }
            }
        }
        QueryCommand::PanelCurrent(target) => {
            match caller_context::read(
                target.as_ref(),
                |name| std::env::var_os(name),
                query::read_snapshot,
            ) {
                Err(failure) => failure_output(failure, Some(format), locale),
                Ok((source, projection)) => {
                    success_output(projection, source, &command, format, locale)
                }
            }
        }
        _ => network_query(&command, format, locale),
    }
}

/// One Agent read answer: pre-admission failures keep the generic failure
/// document, every post-admission fault or semantic refusal answers with the
/// dedicated agent failure document, and a successful read emits the
/// verbatim wire outcome under the correlated `agent-read` source. A
/// captured signal answers the agent document's `cancelled` kind — its
/// owed finalization rides the common emit path like every guarded output.
fn agent_output(
    result: Result<(Value, results::Projection), query::AgentQueryError>,
    command: &QueryCommand,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    match result {
        Err(query::AgentQueryError::Admission(failure)) => {
            failure_output(failure, Some(format), locale)
        }
        Err(query::AgentQueryError::Read(mut failure)) => {
            if failure.failure_schema.is_none() {
                failure = failure.failure_schema(schema::AGENT_FAILURE_ID);
            }
            failure_output(failure, Some(format), locale)
        }
        // Only the Linux signal owner constructs `Cancelled`.
        #[cfg(any(target_os = "linux", windows))]
        Err(query::AgentQueryError::Cancelled(cancelled)) => {
            let query::AgentCancelled {
                failure,
                finalization,
            } = *cancelled;
            let mut output = failure_output(
                failure.failure_schema(schema::AGENT_FAILURE_ID),
                Some(format),
                locale,
            );
            output.finalization = Some(finalization);
            output
        }
        Ok((source, projection)) => success_output(projection, source, command, format, locale),
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

#[cfg(windows)]
impl SourceFinalization {
    fn new(interrupt: crate::windows_signal::ControlInterrupt) -> Self {
        Self { interrupt }
    }
    pub(crate) fn release(self) -> std::io::Result<()> {
        drop(self.interrupt);
        Ok(())
    }
}
