//! Requests the Server wrote before it recorded a move still name the Host terminal as it ran
//! before (`proposals/platform/host-handover.md`, The steps of a move). The Worker runs such a
//! request against the terminal as it runs now and answers it in the request's own words, since
//! the Server checks every echo. It also notes which terminals it relays input to, which decides
//! when one is due to move.

use std::error::Error;

use orchbee_capability_node_protocol::{
    CapabilityNodeMessage, HostTerminalIdentity, TerminalControlAction, TerminalLifecycleRequest,
    TerminalLifecycleResult, TerminalViewerAction, TerminalViewerOutcome,
};

use super::Moves;
use crate::terminal_operations::host_set::HostSet;

/// What a translated request named, for its answer.
pub(in crate::terminal_operations) enum Named {
    Lifecycle(TerminalLifecycleRequest),
    Observed {
        named: HostTerminalIdentity,
        ran: HostTerminalIdentity,
    },
}

impl Moves {
    /// Runs `request` against the terminals as they run now; what it named when that differs.
    pub(in crate::terminal_operations) fn before(
        &mut self,
        request: &mut CapabilityNodeMessage,
        hosts: &HostSet<'_>,
    ) -> Option<Named> {
        match request {
            CapabilityNodeMessage::TerminalLifecycleRequest(lifecycle) => {
                let named = *lifecycle;
                lifecycle.target = self.translate(named.target);
                (lifecycle.target != named.target).then_some(Named::Lifecycle(named))
            }
            CapabilityNodeMessage::TerminalSendRequest(send) => {
                send.target = self.translate(send.target);
                self.note_relayed(send.target.terminal_id());
                None
            }
            CapabilityNodeMessage::TerminalViewerRequest(viewer) => match &mut viewer.action {
                TerminalViewerAction::Attach {
                    host_epoch,
                    terminal_id,
                } => {
                    if let Some(to) = self.alias(*host_epoch, *terminal_id) {
                        *host_epoch = to.host_epoch();
                        *terminal_id = to.terminal_id();
                    }
                    None
                }
                TerminalViewerAction::ReadLifecycle { target } => {
                    let named = *target;
                    *target = self.translate(named);
                    (*target != named).then_some(Named::Observed {
                        named,
                        ran: *target,
                    })
                }
                _ => None,
            },
            CapabilityNodeMessage::TerminalControlRequest(control) => {
                if matches!(
                    control.action,
                    TerminalControlAction::Input { .. }
                        | TerminalControlAction::SemanticInput { .. }
                        | TerminalControlAction::Resize { .. }
                        | TerminalControlAction::Paste { .. }
                ) {
                    let host = hosts.connection(hosts.by_attachment(control.viewer_id));
                    let viewers = host.viewers.borrow();
                    let target = viewers
                        .target(control.viewer_id)
                        .or_else(|| viewers.moved_target(control.viewer_id));
                    drop(viewers);
                    if let Some(target) = target {
                        self.note_relayed(target);
                    }
                }
                None
            }
            _ => None,
        }
    }
}

/// `result` in the words of the request that named `named`.
pub(in crate::terminal_operations) fn lifecycle_answer(
    result: TerminalLifecycleResult,
    named: Option<&Named>,
) -> Result<TerminalLifecycleResult, Box<dyn Error>> {
    let Some(Named::Lifecycle(request)) = named else {
        return Ok(result);
    };
    Ok(TerminalLifecycleResult::new(*request, result.outcome())?)
}

/// `outcome` in the words of the request that named `named`.
pub(in crate::terminal_operations) fn viewer_answer(
    outcome: &mut TerminalViewerOutcome,
    named: Option<&Named>,
) {
    if let (
        Some(Named::Observed { named, ran }),
        TerminalViewerOutcome::LifecycleObserved { target, .. },
    ) = (named, outcome)
        && target == ran
    {
        *target = *named;
    }
}
