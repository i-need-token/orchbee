//! Exact routing facts from an admitted Snapshot, without selectors or display rows.
use orchbee_client_server_protocol::{SessionId, TerminalId, WorkerId};
use orchbee_workbench_protocol::{PanelNode, Snapshot, TerminalPresentationId, WorkbenchPanelId};

#[derive(Clone, Debug)]
pub(crate) struct Binding {
    pub(crate) session: SessionId,
    pub(crate) worker: WorkerId,
    pub(crate) terminal: TerminalId,
}

impl Binding {
    pub(crate) fn parse(
        session: &str,
        worker: &str,
        terminal: &str,
    ) -> Result<Self, InvalidBinding> {
        Ok(Self {
            session: SessionId::parse(session).map_err(|_| InvalidBinding)?,
            worker: WorkerId::parse(worker).map_err(|_| InvalidBinding)?,
            terminal: TerminalId::parse(terminal).map_err(|_| InvalidBinding)?,
        })
    }
}

#[derive(Debug)]
pub(crate) struct InvalidBinding;

#[derive(Debug)]
pub(crate) enum PrimaryError {
    Unavailable(WorkbenchPanelId),
    InvalidBinding,
}

pub(crate) struct PrimaryTerminal {
    pub(crate) binding: Binding,
    pub(crate) presentation: TerminalPresentationId,
}

pub(crate) fn primary(
    snapshot: &Snapshot,
    panel: &WorkbenchPanelId,
) -> Result<PrimaryTerminal, PrimaryError> {
    fn find<'a>(
        node: &'a PanelNode,
        panel: &WorkbenchPanelId,
    ) -> Option<&'a orchbee_workbench_protocol::Presentation> {
        match node {
            PanelNode::Panel {
                panel_id,
                primary_presentation,
                ..
            } => (panel_id == panel).then_some(primary_presentation),
            PanelNode::Split { children, .. } => {
                children.iter().find_map(|slot| find(&slot.node, panel))
            }
        }
    }
    let presentation = snapshot
        .terminal_contexts
        .iter()
        .flat_map(|context| &context.tabs)
        .find_map(|tab| find(&tab.root, panel))
        .ok_or_else(|| PrimaryError::Unavailable(panel.clone()))?;
    let binding = presentation
        .binding
        .as_ref()
        .ok_or_else(|| PrimaryError::Unavailable(panel.clone()))?;
    Ok(PrimaryTerminal {
        binding: Binding::parse(
            snapshot.session_id.as_str(),
            binding.worker_id.as_str(),
            binding.terminal_id.as_str(),
        )
        .map_err(|_| PrimaryError::InvalidBinding)?,
        presentation: presentation.presentation_id.clone(),
    })
}
