//! One carrier: the child that carries a Machine's `machine-link` connection on its standard input
//! and output, started inside a Job only the hub holds. Its owner thread starts it, waits for its
//! root process, settles the Job and reaps it, and reports the end after its reader, writer and
//! standard-error threads have ended. The reader hands each link event to the Machine, and each
//! channel frame to the relay, under the hub state's lock and pauses while 256 frames other than
//! `data` wait to be written; the writer writes the offer at once and then the link's frames.
//! `docs/machines.md#the-carrier-child` owns the rules.

use super::super::{
    askpass::{Pairing, TICKET_BYTES, VARIABLE},
    shared_carrier::frames,
};
use super::{
    environment::{Environment, StandIn},
    machine::{End, Failure, Route, refusal},
    state::Shared,
};
use orchbee_connection_handshake::{
    AcceptorMessage, Build, ConnectionKind, InitiatorMessage, Offer, ProductVersion, ReadError,
    encode_initiator_frame, read_acceptor_message,
};
use orchbee_local_runtime::CommandChild;
use orchbee_machine_protocol::{
    ByeReason, Connection, Delivery, Frame, FrameDecoder, MachineId, Message, SessionName,
    hub::OfflineCause,
    link::{self, Channel, Link, Outcome},
};
use orchbee_workbench_client::application::machine_ssh;
use std::{
    ffi::OsString,
    io::{self, PipeReader, PipeWriter, Read, Write},
    path::PathBuf,
    process::Command,
    sync::{Arc, OnceLock},
    thread,
    time::Duration,
};
use winsafe::{HPROCESS, co, guard::CloseHandleGuard};

/// The one argument a stand-in runs with.
const LINK_ENTRY: &str = "--machine-link-v1";
/// The most standard-error bytes kept to classify an end.
const STDERR_TAIL: usize = 4 * 1024;
/// One read of the link.
const READ_BUFFER_BYTES: usize = 64 * 1024;
/// The reader stops reading while this many frames other than `data` wait to be written.
const BACKLOG: usize = 256;
/// The most bytes of whole frames one write carries.
const WRITE_BATCH_BYTES: usize = 64 * 1024;
/// How long one wait for the root process lasts before the owner waits again.
const ROOT_WAIT: Duration = Duration::from_secs(3600);
/// The exit status the hub gives a carrier it ends.
const ENDED_BY_HUB: u32 = 1;
/// The `DISPLAY` an `ssh` older than 8.4 needs before it runs an askpass program. Such an `ssh`
/// still carries typed answers; it is never given a remembered one, see [`Launch::recalls`].
const DISPLAY: &str = "orchbee:0";

/// One link's state under its own lock, and the signal that wakes its writer.
pub(super) type LinkLine = super::super::shared_carrier::Shared<LinkWire>;

pub(super) struct LinkWire {
    pub(super) connection: Connection<Link>,
    /// False once the carrier is ending; its writer stops and closes the child's input.
    pub(super) open: bool,
    /// The `ensure-session` channel once opened.
    ensure: Option<u32>,
    /// `ping` payloads given out, so each differs.
    pings: u64,
}

/// A carrier as the hub state knows it, from its start until its owner reports its end.
pub(super) struct Handle {
    pub(super) machine: MachineId,
    /// The ticket its askpass helpers bring; all zeros, with which the carrier does not start, when
    /// no random bytes could be drawn.
    pub(super) ticket: [u8; TICKET_BYTES],
    line: Arc<LinkLine>,
    /// The root process, held to end it; set once the child exists.
    root: Option<CloseHandleGuard<HPROCESS>>,
    ending: bool,
}

impl Handle {
    pub(super) fn new(machine: MachineId) -> Self {
        let mut ticket = [0; TICKET_BYTES];
        if getrandom::fill(&mut ticket).is_err() {
            ticket = [0; TICKET_BYTES];
        }
        Self {
            machine,
            ticket,
            line: Arc::new(LinkLine::new(LinkWire {
                connection: Connection::<Link>::initiator(),
                open: true,
                ensure: None,
                pings: 0,
            })),
            root: None,
            ending: false,
        }
    }

