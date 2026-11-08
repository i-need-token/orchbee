//! Typed CLI outcomes shared by command owners and localized text.
//!
//! The failure model fixes each kind's stable token, exit code, and JSON envelope. The result
//! values below are the typed facts that text presentation renders. Nothing here performs effects
//! or formats prose, so `cli_text` depends on these values instead of on the commands that
//! produce them.

use orchbee_agent_observation_protocol::AgentSafeInteger;
use serde_json::{Map, Value, json};

use crate::invocation::grammar::schemas::FAILURE_ID;

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
    ConnectionCapacity,
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
            Self::ConnectionCapacity => "connection-capacity",
            Self::Timeout => "timeout",
            Self::TransportLoss => "transport-loss",
        }
    }

    pub(crate) const fn exit_code(self) -> u8 {
        match self {
            Self::InvalidUsage | Self::InvalidIdentifier => 2,
            Self::NotFound | Self::Ambiguous | Self::Denied | Self::Rejected => 3,
            Self::ServiceUnavailable | Self::ConnectionCapacity => 4,
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
    /// Exact missing-sibling evidence from a native cohort check, carried to
    /// localized text only. Raw I/O details belong to `status client` JSON.
    /// Boxed so it never enlarges the hot Result.
    pub(crate) io_diagnostic: Option<Box<IoDiagnostic>>,
    /// Ambiguity candidates as list handles with their labels, carried to
    /// localized text only; JSON keeps the identifier `candidates` detail.
    /// Boxed so it never enlarges the hot Result.
    pub(crate) candidate_display: Option<Box<CandidateDisplay>>,
    /// The Client's typed reason when it refused one Client view step. Never
    /// serialized here: a command owner publishes it only through its own
    /// closed details vocabulary.
    pub(crate) client_refusal: Option<orchbee_workbench_protocol::ClientViewControlReason>,
}

/// Ambiguity candidates rendered for people, in listing order.
#[derive(Debug)]
pub(crate) struct CandidateDisplay {
    pub(crate) lines: Vec<String>,
}

/// Artifact identity proven absent by its own open operation.
#[derive(Debug)]
pub(crate) struct IoDiagnostic {
    pub(crate) missing_artifact: Option<String>,
}

impl Failure {
    /// Composite operations preserve rich failures alongside earlier known commits.
    pub(crate) fn observation(&self) -> Value {
        json!({"kind":self.kind.as_str(),"details":self.details,"source":self.source,"schema":self.failure_schema.unwrap_or(FAILURE_ID),"version":1})
    }

    pub(crate) fn document(&self) -> Value {
        let mut document = Map::from_iter([
            (
                "schema".to_owned(),
                json!(self.failure_schema.unwrap_or(FAILURE_ID)),
            ),
            ("version".to_owned(), json!(1)),
        ]);
        let mut failure = Map::from_iter([("kind".to_owned(), json!(self.kind.as_str()))]);
        if let Some(details) = &self.details {
            failure.insert("details".to_owned(), json!(details));
        }
        document.insert("failure".to_owned(), Value::Object(failure));
        if let Some(source) = &self.source {
            document.insert("source".to_owned(), source.clone());
        }
        Value::Object(document)
    }

    pub(crate) const fn new(kind: FailureKind) -> Self {
        Self {
            kind,
            details: None,
            source: None,
            usage: None,
            failure_schema: None,
            io_diagnostic: None,
            candidate_display: None,
            client_refusal: None,
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

    /// Retain bounded evidence from a native cohort admission error. The
    /// missing sibling is available only when its artifact-open operation
    /// returned `NotFound`.
    pub(crate) fn io_diagnostic(mut self, error: &orchbee_local_runtime::CohortError) -> Self {
        self.io_diagnostic = Some(Box::new(IoDiagnostic {
            missing_artifact: error
                .missing_artifact()
                .map(orchbee_local_runtime::CohortArtifact::file_name),
        }));
        self
    }
}

pub(crate) fn target_details(
    resource: crate::invocation::Resource,
    target: &str,
) -> Map<String, Value> {
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

/// The authority source of answers computed from local runtime files without a Server.
pub(crate) fn local_runtime_source() -> Value {
    json!({ "kind": "local-runtime" })
}

/// The lifecycle state of one named session as the local runtime observed it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SessionState {
    Running,
    Stopping,
    Stopped,
    Absent,
    Starting,
    Deleting,
    RecoveryRequired,
    Recoverable,
    Unreadable,
}
impl SessionState {
    pub(crate) fn token(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Stopping => "stopping",
            Self::Stopped => "stopped",
            Self::Absent => "absent",
            Self::Starting => "starting",
            Self::Deleting => "deleting",
            Self::RecoveryRequired => "recovery-required",
            Self::Recoverable => "recoverable",
            Self::Unreadable => "unreadable",
        }
    }
    pub(crate) fn running(self) -> Option<bool> {
        match self {
            Self::Running | Self::Stopping => Some(true),
            Self::Stopped | Self::Absent | Self::Recoverable => Some(false),
            _ => None,
        }
    }
}

/// The state column of one `terminal list` row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TerminalListState {
    Observed,
    Agent(orchbee_workbench_protocol::PanelAgentState),
}

/// One Client's attention view of an Agent terminal.
#[derive(Debug)]
pub(crate) struct AgentClientViewDoc {
    pub(crate) client_id: orchbee_workbench_protocol::WorkbenchClientId,
    pub(crate) operation_id: orchbee_workbench_protocol::WorkbenchRequestId,
    pub(crate) phase: orchbee_workbench_protocol::ClientViewPhase,
    pub(crate) outer_focused: Option<bool>,
    pub(crate) focused: bool,
    pub(crate) attention_revision: AgentSafeInteger,
    pub(crate) state: AgentClientVersionMatch,
    pub(crate) state_change_revision: Option<AgentSafeInteger>,
    pub(crate) seen: Option<bool>,
    pub(crate) status: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AgentClientVersionMatch {
    Current,
    Missing,
    Stale,
}

impl AgentClientViewDoc {
    pub(crate) fn document(&self) -> Value {
        let state = match self.state {
            AgentClientVersionMatch::Current => "current",
            AgentClientVersionMatch::Missing => "missing",
            AgentClientVersionMatch::Stale => "stale",
        };
        json!({
            "clientId": self.client_id, "operationId": self.operation_id,
            "phase": self.phase, "outerFocused": self.outer_focused, "focused": self.focused,
            "attentionRevision": self.attention_revision, "state": state,
            "stateChangeRevision": self.state_change_revision, "seen": self.seen, "status": self.status,
        })
    }
}
