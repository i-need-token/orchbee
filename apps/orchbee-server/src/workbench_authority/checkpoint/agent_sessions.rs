//! Retained per-Presentation Agent session references: the closed table of resumable providers,
//! the native session id format, and their whole-document validation and capture. Selecting a live
//! reference from observations belongs to the Session, and delivering a resume belongs to the
//! restore runtime; a retained reference never carries command text.
use super::directories::{StoredDirectory, members};
use super::{WorkbenchAuthority, WorkbenchCheckpointError as Error};
use orchbee_workbench_protocol::{PresentationBinding, Snapshot, TerminalPresentationId, WorkerId};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// A provider whose native session a restart may resume. The table is closed: a provider joins it
/// only with a witness that its hook session key is the id its CLI resumes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum AgentResumeProvider {
    #[serde(rename = "claude-code")]
    ClaudeCode,
    #[serde(rename = "codex")]
    Codex,
}

impl AgentResumeProvider {
    /// Maps a canonical Agent provider id onto the table; any other provider has no row.
    #[must_use]
    pub fn from_provider_id(provider_id: &str) -> Option<Self> {
        match provider_id {
            "claude-code" => Some(Self::ClaudeCode),
            "codex" => Some(Self::Codex),
            _ => None,
        }
    }

    /// The canonical Agent provider id, identical to the checkpoint spelling.
    #[must_use]
    pub const fn provider_id(self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude-code",
            Self::Codex => "codex",
        }
    }
}

/// One admitted native session reference. The id is a hyphenated UUID kept verbatim: its charset
/// is inert in PowerShell argument mode and in cmd, and it cannot begin with `-`.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RetainedAgentSession {
    provider: AgentResumeProvider,
    session_id: String,
}

impl RetainedAgentSession {
    /// Admits `session_id` only when it is a hyphenated UUID; any other text has no reference.
    #[must_use]
    pub fn new(provider: AgentResumeProvider, session_id: &str) -> Option<Self> {
        hyphenated_uuid(session_id).then(|| Self {
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

fn hyphenated_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => byte == b'-',
            _ => byte.is_ascii_hexdigit(),
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
}