    /// Ends the carrier: its writer stops and its root process is terminated, after which the
    /// owner ends the rest of the Job. A carrier still starting is ended once its child exists.
    pub(super) fn end(&mut self) {
        self.ending = true;
        self.line.lock().open = false;
        self.line.wake();
        if let Some(root) = &self.root {
            // A root that already exited refuses; its owner is settling the Job either way.
            let _terminated = root.TerminateProcess(ENDED_BY_HUB);
        }
    }

    /// The link's state and its writer's signal.
    pub(super) const fn line(&self) -> &Arc<LinkLine> {
        &self.line
    }

    /// Opens `ensure-session` for `session`.
    pub(super) fn ensure_session(&self, session: SessionName) {
        let mut wire = self.line.lock();
        if wire.open
            && let Ok(channel) = wire.connection.open(Channel::EnsureSession { session })
        {
            wire.ensure = Some(channel);
        }
        drop(wire);
        self.line.wake();
    }

    /// Sends `ping`; a link already waiting for a `pong` sends none.
    pub(super) fn ping(&self) {
        let mut wire = self.line.lock();
        wire.pings += 1;
        let payload = wire.pings.to_be_bytes();
        let _sent = wire.connection.ping(payload);
        drop(wire);
        self.line.wake();
    }

    /// Queues `bye` `stopping` as the link's last frame.
    pub(super) fn farewell(&self) {
        let _queued = self.line.lock().connection.bye(ByeReason::Stopping);
        self.line.wake();
    }
}

/// How every carrier starts: the program, the child environment, the askpass settings, the working
/// folder and the offer.
pub(super) struct Launch {
    stand_in: StandIn,
    environment: Vec<(String, String)>,
    /// This program, which `ssh` runs as its askpass helper.
    askpass: PathBuf,
    /// The hub's askpass endpoint.
    endpoint: String,
    folder: Option<PathBuf>,
    offer: Vec<u8>,
    /// Whether the program is known to be OpenSSH 8.5 or later, once read.
    recalls: OnceLock<bool>,
}

impl Launch {
    /// `None` when this program's own path is unknown, since no carrier may start without the
    /// askpass helper.
    pub(super) fn new(environment: &Environment, endpoint: String) -> Option<Self> {
        let offer = Offer::local(
            ConnectionKind::MachineLink,
            ProductVersion::new(env!("CARGO_PKG_VERSION"))
                .expect("the package version is printable ASCII"),
            Build::default(),
            orchbee_local_runtime::environment_name(),
        );
        Some(Self {
            stand_in: environment.stand_in(),
            environment: environment.child(),
            askpass: std::env::current_exe().ok()?,
            endpoint,
            folder: winsafe::GetSystemDirectory().ok().map(PathBuf::from),
            offer: encode_initiator_frame(&InitiatorMessage::Offer(offer))
                .expect("this build's offer fits one frame"),
            recalls: OnceLock::new(),
        })
    }

    /// Whether a remembered answer may be recalled: the program carriers run, `ssh` or its
    /// stand-in, is known to be OpenSSH 8.5 or later. Before 8.5 a keyboard-interactive prompt
    /// arrives without its `(user@host)` prefix, so a remote could word it as `ssh`'s own password
    /// or passphrase sentence. The program is run once with `-V` alone, in the child environment;
    /// it contacts nothing, and a version it does not print reads as too old.
    pub(super) fn recalls(&self) -> bool {
        *self.recalls.get_or_init(|| {
            let program = match &self.stand_in {
                StandIn::Program(program) => program.clone(),
                StandIn::Unusable => return false,
                StandIn::None => match machine_ssh::executable() {
                    Ok(ssh) => ssh,
                    Err(_) => return false,
                },
            };
            let mut command = Command::new(program);
            command.env_clear();
            command.envs(self.environment.iter().map(|(name, value)| (name, value)));
            if let Some(folder) = &self.folder {
                command.current_dir(folder);
            }
            machine_ssh::version(command).is_some_and(machine_ssh::Version::remembers)
        })
    }

