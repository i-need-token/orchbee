//! `machine connect` and `machine disconnect`: the machine hub of this data root carries the
//! Machine, and the command asks it through one attachment that holds and watches nothing. An
//! interrupt ends the wait, not the hub's attempt. A hub closing under the request is asked once
//! more, through the hub that takes its place.

#[cfg(not(windows))]
use super::{Context, Error, Report};
#[cfg(not(windows))]
use crate::machines::Store;

#[cfg(windows)]
pub(super) use windows::{connect, disconnect};

#[cfg(not(windows))]
pub(super) fn connect(
    _store: &Store,
    _selector: &str,
    _context: &mut Context,
    _report: &mut Report,
) -> Result<(), Error> {
    Err(Error::Internal)
}

#[cfg(not(windows))]
pub(super) fn disconnect(
    _store: &Store,
    _selector: &str,
    _context: &mut Context,
    _report: &mut Report,
) -> Result<(), Error> {
    Err(Error::Internal)
}

#[cfg(windows)]
mod windows {
    use super::super::{Context, Error, Remote, Report};
    use crate::machines::{Error as CatalogError, Store};
    use orchbee_machine_protocol::{
        MachineId,
        hub::{
            Connect, ConnectFailure, ConnectOutcome, ConnectResult, DisconnectOutcome, Origin,
            Reply, Request,
        },
    };
    use orchbee_workbench_client::application::machine_hub::{
        AttachFailure, Attachment, EndFailure, HubClient, RequestFailure, data_root, running,
    };
    use std::{
        sync::{Arc, mpsc},
        thread,
        time::Duration,
    };

    /// How often a wait looks for an interrupt.
    const POLL: Duration = Duration::from_millis(50);
    /// Requests per command: the first, and one through the hub that replaces a closing one.
    const ROUNDS: usize = 2;

    /// Connects the Machine `selector` names and returns once the hub reports it `online` or the
    /// reason it is not.
    pub(in super::super) fn connect(
        store: &Store,
        selector: &str,
        context: &mut Context,
        report: &mut Report,
    ) -> Result<(), Error> {
        let machine = resolve(store, selector, report)?;
        context.check()?;
        let client = open(context)?;
        let result = ask_connect(&client, &machine, context, report);
        close(client);
        result
    }

    fn ask_connect(
        client: &Arc<HubClient>,
        machine: &MachineId,
        context: &mut Context,
        report: &mut Report,
    ) -> Result<(), Error> {
        for round in 1..=ROUNDS {
            // The hub reads the catalog this command resolved, so a machine saved after the hub
            // started is known to it; an unchanged record keeps its connection.
            match ask(client, Request::Reload, context)? {
                Ok(_) | Err(RequestFailure::Lost) => {}
                Err(failure) => return Err(request_error(failure)),
            }
            report.remote = Remote::PossiblyStarted;
            let request = Request::Connect(Connect {
                machine: machine.clone(),
                origin: Origin::Command { answers: false },
                draft: None,
            });
            match ask(client, request, context)? {
                Ok(Reply::Connect(ConnectResult {
                    outcome: ConnectOutcome::Online { remote_version },
                    ..
                })) => {
                    report.remote = Remote::Ready;
                    report.remote_version = Some(remote_version.as_str().to_owned());
                    return Ok(());
                }
                Ok(Reply::Connect(ConnectResult {
                    outcome: ConnectOutcome::NotOnline(ConnectFailure::Stopping),
                    ..
                }))
                | Err(RequestFailure::Lost)
                    if round < ROUNDS => {}
                Ok(Reply::Connect(ConnectResult {
                    outcome: ConnectOutcome::NotOnline(failure),
                    ..
                })) => {
                    report.remote = remote_after(failure);
                    return Err(connect_error(failure));
                }
                Ok(_) => return Err(Error::Internal),
                Err(failure) => {
                    if !matches!(failure, RequestFailure::Lost | RequestFailure::Timeout) {
                        report.remote = Remote::NotContacted;
                    }
                    return Err(request_error(failure));
                }
            }
        }
        Err(Error::HubUnavailable)
    }

