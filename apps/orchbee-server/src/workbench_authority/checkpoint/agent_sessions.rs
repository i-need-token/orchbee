//! Retained per-Presentation Agent session references: the closed table of resumable providers,
//! the native session id format, and their whole-document validation and capture. Selecting a live
//! reference from observations belongs to the Session, and delivering a resume belongs to the
//! restore runtime; a retained reference never carries command text.
use super::directories::{StoredDirectory, members};
use super::{WorkbenchAuthority, WorkbenchCheckpointError as Error};
use orchbee_agent_observation_protocol::AgentProviderId;
use orchbee_workbench_protocol::{
    AgentNotResumedReason, PresentationAgentNotResumed, PresentationBinding, Snapshot,
    TerminalPresentationId, WorkerId,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// A provider whose native session a restart may resume. The table is closed: a provider joins it
/// only when its resume form is source-verified or documented, the id is bound to a value-taking
/// option or a resume subcommand, a strict validator below admits only inert ids, and a wrong or
/// stale id can only fail or open an empty or new session. Each spelling is both the provider's
/// native process id and its hook-contract provider id.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum AgentResumeProvider {
    #[serde(rename = "claude-code")]
    ClaudeCode,
    #[serde(rename = "codex")]
    Codex,
    #[serde(rename = "grok")]
    Grok,
    #[serde(rename = "opencode")]
    OpenCode,
    #[serde(rename = "kilo")]
    Kilo,
    #[serde(rename = "cursor")]
    Cursor,
    #[serde(rename = "qodercli")]
    QoderCli,
    #[serde(rename = "antigravity")]
    Antigravity,
    #[serde(rename = "droid")]
    Droid,
    #[serde(rename = "kimi")]
    Kimi,
    #[serde(rename = "letta")]
    Letta,
    #[serde(rename = "devin")]
    Devin,
    #[serde(rename = "hermes")]
    Hermes,
    #[serde(rename = "pi")]
    Pi,
    #[serde(rename = "omp")]
    Omp,
    #[serde(rename = "gemini")]
    Gemini,
    #[serde(rename = "github-copilot")]
    GitHubCopilot,
    #[serde(rename = "qwen-code")]
    QwenCode,
    #[serde(rename = "cline")]
    Cline,
}

impl AgentResumeProvider {
    /// Every resumable provider.
    pub const ALL: [Self; 19] = [
        Self::ClaudeCode,
        Self::Codex,
        Self::Grok,
        Self::OpenCode,
        Self::Kilo,
        Self::Cursor,
        Self::QoderCli,
        Self::Antigravity,
        Self::Droid,
        Self::Kimi,
        Self::Letta,
        Self::Devin,
        Self::Hermes,
        Self::Pi,
        Self::Omp,
        Self::Gemini,
        Self::GitHubCopilot,
        Self::QwenCode,
        Self::Cline,
    ];

    /// Maps a canonical Agent provider id onto the table; any other provider has no row.
    #[must_use]
    pub fn from_provider_id(provider_id: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|provider| provider.provider_id() == provider_id)
    }

    /// The canonical Agent provider id, identical to the checkpoint spelling.
    #[must_use]
    pub const fn provider_id(self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude-code",
            Self::Codex => "codex",
            Self::Grok => "grok",
            Self::OpenCode => "opencode",
            Self::Kilo => "kilo",
            Self::Cursor => "cursor",
            Self::QoderCli => "qodercli",
            Self::Antigravity => "antigravity",
            Self::Droid => "droid",
            Self::Kimi => "kimi",
            Self::Letta => "letta",
            Self::Devin => "devin",
            Self::Hermes => "hermes",
            Self::Pi => "pi",
            Self::Omp => "omp",
            Self::Gemini => "gemini",
            Self::GitHubCopilot => "github-copilot",
            Self::QwenCode => "qwen-code",
            Self::Cline => "cline",
        }
    }
}

/// One admitted native session reference. The id is kept verbatim and is valid for its provider:
/// every accepted id is inert in PowerShell argument mode and in cmd, and none begins with `-`.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RetainedAgentSession {
    provider: AgentResumeProvider,
    session_id: String,
}

impl RetainedAgentSession {
    /// Admits `session_id` only when it is valid for `provider`; any other text has no reference.
    #[must_use]
    pub fn new(provider: AgentResumeProvider, session_id: &str) -> Option<Self> {
        valid_id(provider, session_id).then(|| Self {
            provider,
            session_id: session_id.to_owned(),
        })
    }

    #[must_use]
    pub fn provider(&self) -> AgentResumeProvider {
        self.provider
    }