    /// The command toward `route` whose askpass helpers bring `ticket`, or the end it reads as
    /// when it cannot be built.
    fn command(&self, route: &Route, ticket: [u8; TICKET_BYTES]) -> Result<Command, End> {
        let mut command = match &self.stand_in {
            StandIn::Program(program) => {
                let mut command = Command::new(program);
                command.arg(LINK_ENTRY);
                command
            }
            StandIn::Unusable => return Err(End::NotStarted { missing: true }),
            StandIn::None => {
                let ssh = machine_ssh::executable().map_err(|error| End::NotStarted {
                    missing: error.kind() == io::ErrorKind::NotFound,
                })?;
                machine_ssh::carrier_command(&ssh, &route.target, &machine_ssh::link_program())
            }
        };
        // The program's own settings, such as `ssh`'s askpass ones, win over the view.
        let own: Vec<(OsString, Option<OsString>)> = command
            .get_envs()
            .map(|(name, value)| (name.to_owned(), value.map(ToOwned::to_owned)))
            .collect();
        command.env_clear();
        command.envs(self.environment.iter().map(|(name, value)| (name, value)));
        for (name, value) in own {
            match value {
                Some(value) => command.env(name, value),
                None => command.env_remove(name),
            };
        }
        // The askpass helper is the only way a question reaches anyone; it wins over every view.
        let pairing = Pairing {
            endpoint: self.endpoint.clone(),
            ticket,
        };
        command
            .env("SSH_ASKPASS", &self.askpass)
            .env("SSH_ASKPASS_REQUIRE", "force")
            .env("DISPLAY", DISPLAY)
            .env(VARIABLE, pairing.encode());
        if let Some(folder) = &self.folder {
            command.current_dir(folder);
        }
        Ok(command)
    }
}

/// Starts the owner of the carrier `carrier`, whose handle the hub state already holds.
pub(super) fn start(shared: &Arc<Shared>, launch: &Arc<Launch>, carrier: u64, route: Route) {
    let (owner_shared, launch) = (shared.clone(), launch.clone());
    let started = thread::Builder::new()
        .name("orchbee-hub-carrier".into())
        .spawn(move || {
            let end = carry(&owner_shared, &launch, carrier, &route);
            finish(&owner_shared, carrier, &end);
        });
    if started.is_err() {
        finish(shared, carrier, &End::NotStarted { missing: false });
    }
}

/// Runs the carrier until it ends and returns how it ended.
fn carry(shared: &Arc<Shared>, launch: &Launch, carrier: u64, route: &Route) -> End {
    let Some((line, ticket)) = shared
        .lock()
        .carriers
        .get(&carrier)
        .map(|handle| (handle.line.clone(), handle.ticket))
    else {
        return End::NotStarted { missing: false };
    };
    // Without a ticket the hub could not tell this carrier's helpers apart, nor end the carrier
    // of one that cannot ask, so it does not start.
    if ticket == [0; TICKET_BYTES] {
        return End::NotStarted { missing: false };
    }
    let mut command = match launch.command(route, ticket) {
        Ok(command) => command,
        Err(end) => return end,
    };
    let Ok(((child_input, input), (output, child_output), (errors, child_errors))) =
        io::pipe().and_then(|a| Ok((a, io::pipe()?, io::pipe()?)))
    else {
        return End::NotStarted { missing: false };
    };
    command
        .stdin(child_input)
        .stdout(child_output)
        .stderr(child_errors);
    let mut child = match CommandChild::spawn_contained(command) {
        Ok(child) => child,
        Err(failure) => {
            return End::NotStarted {
                missing: !failure.created && failure.error.kind() == io::ErrorKind::NotFound,
            };
        }
    };
    let root = HPROCESS::OpenProcess(
        co::PROCESS::SYNCHRONIZE | co::PROCESS::TERMINATE,
        false,
        child.id(),
    );
    let Ok(root) = root else {
        let code = child.settle().ok().and_then(|status| status.code());
        return End::Exited {
            code,
            stderr: Vec::new(),
        };
    };
    register(shared, carrier, root);
    let threads = start_threads(
        shared,
        carrier,
        &line,
        launch.offer.clone(),
        input,
        output,
        errors,
    );
    if threads.is_none()
        && let Some(handle) = shared.lock().carriers.get_mut(&carrier)
    {
        handle.end();
    }
    while matches!(child.wait_root_exit(ROOT_WAIT), Ok(false)) {}
    let code = child.settle().ok().and_then(|status| status.code());
    line.lock().open = false;
    line.wake();
    // A panic aborts the hub, so a join ends only after the thread returned.
    let stderr = threads.map_or_else(Vec::new, |(writer, reader, tail)| {
        let _written = writer.join();
        let _read = reader.join();
        tail.join().unwrap_or_default()
    });
    End::Exited { code, stderr }
}

