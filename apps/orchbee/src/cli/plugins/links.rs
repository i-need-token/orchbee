//! CLI adaptation for the application-private ordered link-handler route.

use std::time::Instant;

use crate::{
    cli::{Failure, FailureKind, console_interrupt::SignalWatch, results},
    local_client::LocalClient,
    terminal_link_activation::{self, Error, Guard, SnapshotMember},
};
#[cfg(test)]
use orchbee_local_runtime::PluginRegistryEntry;
use orchbee_workbench_protocol::{
    Snapshot, TerminalId, TerminalPresentationId, WorkbenchPanelId, WorkbenchTabId,
    WorkbenchTerminalContextId, WorkerId,
};

pub(in crate::cli) use crate::terminal_link_activation::{
    Activation,
    plugin::{Invocation, Selection},
};

struct CliGuard<'a, S: SignalWatch>(&'a mut S);

impl<S: SignalWatch> Guard for CliGuard<'_, S> {
    type Error = Failure;

    fn check(&mut self, deadline: Instant) -> Result<(), Self::Error> {
        self.0.check_before(deadline)
    }

    fn wake(&self) -> Option<orchbee_local_ipc::LocalIo<'_>> {
        self.0.poll_fd()
    }
}

fn failure(error: Error<Failure>) -> Failure {
    match error {
        Error::Gate(failure) => failure,
        Error::RegistryUnavailable => Failure::new(FailureKind::ServiceUnavailable),
        Error::IdentityUnavailable => Failure::new(FailureKind::InternalFailure),
        Error::MissingClickedUrl | Error::ProtocolInvalid => {
            Failure::new(FailureKind::ProtocolInvalid)
        }
        Error::InvalidContext | Error::RegistryChanged | Error::InvalidRequest => {
            Failure::new(FailureKind::Rejected)
        }
        Error::DefinitelyNotSubmitted => Failure::new(FailureKind::TransportLoss),
        Error::TransportUnavailable | Error::SourceChanged => {
            Failure::new(FailureKind::ServiceUnavailable)
        }
        Error::LimitExceeded => Failure::new(FailureKind::Rejected),
    }
}

pub(in crate::cli) fn select(
    url: &str,
    deadline: Instant,
    signals: &mut impl SignalWatch,
) -> Result<Option<Selection>, Failure> {
    terminal_link_activation::plugin::select(url, deadline, &mut CliGuard(signals))
        .map(|snapshot| snapshot.selection)
        .map_err(failure)
}

pub(in crate::cli) fn preflight(
    url: &str,
    selection: Selection,
) -> Result<terminal_link_activation::plugin::Preflight, Failure> {
    terminal_link_activation::plugin::Preflight::new(url, selection).map_err(failure)
}

pub(in crate::cli) fn observe(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    terminal: &results::TerminalRow,
    pane_directory: Option<String>,
    preflight: terminal_link_activation::plugin::Preflight,
    deadline: Instant,
    signals: &mut impl SignalWatch,
) -> Result<(Invocation, SnapshotMember), Failure> {
    let member = snapshot_member(snapshot, terminal)?;
    let context = terminal_link_activation::context::observe(
        client,
        snapshot,
        &member,
        pane_directory,
        preflight.provided_context(),
        deadline,
        &mut CliGuard(signals),
    )
    .map_err(failure)?;
    let invocation = preflight.finish(context).map_err(failure)?;
    Ok((invocation, member))
}

pub(in crate::cli) fn fence(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    member: &SnapshotMember,
    deadline: Instant,
    signals: &mut impl SignalWatch,
) -> Result<(), Failure> {
    terminal_link_activation::context::fence_member(
        client,
        snapshot,
        member,
        deadline,
        &mut CliGuard(signals),
    )
    .map_err(failure)
}

fn snapshot_member(
    snapshot: &Snapshot,
    terminal: &results::TerminalRow,
) -> Result<SnapshotMember, Failure> {
    let invalid = || Failure::new(FailureKind::ProtocolInvalid);
    Ok(SnapshotMember {
        session: snapshot.session_id.clone(),
        context: WorkbenchTerminalContextId::parse(&terminal.terminal_context_id)
            .map_err(|_| invalid())?,
        tab: WorkbenchTabId::parse(&terminal.tab_id).map_err(|_| invalid())?,
        panel: WorkbenchPanelId::parse(&terminal.panel_id).map_err(|_| invalid())?,
        presentation: TerminalPresentationId::parse(&terminal.presentation_id)
            .map_err(|_| invalid())?,
        worker: WorkerId::parse(&terminal.worker_id).map_err(|_| invalid())?,
        terminal: TerminalId::parse(&terminal.terminal_id).map_err(|_| invalid())?,
    })
}

pub(in crate::cli) fn submit(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    invocation: Invocation,
    deadline: Instant,
    signals: &mut impl SignalWatch,
) -> Result<Activation, Failure> {
    terminal_link_activation::plugin::submit(
        client,
        snapshot,
        invocation,
        deadline,
        &mut CliGuard(signals),
    )
    .map_err(failure)
}

#[cfg(test)]
fn select_entries(
    entries: &[orchbee_local_runtime::PluginRegistryEntry],
    url: &str,
    check: impl FnMut() -> Result<(), Failure>,
) -> Result<Option<Selection>, Failure> {
    terminal_link_activation::plugin::select_entries(entries, url, check).map_err(failure)
}

#[cfg(test)]
mod tests;