    /// The provider's native session id. It is private: it never reaches the wire, logs or notices.
    #[must_use]
    pub fn session_id(&self) -> &str {
        &self.session_id
    }
}

/// Whether `id` is a native session id in `provider`'s own grammar. Every grammar admits only ASCII
/// letters, digits, `_` and `-`, plus the `:` of Letta's stored default-conversation key, which is
/// split before it is typed; none admits a leading `-`. Case is kept verbatim, never folded.
fn valid_id(provider: AgentResumeProvider, id: &str) -> bool {
    use AgentResumeProvider as P;
    match provider {
        P::ClaudeCode
        | P::Codex
        | P::Grok
        | P::Cursor
        | P::QoderCli
        | P::Antigravity
        | P::Droid
        | P::Pi
        | P::Omp
        | P::Gemini
        | P::GitHubCopilot
        | P::QwenCode => hyphenated_uuid(id),
        P::OpenCode | P::Kilo => opencode_session(id),
        P::Kimi => id.strip_prefix("session_").is_some_and(uuid_v4),
        P::Letta => letta_conversation(id),
        P::Devin => devin_session(id),
        P::Hermes => hermes_session(id),
        P::Cline => cline_session(id),
    }
}

/// 8-4-4-4-12 hex digits in either case.
fn hyphenated_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => byte == b'-',
            _ => byte.is_ascii_hexdigit(),
        })
}

/// A hyphenated UUID of version 4 and the RFC variant.
fn uuid_v4(value: &str) -> bool {
    hyphenated_uuid(value)
        && value.as_bytes()[14] == b'4'
        && matches!(
            value.as_bytes()[19],
            b'8' | b'9' | b'a' | b'b' | b'A' | b'B'
        )
}