/// Gives the hub the root process to end, or ends it at once when the carrier is already ending.
fn register(shared: &Shared, carrier: u64, root: CloseHandleGuard<HPROCESS>) {
    let mut hub = shared.lock();
    match hub.carriers.get_mut(&carrier) {
        Some(handle) if !handle.ending => handle.root = Some(root),
        _ => {
            let _terminated = root.TerminateProcess(ENDED_BY_HUB);
        }
    }
}

type Threads = (
    thread::JoinHandle<()>,
    thread::JoinHandle<()>,
    thread::JoinHandle<Vec<u8>>,
);

fn start_threads(
    shared: &Arc<Shared>,
    carrier: u64,
    line: &Arc<LinkLine>,
    offer: Vec<u8>,
    input: PipeWriter,
    output: PipeReader,
    errors: PipeReader,
) -> Option<Threads> {
    let writer = {
        let line = line.clone();
        thread::Builder::new()
            .name("orchbee-hub-link-writer".into())
            .spawn(move || write(&line, input, &offer))
            .ok()?
    };
    let reader = {
        let (shared, line) = (shared.clone(), line.clone());
        thread::Builder::new()
            .name("orchbee-hub-link-reader".into())
            .spawn(move || read(&shared, carrier, &line, output))
            .ok()?
    };
    let tail = thread::Builder::new()
        .name("orchbee-hub-link-stderr".into())
        .spawn(move || tail(errors))
        .ok()?;
    Some((writer, reader, tail))
}

/// Reports the end of the carrier `carrier` and forgets its handle.
fn finish(shared: &Shared, carrier: u64, end: &End) {
    let mut hub = shared.lock();
    hub.carrier_ended(carrier);
    if let Some(handle) = hub.carriers.remove(&carrier) {
        let machine = handle.machine;
        hub.machine(&machine, |machine, cx| machine.ended(carrier, end, cx));
    }
    drop(hub);
    shared.wake();
}

/// The writer: the offer at once, then the link's frames until the carrier ends.
fn write(line: &LinkLine, mut input: PipeWriter, offer: &[u8]) {
    if input.write_all(offer).is_err() {
        return;
    }
    loop {
        let bytes = {
            let mut wire = line.lock();
            loop {
                let mut bytes = Vec::new();
                while wire.open && bytes.len() < WRITE_BATCH_BYTES {
                    match wire.connection.poll_transmit() {
                        Some(frame) => bytes.extend_from_slice(&frame.encode()),
                        None => break,
                    }
                }
                if !bytes.is_empty() {
                    break Some(bytes);
                }
                if !wire.open || wire.connection.is_finished() {
                    break None;
                }
                wire = line.wait(wire);
            }
        };
        // A reader paused at the backlog may read again.
        line.wake();
        let Some(bytes) = bytes else {
            return;
        };
        if input.write_all(&bytes).is_err() {
            // Nothing will drain the queue, so a reader paused at the backlog stops too.
            line.lock().open = false;
            line.wake();
            return;
        }
    }
}

