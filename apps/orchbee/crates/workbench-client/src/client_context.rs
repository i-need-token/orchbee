//! Identity-only Client navigation locations shared by view selection and Workbench projection.
//! This value grants no snapshot, terminal attachment or mutation authority.
use orchbee_workbench_protocol::{WorkbenchTerminalContextId, WorkbenchWorkspaceId};

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", content = "identity", deny_unknown_fields)]
pub enum ClientContextLocation {
    /// No Workspace is open. This UI location has no terminal Context or persistent identity.
    #[serde(skip)]
    NoWorkspace,
    Workspace {
        workspace_id: WorkbenchWorkspaceId,
        terminal_context_id: WorkbenchTerminalContextId,
    },
}

impl ClientContextLocation {
    /// The exact terminal context when this navigation location has one.
    pub(crate) fn terminal_context_id(&self) -> Option<&WorkbenchTerminalContextId> {
        match self {
            Self::NoWorkspace => None,
            Self::Workspace {
                terminal_context_id,
                ..
            } => Some(terminal_context_id),
        }
    }
}
