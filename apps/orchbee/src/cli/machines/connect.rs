//! `machine connect` and `machine disconnect`: the machine hub of this data root carries the
//! Machine, and the command asks it through one attachment that holds and watches nothing, after
//! the hub has taken the current catalog. An interrupt ends the wait, not the hub's attempt. A hub
//! closing under the request is asked once more, through the hub that takes its place. Every
//! committed catalog edit also asks a running hub to take the catalog, starting none. A routed
//! command and a remote stop have the hub take the catalog the same way and bring their Machine
//! `online` through the same steps only when the hub answers that it is not.

#[cfg(not(windows))]
use super::{Context, Error, Report};
#[cfg(not(windows))]
use crate::{localization::Locale, machines::Store};

#[cfg(windows)]
mod asking;

#[cfg(windows)]
pub(super) use windows::{
    ask, ask_connect, ask_reload, close, connect, disconnect, open, reload_running,
};

/// No hub runs where it is not built, so an edit has nothing to tell.
#[cfg(not(windows))]
pub(super) fn reload_running(_context: &mut Context) -> Result<(), Error> {
    Ok(())
}

#[cfg(not(windows))]
pub(super) fn connect(
    _store: &Store,
    _selector: &str,
    _context: &mut Context,
    _report: &mut Report,
    _locale: Locale,
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
    use super::{
        super::{Context, Error, Remote, Report},
        asking::{self, Console},
    };
    use crate::{
        localization::Locale,
        machines::{Connection, Error as CatalogError, Store},
    };
    use orchbee_machine_protocol::{
        MachineId,
        hub::{
            Connect, ConnectFailure, ConnectOutcome, ConnectResult, DisconnectOutcome, Origin,
            ReloadOutcome, Reply, Request,
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
    /// reason it is not. With standard input and standard error both consoles, the command asks
    /// the login questions on them.
    pub(in super::super) fn connect(
        store: &Store,
        selector: &str,
        context: &mut Context,
        report: &mut Report,
        locale: Locale,
    ) -> Result<(), Error> {
        let machine = resolve(store, selector, report)?;
        let console = target(report).and_then(|target| Console::detect(target, locale));
        context.check()?;
        let client = open(context)?;
        let result = connect_through(&client, &machine, console.as_ref(), context, report);
        close(client);
        result
    }

    /// Brings the Machine `machine` `online` through `client` under the `machine connect` rules,
    /// asking nobody: the hub takes the current catalog first, and a hub closing under the request
    /// is asked once more through the one that takes its place.
    pub(in super::super) fn ask_connect(
        client: &Arc<HubClient>,
        machine: &MachineId,
        context: &mut Context,
        report: &mut Report,
    ) -> Result<(), Error> {
        connect_through(client, machine, None, context, report)
    }

    /// `ask_connect`, with `answers` and the questions asked on `console` when there is one.
    fn connect_through(
        client: &Arc<HubClient>,
        machine: &MachineId,
        console: Option<&Console>,
        context: &mut Context,
        report: &mut Report,
    ) -> Result<(), Error> {
        for round in 1..=ROUNDS {
            // The hub reads the catalog this command resolved, so a machine saved after the hub
            // started is known to it; an unchanged record keeps its connection.
            match reload(client, context)? {
                Reloaded::Current => {}
                Reloaded::Stopping | Reloaded::Lost => continue,
            }
            report.remote = Remote::PossiblyStarted;
            let request = Request::Connect(Connect {
                machine: machine.clone(),
                origin: Origin::Command {
                    answers: console.is_some(),
                },
                draft: None,
            });
            let answered = match console {
                Some(console) => asking::converse(
                    client,
                    request,
                    machine,
                    console,
                    context,
                    &mut report.remote,
                )?,
                None => ask(client, request, context)?,
            };
            match answered {
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

    /// Has the hub take the current catalog, asking once more through the hub that replaces a
    /// closing one. It connects nothing, so the hub's countdown stays where it was.
    pub(in super::super) fn ask_reload(
        client: &Arc<HubClient>,
        context: &mut Context,
    ) -> Result<(), Error> {
        for _ in 0..ROUNDS {
            match reload(client, context)? {
                Reloaded::Current => return Ok(()),
                Reloaded::Stopping | Reloaded::Lost => {}
            }
        }
        Err(Error::HubUnavailable)
    }

    /// Ends the hub's connection to the Machine `selector` names. With no hub running there is no
    /// connection, and none is started. A running hub first takes the current catalog, so a record
    /// that was removed or changed without telling it ends there before the selector resolves.
    pub(in super::super) fn disconnect(
        store: &Store,
        selector: &str,
        context: &mut Context,
        report: &mut Report,
    ) -> Result<(), Error> {
        context.check()?;
        let root = data_root().map_err(attach_error)?;
        let running = wait(context, move |deadline| running(&root, deadline))?;
        if running.map_err(end_error)?.is_none() {
            resolve(store, selector, report)?;
            return Ok(());
        }
        let client = open(context)?;
        let result = ask_disconnect(&client, store, selector, context, report);
        close(client);
        result
    }

    fn ask_disconnect(
        client: &Arc<HubClient>,
        store: &Store,
        selector: &str,
        context: &mut Context,
        report: &mut Report,
    ) -> Result<(), Error> {
        for round in 1..=ROUNDS {
            match reload(client, context)? {
                Reloaded::Current => {}
                Reloaded::Stopping | Reloaded::Lost if round < ROUNDS => continue,
                // A closing hub ends every connection it carries.
                Reloaded::Stopping => {
                    resolve(store, selector, report)?;
                    report.remote = Remote::Unchanged;
                    return Ok(());
                }
                Reloaded::Lost => return Err(Error::HubUnavailable),
            }
            let machine = resolve(store, selector, report)?;
            let request = Request::Disconnect { machine };
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

    /// How the hub answered `reload`, when it answered in a way the command goes on from.
    enum Reloaded {
        /// The hub serves the catalog as it is now.
        Current,
        /// The hub is closing; the one that takes its place reads the catalog at its start.
        Stopping,
        /// The connection was lost with the request in flight.
        Lost,
    }

    /// Asks the hub to take the current catalog. A hub that could not read it keeps the one it
    /// had and would act on a record that may since have changed, so that ends the command
    /// before it asks for anything else.
    fn reload(client: &Arc<HubClient>, context: &mut Context) -> Result<Reloaded, Error> {
        match ask(client, Request::Reload, context)? {
            Ok(Reply::Reload(ReloadOutcome::Reloaded { .. })) => Ok(Reloaded::Current),
            Ok(Reply::Reload(ReloadOutcome::Stopping)) => Ok(Reloaded::Stopping),
            Err(RequestFailure::Lost) => Ok(Reloaded::Lost),
            Ok(Reply::Reload(ReloadOutcome::InvalidCatalog)) => {
                Err(Error::Catalog(CatalogError::InvalidCatalog))
            }
            Ok(Reply::Reload(ReloadOutcome::Storage)) => Err(Error::Catalog(CatalogError::Storage)),
            Ok(_) => Err(Error::Internal),
            Err(failure) => Err(request_error(failure)),
        }
    }

    /// After a committed catalog edit, asks the hub of this data root to take the catalog when
    /// one runs; it starts none. The edit stands whatever the hub answers, so only an interrupt
    /// ends the command differently.
    pub(in super::super) fn reload_running(context: &mut Context) -> Result<(), Error> {
        let interrupted = |error: Error| match error {
            Error::Cancelled => Err(Error::Cancelled),
            _ => Ok(()),
        };
        let Ok(root) = data_root() else {
            return Ok(());
        };
        match wait(context, move |deadline| running(&root, deadline)) {
            Ok(Ok(Some(_))) => {}
            Ok(_) => return Ok(()),
            Err(error) => return interrupted(error),
        }
        let client = match open(context) {
            Ok(client) => client,
            Err(error) => return interrupted(error),
        };
        let asked = ask(&client, Request::Reload, context);
        close(client);
        asked.map(drop).or_else(interrupted)
    }

    /// The `ssh` target of the machine the report resolved.
    fn target(report: &Report) -> Option<String> {
        let id = report.id.as_ref()?;
        match &report
            .catalog
            .as_ref()?
            .resolve(id.as_str())
            .ok()?
            .draft
            .connection
        {
            Connection::Ssh { target } => Some(target.clone()),
            Connection::Unsupported(_) => None,
        }
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
    pub(in super::super) fn open(context: &mut Context) -> Result<Arc<HubClient>, Error> {
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
    pub(in super::super) fn close(client: Arc<HubClient>) {
        if let Ok(client) = Arc::try_unwrap(client) {
            client.close();
        }
    }

    /// Sends `request` through `client` and waits for its answer under the command's deadline.
    /// `Err` is an interrupt, which abandons the wait, or a failure to wait at all.
    pub(in super::super) fn ask(
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
            ConnectFailure::Cancelled => Error::AttemptEnded,
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
