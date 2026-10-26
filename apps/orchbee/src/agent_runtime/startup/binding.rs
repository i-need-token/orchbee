//! Exact routing facts from an admitted Snapshot, without selectors or display rows.
use orchbee_client_server_protocol::{SessionId, TerminalId, WorkerId};
use orchbee_workbench_protocol::{PanelNode, Snapshot, TerminalPresentationId, WorkbenchPanelId};

/// Protocol-validated routing identities; construction alone does not prove Snapshot membership.
#[derive(Clone, Debug)]
pub(crate) struct Binding {
    pub(crate) session: SessionId,
    pub(crate) worker: WorkerId,
    pub(crate) terminal: TerminalId,
}

impl Binding {
    /// Admit wire identity syntax without querying or selecting a live terminal.
    ///
    /// # Errors
    /// Returns `InvalidBinding` if any Session, Worker or terminal identity is malformed.
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

/// At least one routing identity failed the destination protocol's syntax checks.
#[derive(Debug)]
pub(crate) struct InvalidBinding;

/// Separate an absent primary binding from identities invalid for the input protocol.
#[derive(Debug)]
pub(crate) enum PrimaryError {
    Unavailable(WorkbenchPanelId),
    InvalidBinding,
}

/// The exact Panel's primary presentation and Server routing from one admitted Snapshot.
pub(crate) struct PrimaryTerminal {
    pub(crate) binding: Binding,
    pub(crate) presentation: TerminalPresentationId,
}

/// Resolve only the requested Panel's primary presentation; never fall back to a child terminal.
/// The caller owns Snapshot admission and freshness checks.
///
/// # Errors
/// Returns `Unavailable` for a missing Panel or unbound primary, and `InvalidBinding`
/// when its routing identities cannot be admitted by the input protocol.
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
