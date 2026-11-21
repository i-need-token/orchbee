//! `machine connect` and `machine disconnect`: the machine hub of this data root carries the
//! Machine, and the command asks it through one attachment that holds and watches nothing, after
//! the hub has taken the current catalog. An interrupt ends the wait, not the hub's attempt. A hub
//! closing under the request is asked once more, through the hub that takes its place. Every
//! committed catalog edit also asks a running hub to take the catalog; it and `machine disconnect`
//! reach only the hub running now and start, replace and wait for none. A routed
//! command and a remote stop have the hub take the catalog the same way and bring their Machine
//! `online` through the same steps only when the hub answers that it is not.
//!
//! `machine add` runs the shared add pipeline through the same attachment: the new Machine is a
//! hub draft until it reads `online`, and only then is the record saved and the hub asked to take
//! the catalog, which keeps the draft's link. `machine remove --stop-session` stops the remote
//! Session the way a remote stop does before the record goes, and keeps the record when it does
//! not stop.

mod asking;
mod remember;

pub(super) use requests::{
    add, ask, ask_connect, ask_reload, close, connect, disconnect, open, reload_running,
    stop_session, wait, watch_present,
};

mod requests {
    use super::{
        super::{Context, Error, Remote, Report},
        asking::{self, Console},
        remember::Remember,
    };
    use crate::{
        cli_text::CliText,
        localization::Locale,
        machines::{Catalog, Connection, Draft, Error as CatalogError, ProfileId, Store},
    };
    use orchbee_machine_protocol::{
        MachineId,
        hub::{
            Connect, ConnectFailure, ConnectOutcome, ConnectResult, DisconnectOutcome, Origin,
            ReloadOutcome, Reply, Request, StopSessionOutcome,
        },
    };
    use orchbee_workbench_client::application::{
        machine_add::Proposal,
        machine_hub::{AttachFailure, Attachment, HubClient, OpenCancel, Opening, RequestFailure},
    };
    use std::{
        sync::{Arc, mpsc},
        thread,
        time::{Duration, Instant},
    };

    /// How often a wait looks for an interrupt.
    const POLL: Duration = Duration::from_millis(50);
    /// Requests per command: the first, and one through the hub that replaces a closing one.
    const ROUNDS: usize = 2;
    /// How long the hub may take to start, take the catalog and bring a Machine `online`.
    const CONNECT: Duration = Duration::from_secs(90);

    /// Adds the machine `draft` describes: the hub connects it as a draft, asking the login
    /// questions on the console when there is one, and the record is saved once it is `online`.
    /// The hub then takes the catalog before this attachment closes, so the link stays; a draft
    /// that never came `online` is dropped as the attachment closes, and nothing was saved.
    pub(in super::super) fn add(
        store: &Store,
        draft: &Draft,
        context: &mut Context,
        report: &mut Report,
        locale: Locale,
    ) -> Result<(), Error> {
        let catalog = store.read().map_err(Error::Catalog)?;
        let proposal = Proposal::new(draft.clone(), &catalog).map_err(Error::Catalog)?;
        report.id = Some(proposal.id.clone());
        let console = draft
            .ssh_target()
            .and_then(|target| Console::detect(target.to_owned(), locale, None));
        let waiting = CliText::new(locale).cli_machine_waiting(&draft.label);
        context.check()?;
        let client = open(context)?;
        let result = add_through(
            &client,
            store,
            &proposal,
            console.as_ref(),
            &waiting,
            context,
            report,
        );
        close(client);
        result
    }