    /// Ends the hub's connection to the Machine `selector` names. With no hub running there is no
    /// connection, and none is started.
    pub(in super::super) fn disconnect(
        store: &Store,
        selector: &str,
        context: &mut Context,
        report: &mut Report,
    ) -> Result<(), Error> {
        let machine = resolve(store, selector, report)?;
        context.check()?;
        let root = data_root().map_err(attach_error)?;
        let running = wait(context, move |deadline| running(&root, deadline))?;
        if running.map_err(end_error)?.is_none() {
            return Ok(());
        }
        let client = open(context)?;
        let result = ask_disconnect(&client, &machine, context, report);
        close(client);
        result
    }

    fn ask_disconnect(
        client: &Arc<HubClient>,
        machine: &MachineId,
        context: &mut Context,
        report: &mut Report,
    ) -> Result<(), Error> {
        for round in 1..=ROUNDS {
            let request = Request::Disconnect {
                machine: machine.clone(),
            };
            match ask(client, request, context)? {
                // A hub whose catalog does not list the machine carries nothing for it.
                Ok(Reply::Disconnect(
                    DisconnectOutcome::Disconnected | DisconnectOutcome::NotFound,
                )) => {
                    report.remote = Remote::Unchanged;
                    return Ok(());
                }
                Ok(Reply::Disconnect(DisconnectOutcome::Stopping)) | Err(RequestFailure::Lost)
                    if round < ROUNDS => {}
                // A closing hub ends every connection it carries.
                Ok(Reply::Disconnect(DisconnectOutcome::Stopping)) => {
                    report.remote = Remote::Unchanged;
                    return Ok(());
                }
                Ok(_) => return Err(Error::Internal),
                Err(failure) => return Err(request_error(failure)),
            }
        }
        Err(Error::HubUnavailable)
    }

    /// The saved machine `selector` names, recorded in the report with the catalog it was read
    /// from.
    fn resolve(store: &Store, selector: &str, report: &mut Report) -> Result<MachineId, Error> {
        let catalog = store.read().map_err(Error::Catalog)?;
        let id = catalog
            .resolve(selector)
            .map_err(Error::Catalog)?
            .id
            .clone();
        let machine = MachineId::new(id.as_str()).map_err(|_| Error::Internal)?;
        report.id = Some(id);
        report.catalog = Some(catalog);
        Ok(machine)
    }

    /// Attaches to the hub of this data root, starting one when none runs.
    fn open(context: &mut Context) -> Result<Arc<HubClient>, Error> {
        let attachment = Attachment {
            presence: false,
            asks: false,
            watch: false,
        };
        let (client, _attached) = wait(context, move |deadline| {
            HubClient::open(attachment, deadline)
        })?
        .map_err(attach_error)?;
        Ok(Arc::new(client))
    }

    /// Says goodbye to the hub unless an abandoned wait still holds the attachment; the process
    /// ending then closes it.
    fn close(client: Arc<HubClient>) {
        if let Ok(client) = Arc::try_unwrap(client) {
            client.close();
        }
    }

    fn ask(
        client: &Arc<HubClient>,
        request: Request,
        context: &mut Context,
    ) -> Result<Result<Reply, RequestFailure>, Error> {
        let client = Arc::clone(client);
        wait(context, move |deadline| client.request(request, deadline))
    }

