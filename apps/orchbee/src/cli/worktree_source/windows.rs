//! Windows preparation owns Client discovery and the one proven root-process cwd read.
use crate::cli::source_context as context;
use crate::{
    cli::{
        Failure, FailureKind,
        caller_context::PanelTarget,
        client_control::{selection, windows as control},
        query,
        source::{ArmedSignals, SignalWatch},
    },
    invocation::Target,
    local_client::LocalClient,
};
use orchbee_workbench_protocol::{
    ClientViewAction, ClientViewOutcome, ClientViewPhase, ClientViewRecord, Snapshot,
    WorkbenchClientId, WorkbenchDirectoryPath, WorkbenchProjectId, WorktreeSource,
};
use std::{path::Path, time::Instant};

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
            _ => return Err(Failure::new(FailureKind::ProtocolInvalid)),
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
            origin.as_ref().and_then(|origin| origin.workspace.clone())
        } else {
            None
        }
    });
    let mut cwd = request.cwd.clone();
    if implicit
        && workspace.is_none()
        && let Some(origin) = &origin
    {
        cwd = Some(root_directory(
            client,
            snapshot,
            origin.panel.as_ref(),
            deadline,
            signals,
        )?);
    }
    let (project, source) = super::resolve(
        request.project,
        &workspace,
        &cwd,
        snapshot,
        std::env::current_dir,
    )?;
    Ok(Prepared {
        project,
        source,
        client: selected,
    })
}

fn root_directory(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    panel: Option<&Target>,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<WorkbenchDirectoryPath, Failure> {
    check(signals, deadline)?;
    let panel = panel.ok_or_else(|| Failure::new(FailureKind::NotFound))?;
    let terminal = query::primary_terminal(snapshot, panel)?;
    let sample = query::read_terminal_processes_cancellable(
        client,
        deadline,
        snapshot,
        &terminal,
        signals.poll_fd(),
    );
    check(signals, deadline)?;
    let (_, sample) = sample?;
    let path = sample
        .info
        .processes
        .iter()
        .find(|process| process.pid == sample.info.root_pid)
        .and_then(|process| process.current_directory.as_deref())
        .filter(|path| Path::new(path).is_absolute())
        .ok_or_else(|| Failure::new(FailureKind::ServiceUnavailable))?;
    WorkbenchDirectoryPath::parse(path).map_err(|_| Failure::new(FailureKind::ProtocolInvalid))
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