fn lower_hex(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

/// `ses_`, 12 lowercase hex digits, then 14 ASCII letters or digits.
fn opencode_session(value: &str) -> bool {
    value.strip_prefix("ses_").is_some_and(|rest| {
        rest.len() == 26
            && rest.is_ascii()
            && lower_hex(&rest[..12])
            && rest[12..].bytes().all(|byte| byte.is_ascii_alphanumeric())
    })
}

/// A conversation, `conv-U` or `local-conv-U`, or an agent's default conversation,
/// `default:agent-U` or `default:agent-local-U`.
fn letta_conversation(value: &str) -> bool {
    let conversation = value
        .strip_prefix("local-")
        .unwrap_or(value)
        .strip_prefix("conv-");
    let agent = value
        .strip_prefix("default:agent-")
        .map(|rest| rest.strip_prefix("local-").unwrap_or(rest));
    conversation.or(agent).is_some_and(hyphenated_uuid)
}

/// One to eight ASCII alphanumeric runs joined by single hyphens, at most 64 characters.
fn devin_session(value: &str) -> bool {
    value.len() <= 64
        && value.split('-').count() <= 8
        && value
            .split('-')
            .all(|run| !run.is_empty() && run.bytes().all(|byte| byte.is_ascii_alphanumeric()))
}

/// `YYYYMMDD_HHMMSS_` and 6 to 12 lowercase hex digits, or `local-` and 64 lowercase hex digits.
fn hermes_session(value: &str) -> bool {
    if let Some(digest) = value.strip_prefix("local-") {
        return digest.len() == 64 && lower_hex(digest);
    }
    let digits =
        |text: &str, len: usize| text.len() == len && text.bytes().all(|b| b.is_ascii_digit());
    let mut parts = value.split('_');
    matches!(
        (parts.next(), parts.next(), parts.next(), parts.next()),
        (Some(date), Some(time), Some(suffix), None)
            if digits(date, 8) && digits(time, 6) && (6..=12).contains(&suffix.len()) && lower_hex(suffix)
    )
}

/// 13 decimal digits, `_`, then 5 lowercase letters or digits.
fn cline_session(value: &str) -> bool {
    value.len() == 19
        && value.bytes().enumerate().all(|(index, byte)| match index {
            0..13 => byte.is_ascii_digit(),
            13 => byte == b'_',
            _ => byte.is_ascii_digit() || byte.is_ascii_lowercase(),
        })
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct StoredAgentSession {
    presentation_id: TerminalPresentationId,
    provider: AgentResumeProvider,
    session_id: String,
}

/// Admits the whole list or rejects the document: every row names a distinct member Presentation
/// that also has a directory row, a valid id, and a distinct `(provider, sessionId)`.
pub(super) fn validate(
    rows: &[StoredAgentSession],
    snapshot: &Snapshot,
    directories: &[StoredDirectory],
) -> Result<BTreeMap<TerminalPresentationId, RetainedAgentSession>, Error> {
    let retained = members(snapshot);
    let directories = directory_rows(directories);
    let mut admitted = BTreeMap::new();
    let mut sessions = BTreeSet::new();
    for row in rows {
        let session = RetainedAgentSession::new(row.provider, &row.session_id)
            .ok_or(Error::InvalidAgentSession)?;
        if !retained.contains_key(&row.presentation_id)
            || !directories.contains(&row.presentation_id)
            || !sessions.insert(session.clone())
            || admitted
                .insert(row.presentation_id.clone(), session)
                .is_some()
        {
            return Err(Error::InvalidAgentSession);
        }
    }
    Ok(admitted)
}

/// `observed = None` means recording is off and writes no rows, pending ones included. Otherwise
/// each member takes its live reference, else its pending restart reference, and is written only
/// when the same checkpoint has its directory row; the first member in capture order keeps a
/// shared `(provider, sessionId)`.
pub(super) fn capture(
    authority: &WorkbenchAuthority,
    worker: &WorkerId,
    observed: Option<&[(PresentationBinding, RetainedAgentSession)]>,
    directories: &[StoredDirectory],
) -> Result<Vec<StoredAgentSession>, Error> {
    let Some(observed) = observed else {
        return Ok(Vec::new());
    };
    let retained = members(authority.snapshot());
    let mut live = BTreeMap::new();
    for (binding, session) in observed {
        if &binding.worker_id != worker
            || !retained.values().any(|b| b.as_ref() == Some(binding))
            || live.insert(binding.terminal_id.clone(), session).is_some()
        {
            return Err(Error::InvalidAgentSession);
        }
    }
    let directories = directory_rows(directories);
    let mut sessions = BTreeSet::new();
    Ok(retained
        .into_iter()
        .filter(|(id, _)| directories.contains(id))
        .filter_map(|(id, binding)| {
            let session = binding
                .as_ref()
                .and_then(|b| live.get(&b.terminal_id).copied())
                .or_else(|| authority.restart_agent_sessions.get(&id))?;
            sessions
                .insert(session.clone())
                .then(|| StoredAgentSession {
                    presentation_id: id,
                    provider: session.provider,
                    session_id: session.session_id.clone(),
                })
        })
        .collect())
}

fn directory_rows(directories: &[StoredDirectory]) -> BTreeSet<&TerminalPresentationId> {
    directories.iter().map(|row| &row.presentation_id).collect()
}

impl WorkbenchAuthority {
    /// The restart reference still pending for this Presentation, if any.
    #[must_use]
    pub fn pending_agent_session(
        &self,
        presentation_id: &TerminalPresentationId,
    ) -> Option<&RetainedAgentSession> {
        self.restart_agent_sessions.get(presentation_id)
    }

    /// Consumes the pending restart reference so later captures no longer carry it. Taking it
    /// never changes the Workbench revision.
    pub fn take_pending_agent_session(
        &mut self,
        presentation_id: &TerminalPresentationId,
    ) -> Option<RetainedAgentSession> {
        self.restart_agent_sessions.remove(presentation_id)
    }

    /// Puts back a reference a delivery took when the Host then typed nothing, so later attempts
    /// and captures see it again. A slot that holds a reference keeps it. Never changes the
    /// Workbench revision.
    pub fn return_pending_agent_session(
        &mut self,
        presentation_id: &TerminalPresentationId,
        session: RetainedAgentSession,
    ) {
        self.restart_agent_sessions
            .entry(presentation_id.clone())
            .or_insert(session);
    }

    /// Consumes the pending reference and publishes, in exactly one revision on the Presentation
    /// still bound as `binding`, that its resume typed nothing for `reason` at `at` Unix
    /// milliseconds. The fact names the provider only. Returns whether a fact was published;
    /// nothing later clears it except a rebinding, a removal or the end of this Server lifetime.
    pub fn give_up_agent_resume(
        &mut self,
        presentation_id: &TerminalPresentationId,
        binding: Option<&PresentationBinding>,
        reason: AgentNotResumedReason,
        at: u64,
    ) -> bool {
        let Some(session) = self.restart_agent_sessions.remove(presentation_id) else {
            return false;
        };
        let (Some(binding), Ok(provider_id)) = (
            binding,
            AgentProviderId::parse(session.provider().provider_id()),
        ) else {
            return false;
        };
        let fact = PresentationAgentNotResumed {
            at,
            provider_id,
            reason,
        };
        self.commit_presentation_fact(presentation_id, binding, |presentation| {
            presentation.agent_not_resumed = Some(Box::new(fact));
            true
        })
    }
}
