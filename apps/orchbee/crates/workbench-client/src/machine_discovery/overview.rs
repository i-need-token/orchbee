//! Settled discovery values: one saved machine's connection observation and the bounded
//! overview of its canonical navigation. Discovery produces them; Client state holds them.
use crate::client_context::{ClientContextLocation, ClientContextTarget};
use crate::terminal_text::sanitize_terminal_text;
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Observation {
    Waiting,
    Connecting,
    Online(Overview),
    Reconnecting,
    Attention,
    Stale,
}
impl Observation {
    pub(crate) fn key(&self) -> &'static str {
        match self {
            Self::Waiting => "waiting",
            Self::Connecting => "opening",
            Self::Online(_) => "online",
            Self::Reconnecting => "reconnecting",
            Self::Attention => "attention",
            Self::Stale => "stale",
        }
    }
}