/// The reader: the helper's `OBHS` answer, then the link's frames until its output ends. It
/// pauses while the replies the helper provoked wait to be written, so a helper that stops reading
/// cannot grow the link's queue.
fn read(shared: &Shared, carrier: u64, line: &LinkLine, mut output: PipeReader) {
    let answer = read_acceptor_message(&mut output);
    let answered = {
        let mut hub = shared.lock();
        match answer {
            Ok(AcceptorMessage::Answer(answer)) => {
                let version = answer.product_version;
                hub.machine_of(carrier, |machine, _| machine.answered(carrier, version));
                true
            }
            Ok(AcceptorMessage::Rejection(rejection)) => {
                hub.machine_of(carrier, |machine, cx| {
                    machine.failed(carrier, refusal(rejection.reason), cx)
                });
                false
            }
            Err(ReadError::Closed | ReadError::Io(_)) => {
                hub.machine_of(carrier, |machine, cx| machine.closed(carrier, cx));
                false
            }
            Ok(_) | Err(ReadError::ForeignProtocol | ReadError::Malformed(_)) => {
                hub.machine_of(carrier, |machine, cx| {
                    machine.failed(carrier, Failure::Offline(OfflineCause::RemoteProtocol), cx)
                });
                false
            }
        }
    };
    shared.wake();
    if !answered {
        return;
    }
    let mut decoder = FrameDecoder::new();
    let mut buffer = vec![0; READ_BUFFER_BYTES];
    loop {
        {
            let mut wire = line.lock();
            while wire.open && wire.connection.queued_frames() >= BACKLOG {
                wire = line.wait(wire);
            }
            // The carrier is ending; its owner reports the end.
            if !wire.open {
                break;
            }
        }
        let read = match output.read(&mut buffer) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Ok(read) => read,
            Err(_) => 0,
        };
        if read == 0 {
            break;
        }
        let bytes = buffer.get(..read).unwrap_or_default();
        match frames(&mut decoder, bytes, |frame| {
            receive(shared, carrier, line, frame)
        }) {
            Ok(true) => {}
            Ok(false) => break,
            Err(_) => {
                let _violation = line.lock().connection.frame_failed();
                line.wake();
                break;
            }
        }
    }
    shared
        .lock()
        .machine_of(carrier, |machine, cx| machine.broken(carrier, cx));
    shared.wake();
}

/// Takes one frame; false once the link stops reading.
fn receive(shared: &Shared, carrier: u64, line: &LinkLine, frame: &Frame) -> bool {
    let (delivery, ensure) = {
        let mut wire = line.lock();
        match wire.connection.receive(frame) {
            Ok(receipt) => (receipt.delivery, wire.ensure),
            Err(_) => {
                drop(wire);
                line.wake();
                return false;
            }
        }
    };
    // The state machine may owe the helper a `pong` or credit.
    line.wake();
    let Some(delivery) = delivery else {
        return true;
    };
    let mut hub = shared.lock();
    let reading = match delivery {
        Delivery::Message(Message::Event {
            event: link::Event::Facts(facts),
            ..
        }) => {
            hub.machine_of(carrier, |machine, cx| {
                machine.facts(carrier, facts.platform, facts.architecture, cx)
            });
            true
        }
        Delivery::Close {
            channel, outcome, ..
        } if Some(channel) == ensure => {
            let ready = matches!(outcome, Outcome::Ready { .. });
            hub.machine_of(carrier, |machine, cx| machine.session(carrier, ready, cx));
            true
        }
        Delivery::Reset { channel, .. } if Some(channel) == ensure => {
            hub.machine_of(carrier, |machine, cx| machine.session(carrier, false, cx));
            true
        }
        Delivery::Pong => {
            hub.machine_of(carrier, |machine, _| machine.pong(carrier));
            true
        }
        Delivery::Message(Message::Bye { .. }) => false,
        Delivery::Accept { channel, .. } => {
            hub.link_accepted(carrier, channel);
            true
        }
        Delivery::Data { channel, bytes } => {
            hub.link_data(carrier, channel, bytes);
            true
        }
        Delivery::Writable { channel } => {
            hub.link_writable(carrier, channel);
            true
        }
        Delivery::End { channel } => {
            hub.link_end(carrier, channel);
            true
        }
        Delivery::Close {
            channel, outcome, ..
        } => {
            hub.link_closed(carrier, channel, outcome);
            true
        }
        Delivery::Reset {
            channel, reason, ..
        } => {
            hub.link_reset(carrier, channel, reason);
            true
        }
        _ => true,
    };
    drop(hub);
    shared.wake();
    reading
}

/// Keeps the last bytes of the child's standard error until it ends.
fn tail(mut errors: PipeReader) -> Vec<u8> {
    let mut kept = Vec::with_capacity(STDERR_TAIL);
    let mut buffer = [0; 4096];
    loop {
        match errors.read(&mut buffer) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Ok(0) | Err(_) => return kept,
            Ok(read) => {
                kept.extend_from_slice(buffer.get(..read).unwrap_or_default());
                let excess = kept.len().saturating_sub(STDERR_TAIL);
                kept.drain(..excess);
            }
        }
    }
}
