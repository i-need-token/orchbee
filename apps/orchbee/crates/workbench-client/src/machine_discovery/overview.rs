//! Settled discovery values: one saved machine's row observation and the bounded overview of its
//! canonical navigation. Discovery produces them; Client state holds them.
use crate::client_context::{ClientContextLocation, ClientContextTarget};
use crate::terminal_text::sanitize_terminal_text;
use orchbee_machine_protocol::hub::MachineStateName;
use orchbee_workbench_protocol::{SessionId, Snapshot, WorkbenchProjectId, WorkbenchRevision};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Overview {
    pub(crate) session: SessionId,
    pub(crate) revision: WorkbenchRevision,
    pub(crate) projects: usize,
    pub(crate) workspaces: usize,
    pub(crate) tabs: usize,
    pub(crate) labels: Vec<String>,
    pub(crate) contexts: std::sync::Arc<[Context]>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Context {
    pub target: ClientContextTarget,
    /// Organizational metadata; changing it must not change the terminal target.
    pub project_id: Option<WorkbenchProjectId>,
    pub project: Option<String>,
    pub workspace: Option<String>,
}
impl Overview {
    pub(crate) fn from_snapshot(snapshot: &Snapshot) -> Result<Self, ()> {
        let mut labels = Vec::new();
        for project in &snapshot.navigation.projects {
            if labels.len() < 8 {
                labels.push(display_label(&project.label));
            }
        }
        for workspace in &snapshot.navigation.workspaces {
            if labels.len() < 8 {
                labels.push(display_label(&workspace.label));
            }
        }
        let mut contexts = Vec::new();
        // The wire boundary has already validated ordering, membership and protocol limits.
        {
            let ordered = snapshot.navigation.ordered_workspaces().map_err(|_| ())?;
            contexts.extend(ordered.into_iter().map(|(project, workspace)| Context {
                target: ClientContextTarget {
                    session_id: snapshot.session_id.clone(),
                    location: ClientContextLocation::Workspace {
                        workspace_id: workspace.workspace_id.clone(),
                        terminal_context_id: workspace.terminal_context_id.clone(),
                    },
                },
                project_id: workspace.project_id.clone(),
                project: project.map(|project| display_label(&project.label)),
                workspace: Some(display_label(&workspace.label)),
            }));
        }
        Ok(Self {
            session: snapshot.session_id.clone(),
            revision: snapshot.revision,
            projects: snapshot.navigation.projects.len(),
            workspaces: contexts.len(),
            tabs: snapshot
                .terminal_contexts
                .iter()
                .map(|c| c.tabs.len())
                .sum(),
            labels,
            contexts: contexts.into(),
        })
    }
}
pub fn display_label(value: &str) -> String {
    let sanitized = sanitize_terminal_text(value);
    let mut label = String::new();
    let mut scalars = 0;
    for grapheme in sanitized.graphemes(true) {
        let length = grapheme.chars().count();
        if scalars + length > 80 {
            break;
        }
        label.push_str(grapheme);
        scalars += length;
    }
    label
}
/// A saved machine's row: the hub's state of the Machine, its overview while one is observed, or
/// the hub out of reach. A Machine the hub has not reported has none.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Observation {
    /// No hub can be started or reached.
    Unavailable,
    /// The hub's state, without a current overview.
    State(MachineStateName),
    /// The hub reads the Machine `online` and its observer holds a current overview.
    Online(Overview),
}
/// What a row's word means at a glance; render chooses the color.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Tone {
    Success,
    Progress,
    Neutral,
    Warning,
    Error,
}
impl Observation {
    /// The picker message of the row's word.
    pub(crate) fn key(&self) -> &'static str {
        match self {
            Self::Unavailable => "hub-unavailable",
            Self::Online(_) => "online",
            Self::State(name) => match name {
                MachineStateName::Online => "online",
                // "connecting" is the picker's message while a selection waits.
                MachineStateName::Connecting => "opening",
                MachineStateName::Reconnecting => "reconnecting",
                MachineStateName::Offline => "offline",
                MachineStateName::Disabled => "disabled",
                MachineStateName::NeedsLogin => "needs-login",
                MachineStateName::Unresponsive => "unresponsive",
                MachineStateName::Untrusted => "untrusted",
                MachineStateName::NotInstalled => "not-installed",
                MachineStateName::NeedsUpdate => "needs-update",
                MachineStateName::LocalNeedsUpdate => "local-needs-update",
                MachineStateName::LoginFailed => "login-failed",
                MachineStateName::KeyChanged => "key-changed",
                MachineStateName::Unsupported => "unsupported",
                MachineStateName::Incompatible => "incompatible",
                MachineStateName::Unknown => "unknown",
            },
        }
    }
    pub(crate) fn tone(&self) -> Tone {
        match self {
            Self::Unavailable => Tone::Warning,
            Self::Online(_) => Tone::Success,
            Self::State(name) => match name {
                MachineStateName::Online => Tone::Success,
                MachineStateName::Connecting | MachineStateName::Reconnecting => Tone::Progress,
                MachineStateName::Offline
                | MachineStateName::Disabled
                | MachineStateName::NeedsLogin
                | MachineStateName::Unknown => Tone::Neutral,
                MachineStateName::Unresponsive
                | MachineStateName::Untrusted
                | MachineStateName::NotInstalled
                | MachineStateName::NeedsUpdate
                | MachineStateName::LocalNeedsUpdate => Tone::Warning,
                MachineStateName::LoginFailed
                | MachineStateName::KeyChanged
                | MachineStateName::Unsupported
                | MachineStateName::Incompatible => Tone::Error,
            },
        }
    }
}
