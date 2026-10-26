//! Windows preparation resolves Client membership to an explicit Workspace source.
use crate::cli::source_context as context;
use crate::{
    cli::{
        Failure, FailureKind,
        caller_context::PanelTarget,
        client_control::{selection, windows as control},
        source::{ArmedSignals, SignalWatch},
    },
    invocation::Target,
    local_client::LocalClient,
};
use orchbee_workbench_protocol::{
    ClientViewAction, ClientViewOutcome, ClientViewPhase, ClientViewRecord, Snapshot,
    WorkbenchClientId, WorkbenchDirectoryPath, WorkbenchProjectId, WorktreeSource,
};
use std::time::Instant;

pub(in crate::cli) struct Request<'a> {
    pub project: &'a Option<Target>,
    pub workspace: &'a Option<Target>,
    pub cwd: &'a Option<WorkbenchDirectoryPath>,
    pub client: Option<&'a WorkbenchClientId>,
    pub focus: bool,
}
pub(in crate::cli) struct Prepared {
    pub project: Option<WorkbenchProjectId>,
    pub source: WorktreeSource,
    pub client: Option<ClientViewRecord>,
}

pub(in crate::cli) fn prepare(
    request: Request<'_>,
    client: &mut LocalClient,
    snapshot: &Snapshot,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<Prepared, Failure> {
    check(signals, deadline)?;
    let implicit =
        request.workspace.is_none() && request.cwd.is_none() && request.project.is_none();
    let caller = if implicit && request.client.is_none() {
        PanelTarget::prepare_optional(|name| std::env::var_os(name))?
            .map(|caller| caller.resolve(snapshot))
            .transpose()?
    } else {
        None
    };
    let selected = if request.client.is_some() || request.focus || (implicit && caller.is_none()) {
        let reply = control::exchange(
            client,
            &snapshot.site_id,
            ClientViewAction::List {},
            deadline,
            signals,
        )?;
        check(signals, deadline)?;
        let clients = match reply.outcome {
            ClientViewOutcome::Listed { clients } => clients,
            ClientViewOutcome::Denied {} => return Err(Failure::new(FailureKind::Denied)),
            ClientViewOutcome::Unavailable { .. } => {
                return Err(Failure::new(FailureKind::ServiceUnavailable));
            }
            ClientViewOutcome::Synchronized { .. }
            | ClientViewOutcome::Found { .. }
            | ClientViewOutcome::Operation { .. }
            | ClientViewOutcome::NotFound { .. }
            | ClientViewOutcome::Conflict { .. } => {
                return Err(Failure::new(FailureKind::ProtocolInvalid));
            }
        };
        if clients.is_empty() && request.client.is_none() && !request.focus && caller.is_none() {
            None
        } else {
            let selected = selection::select(&clients, request.client, caller.as_ref())?.clone();
            if selected.view.phase != ClientViewPhase::Ready {
                return Err(Failure::new(FailureKind::ServiceUnavailable));
            }
            Some(selected)
        }
    } else {
        None
    };
    let origin = if request.cwd.is_none() {
        context::resolve(snapshot, caller.as_ref(), selected.as_ref())?
    } else {
        None
    };
    let workspace = request.workspace.clone().or_else(|| {
        if implicit {
            origin.as_ref().map(|origin| origin.workspace.clone())
        } else {
            None
        }
    });
    let (project, source) = super::resolve(
        request.project,
        &workspace,
        request.cwd,
        snapshot,
        std::env::current_dir,
    )?;
    Ok(Prepared {
        project,
        source,
        client: selected,
    })
}

pub(in crate::cli) fn check(signals: &mut ArmedSignals, deadline: Instant) -> Result<(), Failure> {
    if signals
        .take()
        .map_err(|_| Failure::new(FailureKind::InternalFailure))?
        .is_some()
    {
        Err(Failure::new(FailureKind::Cancelled))
    } else if Instant::now() >= deadline {
        Err(Failure::new(FailureKind::Timeout))
    } else {
        Ok(())
    }
}