    /// Runs `work` against the command's deadline on its own thread and waits for it, looking for
    /// an interrupt meanwhile. An interrupt abandons the work, which ends at the deadline or with
    /// the process.
    fn wait<T: Send + 'static>(
        context: &mut Context,
        work: impl FnOnce(std::time::Instant) -> T + Send + 'static,
    ) -> Result<T, Error> {
        let (sender, receiver) = mpsc::channel();
        let deadline = context.deadline;
        thread::Builder::new()
            .name("machine-command".to_owned())
            .spawn(move || {
                let _ = sender.send(work(deadline));
            })
            .map_err(|_| Error::Internal)?;
        loop {
            match receiver.recv_timeout(POLL) {
                Ok(value) => return Ok(value),
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if context.interrupted().map_err(|_| Error::Internal)? {
                        return Err(Error::Cancelled);
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => return Err(Error::Internal),
            }
        }
    }

    /// What the remote side may hold after a connect that did not reach `online`.
    const fn remote_after(failure: ConnectFailure) -> Remote {
        match failure {
            ConnectFailure::RemoteNotReady
            | ConnectFailure::Timeout
            | ConnectFailure::Cancelled => Remote::PossiblyStarted,
            ConnectFailure::NotFound
            | ConnectFailure::DisabledMachine
            | ConnectFailure::UnsupportedMachine
            | ConnectFailure::SshUnavailable
            | ConnectFailure::InvalidDraft
            | ConnectFailure::Stopping
            | ConnectFailure::Internal
            | ConnectFailure::Unknown => Remote::NotContacted,
            _ => Remote::Unchanged,
        }
    }

    const fn connect_error(failure: ConnectFailure) -> Error {
        match failure {
            ConnectFailure::DisabledMachine => Error::DisabledMachine,
            ConnectFailure::UnsupportedMachine => Error::UnsupportedMachine,
            ConnectFailure::HostUnreachable => Error::HostUnreachable,
            ConnectFailure::SshUnavailable => Error::SshUnavailable,
            ConnectFailure::SshFailed => Error::SshFailed,
            ConnectFailure::RemoteProtocol => Error::RemoteProtocol,
            ConnectFailure::RemoteNotReady => Error::RemoteNotReady,
            ConnectFailure::AuthRequired => Error::AuthRequired,
            ConnectFailure::AuthFailed => Error::AuthFailed,
            ConnectFailure::HostUntrusted => Error::HostUntrusted,
            ConnectFailure::HostKeyChanged => Error::HostKeyChanged,
            ConnectFailure::RemoteNotInstalled => Error::RemoteNotInstalled,
            ConnectFailure::RemoteUnsupported => Error::RemoteUnsupported,
            ConnectFailure::VersionIncompatible => Error::VersionIncompatible,
            ConnectFailure::RemoteOtherEnvironment => Error::RemoteOtherEnvironment,
            ConnectFailure::NotFound => Error::Catalog(CatalogError::NotFound),
            ConnectFailure::Cancelled => Error::Disconnected,
            ConnectFailure::Timeout => Error::Timeout,
            ConnectFailure::RemoteInstallFailed => Error::RemoteInstallFailed,
            ConnectFailure::Stopping => Error::HubUnavailable,
            ConnectFailure::InvalidDraft | ConnectFailure::Internal | ConnectFailure::Unknown => {
                Error::Internal
            }
        }
    }

    fn request_error(failure: RequestFailure) -> Error {
        match failure {
            RequestFailure::Timeout => Error::Timeout,
            RequestFailure::Lost => Error::HubUnavailable,
            RequestFailure::Ended(failure) => attach_error(failure),
            RequestFailure::Unsendable | RequestFailure::Unsupported => Error::Internal,
        }
    }

    fn attach_error(failure: AttachFailure) -> Error {
        match failure {
            AttachFailure::DataRoot => Error::Catalog(CatalogError::Configuration),
            AttachFailure::Incompatible(_) => Error::VersionIncompatible,
            AttachFailure::Timeout => Error::Timeout,
            AttachFailure::Unavailable
            | AttachFailure::Denied
            | AttachFailure::Closing
            | AttachFailure::Lost => Error::HubUnavailable,
        }
    }

    fn end_error(failure: EndFailure) -> Error {
        match failure {
            EndFailure::Foreign { .. } => Error::VersionIncompatible,
            EndFailure::Timeout { .. } => Error::Timeout,
            EndFailure::Denied | EndFailure::Unavailable => Error::HubUnavailable,
        }
    }
}