    fn add_through(
        client: &Arc<HubClient>,
        store: &Store,
        proposal: &Proposal,
        console: Option<&Console>,
        waiting: &str,
        context: &mut Context,
        report: &mut Report,
    ) -> Result<(), Error> {
        let machine = proposal.machine().ok_or(Error::Internal)?;
        let mut waiting = asking::Waited::new(waiting);
        for round in 1..=ROUNDS {
            let request = proposal
                .request(Origin::Command {
                    answers: console.is_some(),
                })
                .ok_or(Error::Internal)?;
            report.remote = Remote::PossiblyStarted;
            let answered = asking::converse(
                client,
                request,
                &machine,
                console,
                &mut waiting,
                context,
                &mut report.remote,
            )?;
            match answered {
                Ok(Reply::Connect(ConnectResult {
                    outcome: ConnectOutcome::Online { remote_version },
                    ..
                })) => {
                    report.remote = Remote::Ready;
                    report.remote_version = Some(remote_version.as_str().to_owned());
                    let (catalog, changed) = proposal.save(store).map_err(Error::Catalog)?;
                    report.catalog = Some(catalog);
                    report.changed = changed;
                    report.committed = changed;
                    // The hub keeps the link once it holds the record; the record stands anyway.
                    return match reload(client, context) {
                        Err(Error::Cancelled) => Err(Error::Cancelled),
                        _ => Ok(()),
                    };
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

    /// Stops the remote Session of the confirmed Machine `id`, which `catalog` lists, before its
    /// record is removed; it never resolves a selector again. A Machine the hub does not carry
    /// `online` is brought `online` under the `machine connect` rules and asked once more. Only
    /// `stopped` and `not-running` let the removal go on.
    pub(in super::super) fn stop_session(
        id: &ProfileId,
        catalog: Catalog,
        context: &mut Context,
        report: &mut Report,
        locale: Locale,
    ) -> Result<(), Error> {
        let machine = MachineId::new(id.as_str()).map_err(|_| Error::Internal)?;
        report.id = Some(id.clone());
        report.catalog = Some(catalog);
        let waiting = CliText::new(locale).cli_machine_waiting(&label(report));
        context.deadline = Instant::now() + CONNECT;
        context.check()?;
        let client = open(context)?;
        let result = stop_through(&client, &machine, &waiting, context);
        close(client);
        result
    }

    /// The `machine remove --stop-session` reading of the routed stop's outcome: only `stopped`
    /// and `not-running` let the removal go on.
    fn stop_through(
        client: &Arc<HubClient>,
        machine: &MachineId,
        waiting: &str,
        context: &mut Context,
    ) -> Result<(), Error> {
        let stopped = super::super::routing::stop::hub::carry(client, machine, waiting, context)?;
        match stopped.outcome {
            _ if stopped.interrupted => Err(Error::Cancelled),
            StopSessionOutcome::Stopped | StopSessionOutcome::NotRunning => Ok(()),
            // The Machine came `online` and went before the second ask.
            StopSessionOutcome::NotOnline => Err(Error::HostUnreachable),
            StopSessionOutcome::Stopping => Err(Error::HubUnavailable),
            StopSessionOutcome::NotFound => Err(Error::Catalog(CatalogError::NotFound)),
            StopSessionOutcome::Failed => Err(Error::SessionStopFailed),
            // The request may have reached the Session: only an answer settles it.
            StopSessionOutcome::Unknown => Err(Error::PhysicalUnknown),
        }
    }

    /// Connects the Machine `selector` names and returns once the hub reports it `online` or the
    /// reason it is not. With standard input and standard error both consoles, the command asks
    /// the login questions on them unless an interface that asks is attached to the hub, and with
    /// `remember` it remembers a password or passphrase typed there once the login succeeds.
    pub(in super::super) fn connect(
        store: &Store,
        selector: &str,
        remember: bool,
        context: &mut Context,
        report: &mut Report,
        locale: Locale,
    ) -> Result<(), Error> {
        let machine = resolve(store, selector, report)?;
        let remembering = remember.then(|| Remember::new(machine.clone(), &label(report)));
        let console =
            target(report).and_then(|target| Console::detect(target, locale, remembering));
        let waiting = CliText::new(locale).cli_machine_waiting(&label(report));
        context.check()?;
        let client = open(context)?;
        let result = connect_through(
            &client,
            &machine,
            console.as_ref(),
            &waiting,
            context,
            report,
        );
        close(client);
        result
    }

    /// Brings the Machine `machine` `online` through `client` under the `machine connect` rules,
    /// asking nobody here: the hub takes the current catalog first, a hub closing under the
    /// request is asked once more through the one that takes its place, and a login the hub
    /// offers to an interface writes `waiting` and waits for the person there.
    pub(in super::super) fn ask_connect(
        client: &Arc<HubClient>,
        machine: &MachineId,
        waiting: &str,
        context: &mut Context,
        report: &mut Report,
    ) -> Result<(), Error> {
        connect_through(client, machine, None, waiting, context, report)
    }

    /// `ask_connect`, with `answers` and the questions asked on `console` when there is one.
    fn connect_through(
        client: &Arc<HubClient>,
        machine: &MachineId,
        console: Option<&Console>,
        waiting: &str,
        context: &mut Context,
        report: &mut Report,
    ) -> Result<(), Error> {
        // The line is written once per command, whichever request the hub makes wait.
        let mut waiting = asking::Waited::new(waiting);
        for round in 1..=ROUNDS {
            // The hub reads the catalog this command resolved, so a machine saved after the hub
            // started is known to it; an unchanged record keeps its connection.
            match reload(client, context)? {
                Reloaded::Current => {}
                Reloaded::Stopping | Reloaded::Lost => continue,
                Reloaded::Gone => return Err(Error::HubUnavailable),
            }
            report.remote = Remote::PossiblyStarted;
            let request = Request::Connect(Connect {
                machine: machine.clone(),
                origin: Origin::Command {
                    answers: console.is_some(),
                },
                draft: None,
            });
            let answered = asking::converse(
                client,
                request,
                machine,
                console,
                &mut waiting,
                context,
                &mut report.remote,
            )?;
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
                Reloaded::Gone => return Err(Error::HubUnavailable),
            }
        }
        Err(Error::HubUnavailable)
    }

    /// Ends the hub's connection to the Machine `selector` names. It reaches only the hub running
    /// now: with none, or only a closing one, there is no connection, and none is started. A
    /// running hub first takes the current catalog, so a record that was removed or changed
    /// without telling it ends there before the selector resolves.
    pub(in super::super) fn disconnect(
        store: &Store,
        selector: &str,
        context: &mut Context,
        report: &mut Report,
    ) -> Result<(), Error> {
        context.check()?;
        let Some(client) = open_present(context)? else {
            resolve(store, selector, report)?;
            return Ok(());
        };
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
                // A closing hub, or one gone with none in its place, ends every connection it
                // carried.
                Reloaded::Stopping | Reloaded::Gone => {
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
                // A closing hub, or one gone with none in its place, ends every connection it
                // carried.
                Ok(Reply::Disconnect(DisconnectOutcome::Stopping))
                | Err(RequestFailure::Ended(AttachFailure::Closing)) => {
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
        /// The owner ended `closing`: no hub runs any more, and an owner that reaches only the hub
        /// present starts none.
        Gone,
    }

    /// Asks the hub to take the current catalog. A hub that could not read it keeps the one it
    /// had and would act on a record that may since have changed, so that ends the command
    /// before it asks for anything else.
    fn reload(client: &Arc<HubClient>, context: &mut Context) -> Result<Reloaded, Error> {
        match ask(client, Request::Reload, context)? {
            Ok(Reply::Reload(ReloadOutcome::Reloaded { .. })) => Ok(Reloaded::Current),
            Ok(Reply::Reload(ReloadOutcome::Stopping)) => Ok(Reloaded::Stopping),
            Err(RequestFailure::Lost) => Ok(Reloaded::Lost),
            Err(RequestFailure::Ended(AttachFailure::Closing)) => Ok(Reloaded::Gone),
            Ok(Reply::Reload(ReloadOutcome::InvalidCatalog)) => {
                Err(Error::Catalog(CatalogError::InvalidCatalog))
            }
            Ok(Reply::Reload(ReloadOutcome::Storage)) => Err(Error::Catalog(CatalogError::Storage)),
            Ok(_) => Err(Error::Internal),
            Err(failure) => Err(request_error(failure)),
        }
    }

    /// After a committed catalog edit, asks the hub of this data root to take the catalog when
    /// one runs; it starts, replaces and waits for none, and a hub that serves later reads the
    /// edit at its start. The edit stands whatever the hub answers, so only an interrupt ends the
    /// command differently.
    pub(in super::super) fn reload_running(context: &mut Context) -> Result<(), Error> {
        let interrupted = |error: Error| match error {
            Error::Cancelled => Err(Error::Cancelled),
            _ => Ok(()),
        };
        let client = match open_present(context) {
            Ok(Some(client)) => client,
            Ok(None) => return Ok(()),
            Err(error) => return interrupted(error),
        };
        let asked = ask(&client, Request::Reload, context);
        close(client);
        asked.map(drop).or_else(interrupted)
    }

    /// The label of the machine the report resolved.
    fn label(report: &Report) -> String {
        report
            .id
            .as_ref()
            .and_then(|id| report.catalog.as_ref()?.resolve(id.as_str()).ok())
            .map(|profile| profile.draft.label.clone())
            .unwrap_or_default()
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

    /// Attaches to the hub serving this data root now, as one that holds and watches nothing;
    /// `None` when none does or it is closing. It starts, replaces and waits for no hub, and the
    /// owner ends instead of starting one when that hub goes.
    fn open_present(context: &mut Context) -> Result<Option<Arc<HubClient>>, Error> {
        present(context, false)
    }

    /// Attaches as [`open_present`] does, watching the catalog and every Machine's state.
    pub(in super::super) fn watch_present(
        context: &mut Context,
    ) -> Result<Option<Arc<HubClient>>, Error> {
        present(context, true)
    }

    fn present(context: &mut Context, watch: bool) -> Result<Option<Arc<HubClient>>, Error> {
        let attachment = Attachment {
            presence: false,
            asks: false,
            watch,
        };
        match wait(context, move |deadline| {
            HubClient::open_with(
                attachment,
                deadline,
                Opening::Present,
                &OpenCancel::default(),
            )
        })? {
            Ok((client, _attached)) => Ok(Some(Arc::new(client))),
            Err(AttachFailure::Closing) => Ok(None),
            Err(failure) => Err(attach_error(failure)),
        }
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
    pub(in super::super) fn wait<T: Send + 'static>(
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
            AttachFailure::Elevated => Error::HubElevated,
            AttachFailure::Unavailable
            | AttachFailure::Denied
            | AttachFailure::Closing
            | AttachFailure::Lost => Error::HubUnavailable,
        }
    }
}
