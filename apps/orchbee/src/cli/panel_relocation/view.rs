//! Read-only layout guards and revision-fenced Client effects for relocation.
use super::{
    internal,
    targeting::{self, Panel},
};
use crate::{
    cli::{
        Failure, FailureKind,
        client_control::{Answer, exchange as control},
        query,
        source::ArmedSignals,
    },
    local_client::LocalClient,
};
use orchbee_workbench_protocol::{
    ClientPanelLayout, ClientSplitIdentity, ClientSplitPath, ClientViewAction,
    ClientViewAvailability, ClientViewControl, ClientViewControlStatus, ClientViewIntent,
    ClientViewOutcome, ClientViewPhase, ClientViewRecord, PanelNode, Snapshot,
};
use std::time::{Duration, Instant};

pub(super) fn observe(
    client: &mut LocalClient,
    original: &ClientViewRecord,
    snapshot: &Snapshot,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<ClientViewRecord, Failure> {
    loop {
        signals.check_before(deadline)?;
        let reply = control::exchange(
            client,
            &snapshot.session_id,
            ClientViewAction::Get {
                client_id: original.client_id.clone(),
            },
            deadline,
            signals,
        )?;
        let current = match reply.outcome {
            ClientViewOutcome::Found { client } => *client,
            ClientViewOutcome::Denied {} => return Err(Failure::new(FailureKind::Denied)),
            ClientViewOutcome::NotFound {} | ClientViewOutcome::Unavailable { .. } => {
                return Err(Failure::new(FailureKind::ServiceUnavailable));
            }
            ClientViewOutcome::Synchronized { .. }
            | ClientViewOutcome::Listed { .. }
            | ClientViewOutcome::Operation { .. }
            | ClientViewOutcome::Conflict { .. } => {
                return Err(Failure::new(FailureKind::ProtocolInvalid));
            }
        };
        if current.availability != ClientViewAvailability::Current
            || current.view.phase == ClientViewPhase::Unavailable
        {
            return Err(Failure::new(FailureKind::ServiceUnavailable));
        }
        let can_refresh_empty = current.view.workbench_revision >= original.view.workbench_revision
            && targeting::unchanged_empty_selection(
                original.view.selection.as_ref(),
                current.view.selection.as_ref(),
            );
        if current.view.phase == ClientViewPhase::Ready
            && (current.view.workbench_revision >= snapshot.revision || can_refresh_empty)
        {
            if !targeting::follows_original(
                original.view.selection.as_ref(),
                current.view.selection.as_ref(),
                snapshot,
            ) {
                return Err(Failure::new(FailureKind::Denied));
            }
            return Ok(current);
        }
        std::thread::sleep(
            Duration::from_millis(50).min(deadline.saturating_duration_since(Instant::now())),
        );
    }
}
pub(super) fn submit(
    client: &mut LocalClient,
    current: &ClientViewRecord,
    snapshot: &Snapshot,
    intent: ClientViewIntent,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<Answer, Failure> {
    signals.check_before(deadline)?;
    control::submit_control(
        client,
        &snapshot.session_id,
        ClientViewControl {
            operation_id: query::request_id()?,
            client_id: current.client_id.clone(),
            expected_view_revision: current.view_revision,
            minimum_workbench_revision: snapshot.revision,
            intent,
        },
        deadline,
        signals,
    )
}
pub(super) fn layout(
    client: &mut LocalClient,
    current: &ClientViewRecord,
    snapshot: &Snapshot,
    target: &Panel,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<ClientPanelLayout, Failure> {
    let answer = submit(
        client,
        current,
        snapshot,
        ClientViewIntent::Layout {
            terminal_context_id: target.terminal_context_id.clone(),
            tab_id: target.tab_id.clone(),
            panel_id: target.panel_id.clone(),
        },
        deadline,
        signals,
    )?;
    let layout = answer
        .reply
        .as_ref()
        .and_then(|reply| match &reply.outcome {
            ClientViewOutcome::Operation { operation } => match &operation.status {
                ClientViewControlStatus::LayoutRead { layout, .. } => Some(layout),
                ClientViewControlStatus::PopupOpened { .. }
                | ClientViewControlStatus::PopupClosed { .. }
                | ClientViewControlStatus::PresentationRead { .. }
                | ClientViewControlStatus::ViewportRead { .. }
                | ClientViewControlStatus::NavigationRead { .. }
                | ClientViewControlStatus::TerminalAttentionRead { .. }
                | ClientViewControlStatus::AttentionRead { .. }
                | ClientViewControlStatus::LayoutAdjusted { .. }
                | ClientViewControlStatus::DirectionFocused { .. }
                | ClientViewControlStatus::DirectionUnchanged { .. }
                | ClientViewControlStatus::Queued { .. }
                | ClientViewControlStatus::Delivered { .. }
                | ClientViewControlStatus::Applied { .. }
                | ClientViewControlStatus::ZoomUnchanged { .. }
                | ClientViewControlStatus::ZoomToggled { .. }
                | ClientViewControlStatus::Rejected { .. }
                | ClientViewControlStatus::Incomplete { .. } => None,
            },
            ClientViewOutcome::Synchronized { .. }
            | ClientViewOutcome::Listed { .. }
            | ClientViewOutcome::Found { .. }
            | ClientViewOutcome::NotFound { .. }
            | ClientViewOutcome::Conflict { .. }
            | ClientViewOutcome::Denied { .. }
            | ClientViewOutcome::Unavailable { .. } => None,
        });
    if let Some(layout) = layout.filter(|_| answer.exit_code() == 0) {
        if layout.workbench_revision == snapshot.revision
            && layout.terminal_context_id == target.terminal_context_id
            && layout.tab_id == target.tab_id
            && layout.source_panel_id == target.panel_id
        {
            return Ok((**layout).clone());
        }
        return Err(Failure::new(FailureKind::Denied));
    }
    Err(Failure::new(match answer.exit_code() {
        130 => FailureKind::Cancelled,
        7 => FailureKind::Timeout,
        8 => FailureKind::OutcomeUnknown,
        4 => FailureKind::ServiceUnavailable,
        5 => FailureKind::TransportLoss,
        _ => FailureKind::Denied,
    }))
}
pub(super) fn split_path(
    snapshot: &Snapshot,
    target: &Panel,
    identity: &ClientSplitIdentity,
) -> Result<ClientSplitPath, Failure> {
    let tab = snapshot
        .terminal_contexts
        .iter()
        .find(|context| context.terminal_context_id == target.terminal_context_id)
        .and_then(|context| context.tabs.iter().find(|tab| tab.tab_id == target.tab_id))
        .ok_or_else(|| Failure::new(FailureKind::NotFound))?;
    fn walk(
        node: &PanelNode,
        identity: &ClientSplitIdentity,
        path: &mut Vec<u8>,
    ) -> Option<ClientSplitPath> {
        let PanelNode::Split {
            split_id, children, ..
        } = node
        else {
            return None;
        };
        if *split_id == identity.split_id {
            return (children[0].slot_id == identity.first_slot_id
                && children[1].slot_id == identity.second_slot_id)
                .then(|| ClientSplitPath::new(path.clone()).ok())
                .flatten();
        }
        for (index, child) in children.iter().enumerate() {
            path.push(u8::try_from(index).ok()?);
            let found = walk(&child.node, identity, path);
            path.pop();
            if found.is_some() {
                return found;
            }
        }
        None
    }
    walk(&tab.root, identity, &mut vec![]).ok_or_else(internal)
}

pub(super) fn parent_split(
    snapshot: &Snapshot,
    target: &Panel,
) -> Result<ClientSplitIdentity, Failure> {
    let tab = snapshot
        .terminal_contexts
        .iter()
        .find(|context| context.terminal_context_id == target.terminal_context_id)
        .and_then(|context| context.tabs.iter().find(|tab| tab.tab_id == target.tab_id))
        .ok_or_else(|| Failure::new(FailureKind::NotFound))?;
    fn find(node: &PanelNode, target: &Panel) -> Option<ClientSplitIdentity> {
        let PanelNode::Split {
            split_id, children, ..
        } = node
        else {
            return None;
        };
        if children.iter().any(|child| matches!(&child.node, PanelNode::Panel { panel_id, .. } if panel_id == &target.panel_id)) {
            return Some(ClientSplitIdentity {
                split_id: split_id.clone(),
                first_slot_id: children[0].slot_id.clone(),
                second_slot_id: children[1].slot_id.clone(),
            });
        }
        children.iter().find_map(|child| find(&child.node, target))
    }
    find(&tab.root, target).ok_or_else(|| Failure::new(FailureKind::Rejected))
}
