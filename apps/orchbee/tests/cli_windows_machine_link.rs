//! The remote helper `--machine-link-v1` started as a child process with pipes against the real
//! scratch runtime: this test speaks the hub's end with the codec, on this computer only. Real CLI
//! control attachments reach the default Session through `session` channels, a Session endpoint
//! that floods while its channel is left unread holds no routed command back, `ensure-session`
//! finds one Session and starts another, and a link ends by `bye` or by its pipe closing while the
//! Sessions keep running. A console on the helper's standard input or output refuses its start.
#![cfg(windows)]

#[allow(dead_code)]
#[path = "support/windows_tui_session.rs"]
mod session;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use orchbee_connection_handshake::{
    AcceptorMessage, Build, ConnectionKind, InitiatorMessage, Offer, ProductVersion,
    encode_initiator_frame, read_acceptor_message,
};
use orchbee_local_ipc::{LocalListener, LocalStream};
use orchbee_local_runtime::{CohortArtifact, ModeCheckedLocalSessionRendezvous, SessionScope};
use orchbee_machine_protocol::{
    ByeReason, Connection, Delivery, FrameDecoder, INITIAL_CREDIT, Message, ResetReason,
    SessionName, link,
};
use serde_json::{Value, json};
use session::Fixture;
use std::{
    collections::BTreeMap,
    fs,
    io::{self, BufRead, BufReader, Read, Write},
    net::Shutdown,
    path::{Path, PathBuf},
    process::{Child, ChildStdin, ChildStdout, Stdio},
    sync::{
        Arc, Condvar, Mutex, MutexGuard,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

/// Bounds each wait for the helper or a Session.
const WAIT: Duration = Duration::from_secs(20);
/// Bounds a Session start through `ensure-session`, past the helper's own 30 seconds.
const START: Duration = Duration::from_secs(40);
/// What the flooding endpoint offers its channel.
const FLOOD_BYTES: usize = 4 * 1024 * 1024;

#[test]
fn one_link_carries_sessions_a_route_past_a_stalled_stream_and_session_starts() {
    let fixture = Fixture::new("ml3-link-", &[]);
    let alpha = terminal_of(&fixture, &fixture.workspace_in("alpha", "work"));
    let beta = terminal_of(&fixture, &fixture.workspace_in("beta", "work"));

    let mut hub = Hub::start(&fixture);
    let facts = hub.until(WAIT, "facts", |hub| hub.facts.clone());
    assert_eq!(facts.platform.as_str(), "windows");
    assert_eq!(facts.architecture.as_str(), std::env::consts::ARCH);
    assert!(
        facts
            .executable
            .as_str()
            .ends_with(&CohortArtifact::Client.file_name()),
        "{}",
        facts.executable.as_str()
    );
    assert!(facts.store_root.is_some(), "the store root resolves");

    // Two real control attachments reach the default Session, each through its own channel.
    let proxy = Proxy::start(&fixture, &hub);
    let mut first = Attachment::start(&fixture, &proxy.base, &alpha);
    let mut second = Attachment::start(&fixture, &proxy.base, &beta);
    first.expect_echo("LINK-ALPHA-7");
    second.expect_echo("LINK-BETA-9");
    let relayed = proxy.channels();
    assert_eq!(relayed.len(), 2, "one channel per attachment");

    // An endpoint that floods while the hub never takes its bytes holds exactly one window.
    let flood = Flood::start(&fixture);
    let flooded = hub.open(link::Channel::Session {
        session: SessionName::new("flood").unwrap(),
    });
    let window = usize::try_from(INITIAL_CREDIT).unwrap();
    hub.until(WAIT, "the flood's window", |hub| {
        (hub.lanes[&flooded].received.len() == window).then_some(())
    });
    thread::sleep(Duration::from_millis(500));
    let route_started = Instant::now();
    let route = hub.open(link::Channel::Route);
    let packet = json!({"version": 1, "session": "default",
        "arguments": ["workspace", "list", "--format=json"], "input": null,
        "locale": "en_US.UTF-8"});
    hub.send(route, &serde_json::to_vec(&packet).unwrap());
    hub.end(route);
    let outcome = hub.until(WAIT, "the route's close", |hub| hub.lanes[&route].outcome);
    let route_elapsed = route_started.elapsed();
    assert_eq!(outcome, link::Outcome::Replied);
    let reply: Value = serde_json::from_slice(&hub.received(route)).unwrap();
    assert_eq!(reply["outcome"], "completed", "{reply}");
    assert_eq!(reply["status"], 0, "{reply}");
    let listed =
        String::from_utf8(STANDARD.decode(reply["stdout"].as_str().unwrap()).unwrap()).unwrap();
    assert!(
        listed.contains("\"alpha\"") && listed.contains("\"beta\""),
        "{listed}"
    );
    let held = hub.lock().lanes[&flooded].received.len();
    let offered = flood.written.load(Ordering::Acquire);
    assert_eq!(held, window, "the unread channel stays at its window");
    assert!(offered < FLOOD_BYTES, "the flood is held back: {offered}");
    eprintln!(
        "stall: unread channel held {held} bytes, endpoint wrote {offered} of {FLOOD_BYTES}; \
         route reply {} bytes after {route_elapsed:?}",
        hub.received(route).len()
    );
    // The attachments still exchange bytes while the flood stays stalled.
    first.expect_echo("LINK-ALPHA-11");

    // A Session that is not running, and a name the platform refuses.
    let absent = hub.open(link::Channel::Session {
        session: SessionName::new("absent").unwrap(),
    });
    let reset = hub.until(WAIT, "the absent Session's reset", |hub| {
        hub.lanes[&absent].reset
    });
    assert_eq!(reset, ResetReason::SessionNotRunning);

    // `ensure-session` finds the running default Session and starts a fresh one.
    let started = Instant::now();
    let existing = hub.open(link::Channel::EnsureSession {
        session: SessionName::new("default").unwrap(),
    });
    let found = hub.until(WAIT, "the existing Session", |hub| {
        hub.lanes[&existing].outcome
    });
    assert_eq!(found, link::Outcome::Ready { started: false });
    let found_after = started.elapsed();
    let started = Instant::now();
    let fresh = hub.open(link::Channel::EnsureSession {
        session: SessionName::new("fresh").unwrap(),
    });
    let made = hub.until(START, "the fresh Session", |hub| hub.lanes[&fresh].outcome);
    assert_eq!(made, link::Outcome::Ready { started: true });
    eprintln!(
        "ensure-session: existing after {found_after:?}, started after {:?}",
        started.elapsed()
    );

    // `bye` ends the link: the helper answers `done` and exits 0.
    hub.bye();
    let (status, farewell) = hub.finish(WAIT);
    assert_eq!(status, Some(0));
    assert_eq!(farewell, Some(ByeReason::Done));
    drop(first);
    drop(second);
    drop(proxy);
    drop(flood);

    // A second link whose pipe closes with a Session channel open: the helper exits and the
    // Session, its Worker and terminals keep running.
    let mut hub = Hub::start(&fixture);
    hub.until(WAIT, "facts", |hub| hub.facts.clone());
    let open = hub.open(link::Channel::Session {
        session: SessionName::new("default").unwrap(),
    });
    hub.until(WAIT, "the accept", |hub| {
        hub.lanes[&open].accepted.then_some(())
    });
    let closed = Instant::now();
    let (status, _) = hub.close(WAIT);
    assert_eq!(status, Some(0));
    eprintln!("pipe closed: helper exited after {:?}", closed.elapsed());
    let terminals = fixture.run(&["terminal", "list"]);
    assert_eq!(terminals.code, Some(0), "{terminals}");
    for terminal in [&alpha, &beta] {
        assert!(
            terminals.document["result"]["terminals"]
                .as_array()
                .unwrap()
                .iter()
                .any(|entry| entry["terminalId"] == terminal.as_str()),
            "{terminals}"
        );
    }
    let mut after = Attachment::start(&fixture, &fixture.path("runtime"), &alpha);
    after.expect_echo("LINK-AFTER-13");
    drop(after);

    let stopped = fixture.run(&["server", "stop", "--confirm", "--session", "fresh"]);
    assert_eq!(stopped.code, Some(0), "{stopped}");

    console_refuses(&fixture);
}

/// A console on standard input, on standard output, or on both refuses the start, writing nothing.
fn console_refuses(fixture: &Fixture) {
    let client = fixture.path(&format!("cohort/{}", CohortArtifact::Client.file_name()));
    let report = |case: &str| fixture.path(&format!("console-{case}.txt"));
    let output = fixture.path("console-output.bin");
    let launcher = PathBuf::from(fixture.path("console.cmd"));
    fs::write(
        &launcher,
        format!(
            // The redirection leads each `echo`: `echo 5>file` would redirect handle 5.
            "@echo off\r\n\"{client}\" --machine-link-v1\r\n>\"{}\" echo %errorlevel%\r\n\
             echo x| \"{client}\" --machine-link-v1\r\n>\"{}\" echo %errorlevel%\r\n\
             \"{client}\" --machine-link-v1 >\"{output}\"\r\n>\"{}\" echo %errorlevel%\r\n",
            report("both"),
            report("output"),
            report("input"),
        ),
    )
    .unwrap();
    let shell = session::system_program("cmd.exe");
    let work = fixture.path("work");
    let mut words = vec![
        "workspace",
        "create",
        "--label",
        "console",
        "--cwd",
        &work,
        "--no-focus",
        "--exec",
        &shell,
        "--arg",
        "/d",
        "--arg",
        "/c",
        "--arg",
        launcher.to_str().unwrap(),
    ];
    let environment = fixture.terminal_environment();
    words.extend(environment.iter().map(String::as_str));
    let created = fixture.run(&words);
    assert_eq!(created.code, Some(0), "{created}");
    let deadline = Instant::now() + WAIT;
    for case in ["both", "output", "input"] {
        let status = loop {
            if let Ok(text) = fs::read_to_string(report(case))
                && !text.trim().is_empty()
            {
                break text.trim().to_owned();
            }
            assert!(Instant::now() < deadline, "the {case} case never reported");
            thread::sleep(Duration::from_millis(100));
        };
        assert_eq!(status, "5", "a console on {case} refuses the start");
    }
    assert_eq!(
        fs::read(&output).unwrap(),
        b"",
        "nothing reaches standard output"
    );
}

/// The terminal bound to a creation result's Panel.
fn terminal_of(fixture: &Fixture, created: &Value) -> String {
    let listed = fixture.run(&["terminal", "list"]);
    listed.document["result"]["terminals"]
        .as_array()
        .and_then(|terminals| {
            terminals
                .iter()
                .find(|terminal| terminal["boundTo"]["panelId"] == created["panelId"])
        })
        .and_then(|terminal| terminal["terminalId"].as_str())
        .unwrap_or_else(|| panic!("{listed}"))
        .to_owned()
}

/// What the hub's end has seen of one channel.
#[derive(Default)]
struct Lane {
    accepted: bool,
    /// Bytes received and kept: never taken unless `take` is set.
    received: Vec<u8>,
    /// Received bytes are taken at once, as a reading consumer would.
    take: bool,
    /// A proxied Client connection the channel's bytes go to, taken once written.
    sink: Option<LocalStream>,
    outcome: Option<link::Outcome>,
    reset: Option<ResetReason>,
}

/// The hub's end of the link.
struct HubState {
    connection: Connection<link::Link>,
    facts: Option<link::Facts>,
    lanes: BTreeMap<u32, Lane>,
    farewell: Option<ByeReason>,
    /// The helper's standard output ended.
    output_closed: bool,
    /// The hub stops writing and closes the helper's standard input.
    input_closed: bool,
}

type Shared = Arc<(Mutex<HubState>, Condvar)>;

/// The helper process and the threads that speak the hub's end to it.
struct Hub {
    child: Child,
    shared: Shared,
    threads: Vec<thread::JoinHandle<()>>,
}

impl Hub {
    /// Starts the helper and completes the `OBHS` exchange.
    fn start(fixture: &Fixture) -> Self {
        let mut child = fixture
            .command(&["--machine-link-v1"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut stdin = child.stdin.take().unwrap();
        let mut stdout = child.stdout.take().unwrap();
        let offer = Offer::local(
            ConnectionKind::MachineLink,
            ProductVersion::new(env!("CARGO_PKG_VERSION")).unwrap(),
            Build::default(),
            orchbee_local_runtime::environment_name(),
        );
        stdin
            .write_all(&encode_initiator_frame(&InitiatorMessage::Offer(offer)).unwrap())
            .unwrap();
        stdin.flush().unwrap();
        let answer = read_acceptor_message(&mut stdout).unwrap();
        assert!(
            matches!(answer, AcceptorMessage::Answer(_)),
            "the helper answers the offer: {answer:?}"
        );
        let shared: Shared = Arc::new((
            Mutex::new(HubState {
                connection: Connection::<link::Link>::initiator(),
                facts: None,
                lanes: BTreeMap::new(),
                farewell: None,
                output_closed: false,
                input_closed: false,
            }),
            Condvar::new(),
        ));
        let reader = {
            let shared = shared.clone();
            thread::spawn(move || read_link(&shared, stdout))
        };
        let writer = {
            let shared = shared.clone();
            thread::spawn(move || write_link(&shared, stdin))
        };
        Self {
            child,
            shared,
            threads: vec![reader, writer],
        }
    }

    fn lock(&self) -> MutexGuard<'_, HubState> {
        self.shared.0.lock().unwrap()
    }

    /// Waits until `ready` returns a value.
    fn until<T>(
        &self,
        within: Duration,
        what: &str,
        mut ready: impl FnMut(&HubState) -> Option<T>,
    ) -> T {
        let deadline = Instant::now() + within;
        let mut state = self.lock();
        loop {
            if let Some(value) = ready(&state) {
                return value;
            }
            assert!(Instant::now() < deadline, "timed out waiting for {what}");
            state = self
                .shared
                .1
                .wait_timeout(state, deadline.saturating_duration_since(Instant::now()))
                .unwrap()
                .0;
        }
    }

    fn open(&self, channel: link::Channel) -> u32 {
        open_lane(&self.shared, channel, None)
    }

    /// Sends `bytes` on `channel` within the helper's credit.
    fn send(&self, channel: u32, bytes: &[u8]) {
        send_lane(&self.shared, channel, bytes);
    }

    fn end(&self, channel: u32) {
        self.lock().connection.end(channel).unwrap();
        self.shared.1.notify_all();
    }

    fn received(&self, channel: u32) -> Vec<u8> {
        self.lock().lanes[&channel].received.clone()
    }

    fn bye(&self) {
        self.lock().connection.bye(ByeReason::Done).unwrap();
        self.shared.1.notify_all();
    }

    /// Waits for the helper to end the link and exit; then closes its input.
    fn finish(&mut self, within: Duration) -> (Option<i32>, Option<ByeReason>) {
        self.until(within, "the helper's output to end", |hub| {
            hub.output_closed.then_some(())
        });
        let status = self.exit(within);
        let farewell = self.lock().farewell;
        (status, farewell)
    }

    /// Closes the helper's input, then waits for it to exit.
    fn close(&mut self, within: Duration) -> (Option<i32>, Option<ByeReason>) {
        self.lock().input_closed = true;
        self.shared.1.notify_all();
        let status = self.exit(within);
        let farewell = self.lock().farewell;
        (status, farewell)
    }

    fn exit(&mut self, within: Duration) -> Option<i32> {
        let deadline = Instant::now() + within;
        let status = loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                break status;
            }
            assert!(Instant::now() < deadline, "the helper did not exit");
            thread::sleep(Duration::from_millis(20));
        };
        self.lock().input_closed = true;
        self.shared.1.notify_all();
        for thread in self.threads.drain(..) {
            thread.join().unwrap();
        }
        status.code()
    }
}

impl Drop for Hub {
    fn drop(&mut self) {
        if !self.threads.is_empty() {
            let _closed = self.close(WAIT);
        }
    }
}

fn open_lane(shared: &Shared, channel: link::Channel, sink: Option<LocalStream>) -> u32 {
    let mut state = shared.0.lock().unwrap();
    let id = state.connection.open(channel).unwrap();
    state.lanes.insert(
        id,
        Lane {
            take: sink.is_some(),
            sink,
            ..Lane::default()
        },
    );
    shared.1.notify_all();
    id
}

fn send_lane(shared: &Shared, channel: u32, mut bytes: &[u8]) -> bool {
    let mut state = shared.0.lock().unwrap();
    while !bytes.is_empty() {
        let lane = state.lanes.get(&channel);
        if lane.is_none_or(|lane| lane.outcome.is_some() || lane.reset.is_some()) {
            return false;
        }
        match state.connection.write(channel, bytes) {
            Ok(0) => state = shared.1.wait(state).unwrap(),
            Ok(written) => {
                bytes = &bytes[written..];
                shared.1.notify_all();
            }
            Err(_) => return false,
        }
    }
    true
}

/// Reads the helper's standard output: every byte must belong to an `OBMX` frame.
fn read_link(shared: &Shared, mut stdout: ChildStdout) {
    let mut decoder = FrameDecoder::new();
    let mut buffer = vec![0; 65536];
    while let Ok(read) = stdout.read(&mut buffer)
        && read > 0
    {
        let mut offset = 0;
        while offset < read {
            let (consumed, frame) = decoder
                .decode(&buffer[offset..read])
                .expect("the helper writes only OBMX frames");
            offset += consumed;
            if let Some(frame) = frame {
                take_frame(shared, &frame);
            }
        }
    }
    shared.0.lock().unwrap().output_closed = true;
    shared.1.notify_all();
}

fn take_frame(shared: &Shared, frame: &orchbee_machine_protocol::Frame) {
    let mut state = shared.0.lock().unwrap();
    let receipt = state
        .connection
        .receive(frame)
        .expect("the helper keeps the contract");
    let mut forward = None;
    match receipt.delivery {
        Some(Delivery::Message(Message::Event {
            event: link::Event::Facts(facts),
            ..
        })) => state.facts = Some(facts),
        Some(Delivery::Message(Message::Bye { reason, .. })) => state.farewell = Some(reason),
        Some(Delivery::Accept { channel, .. }) => {
            if let Some(lane) = state.lanes.get_mut(&channel) {
                lane.accepted = true;
            }
        }
        Some(Delivery::Data { channel, bytes }) => {
            let length = bytes.len();
            let lane = state.lanes.get_mut(&channel).unwrap();
            if let Some(sink) = &lane.sink {
                forward = Some((channel, sink.try_clone().unwrap(), bytes));
            } else {
                lane.received.extend_from_slice(&bytes);
                if lane.take {
                    state.connection.take(channel, length).unwrap();
                }
            }
        }
        Some(Delivery::Close {
            channel, outcome, ..
        }) => {
            let lane = state.lanes.get_mut(&channel).unwrap();
            lane.outcome = Some(outcome);
            if let Some(sink) = lane.sink.take() {
                let _closed = sink.shutdown(Shutdown::Both);
            }
        }
        Some(Delivery::Reset {
            channel, reason, ..
        }) => {
            let lane = state.lanes.get_mut(&channel).unwrap();
            lane.reset = Some(reason);
            if let Some(sink) = lane.sink.take() {
                let _closed = sink.shutdown(Shutdown::Both);
            }
        }
        _ => {}
    }
    shared.1.notify_all();
    drop(state);
    if let Some((channel, sink, bytes)) = forward
        && (&sink).write_all(&bytes).is_ok()
    {
        let _taken = shared
            .0
            .lock()
            .unwrap()
            .connection
            .take(channel, bytes.len());
        shared.1.notify_all();
    }
}

/// Writes the hub's frames to the helper's standard input until the hub closes it.
fn write_link(shared: &Shared, mut stdin: ChildStdin) {
    loop {
        let frame = {
            let mut state = shared.0.lock().unwrap();
            loop {
                if state.input_closed {
                    return;
                }
                if let Some(frame) = state.connection.poll_transmit() {
                    break frame;
                }
                state = shared.1.wait(state).unwrap();
            }
        };
        if stdin
            .write_all(&frame.encode())
            .and_then(|()| stdin.flush())
            .is_err()
        {
            return;
        }
    }
}

/// A runtime base whose default Session endpoint relays each Client connection into its own
/// `session` channel of the link.
struct Proxy {
    base: String,
    stop: Arc<AtomicBool>,
    opened: Arc<Mutex<Vec<u32>>>,
    task: Option<thread::JoinHandle<()>>,
}

impl Proxy {
    fn start(fixture: &Fixture, hub: &Hub) -> Self {
        let base = fixture.path("proxy");
        fs::create_dir_all(&base).unwrap();
        let session = ModeCheckedLocalSessionRendezvous::prepare_mode_checked(
            Path::new(&base),
            SessionScope::parse("default").unwrap(),
        )
        .unwrap();
        let listener = LocalListener::bind(session.client_socket_path()).unwrap();
        listener.set_nonblocking(true).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let opened = Arc::new(Mutex::new(Vec::new()));
        let task = {
            let (stop, opened, shared) = (stop.clone(), opened.clone(), hub.shared.clone());
            thread::spawn(move || {
                let _session = session;
                let mut pumps = Vec::new();
                while !stop.load(Ordering::Acquire) {
                    match listener.accept() {
                        Ok((stream, ())) => {
                            let channel = open_lane(
                                &shared,
                                link::Channel::Session {
                                    session: SessionName::new("default").unwrap(),
                                },
                                Some(stream.try_clone().unwrap()),
                            );
                            opened.lock().unwrap().push(channel);
                            let shared = shared.clone();
                            pumps.push(thread::spawn(move || pump(&shared, channel, &stream)));
                        }
                        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(5));
                        }
                        Err(error) => panic!("proxy accept: {error}"),
                    }
                }
                drop(pumps);
            })
        };
        Self {
            base,
            stop,
            opened,
            task: Some(task),
        }
    }

    fn channels(&self) -> Vec<u32> {
        self.opened.lock().unwrap().clone()
    }
}

impl Drop for Proxy {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(task) = self.task.take() {
            let _joined = task.join();
        }
    }
}

/// Sends a proxied Client's bytes into its channel, then `end` once the Client closes.
fn pump(shared: &Shared, channel: u32, stream: &LocalStream) {
    let mut buffer = vec![0; 32 * 1024];
    while let Ok(read) = (&*stream).read(&mut buffer)
        && read > 0
    {
        if !send_lane(shared, channel, &buffer[..read]) {
            return;
        }
    }
    let _ended = shared.0.lock().unwrap().connection.end(channel);
    shared.1.notify_all();
}

/// A fake Session endpoint in the real runtime base that writes as fast as it can.
struct Flood {
    written: Arc<AtomicUsize>,
    task: Option<thread::JoinHandle<()>>,
}

impl Flood {
    fn start(fixture: &Fixture) -> Self {
        let session = ModeCheckedLocalSessionRendezvous::prepare_mode_checked(
            Path::new(&fixture.path("runtime")),
            SessionScope::parse("flood").unwrap(),
        )
        .unwrap();
        let listener = LocalListener::bind(session.client_socket_path()).unwrap();
        listener.set_nonblocking(true).unwrap();
        let written = Arc::new(AtomicUsize::new(0));
        let task = {
            let written = written.clone();
            thread::spawn(move || {
                let _session = session;
                let deadline = Instant::now() + WAIT;
                let (stream, ()) = loop {
                    match listener.accept() {
                        Ok(stream) => break stream,
                        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                            assert!(Instant::now() < deadline, "the helper never connected");
                            thread::sleep(Duration::from_millis(5));
                        }
                        Err(error) => panic!("flood accept: {error}"),
                    }
                };
                let piece = vec![0x5a; 16 * 1024];
                while written.load(Ordering::Acquire) < FLOOD_BYTES {
                    match (&stream).write(&piece) {
                        Ok(count) => {
                            written.fetch_add(count, Ordering::AcqRel);
                        }
                        Err(_) => break,
                    }
                }
            })
        };
        Self {
            written,
            task: Some(task),
        }
    }
}

impl Drop for Flood {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            let _joined = task.join();
        }
    }
}

/// A real `terminal session control` attachment to one terminal of the default Session, reached
/// through the Session endpoint under `base`.
struct Attachment {
    child: Child,
    output: mpsc::Receiver<Vec<u8>>,
    seen: Vec<u8>,
    task: Option<thread::JoinHandle<()>>,
}

impl Attachment {
    fn start(fixture: &Fixture, base: &str, terminal: &str) -> Self {
        let mut child = fixture
            .command(&[
                "terminal",
                "session",
                "control",
                terminal,
                "--takeover",
                "--cols",
                "100",
                "--rows",
                "30",
                "--timeout",
                "120000",
            ])
            .env("ORCHBEE_RUNTIME_BASE", base)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let (sender, output) = mpsc::channel();
        let task = thread::spawn(move || {
            let mut lines = BufReader::new(stdout);
            let mut line = Vec::new();
            while lines
                .read_until(b'\n', &mut line)
                .is_ok_and(|read| read > 0)
            {
                if let Ok(record) = serde_json::from_slice::<Value>(&line) {
                    let bytes = match record["kind"].as_str() {
                        Some("opened") => b"\0opened\0".to_vec(),
                        _ => record["result"]["bytes"]
                            .as_str()
                            .and_then(|text| STANDARD.decode(text).ok())
                            .unwrap_or_default(),
                    };
                    if sender.send(bytes).is_err() {
                        break;
                    }
                }
                line.clear();
            }
        });
        let mut attachment = Self {
            child,
            output,
            seen: Vec::new(),
            task: Some(task),
        };
        attachment.wait_for(b"\0opened\0");
        attachment
    }

    /// Types `echo <marker>` and waits for the marker in the terminal's output.
    fn expect_echo(&mut self, marker: &str) {
        let input = json!({"type": "terminal.input",
            "bytes": STANDARD.encode(format!("echo {marker}\r"))});
        let stdin = self.child.stdin.as_mut().unwrap();
        writeln!(stdin, "{input}").unwrap();
        stdin.flush().unwrap();
        self.wait_for(marker.as_bytes());
    }

    fn wait_for(&mut self, needle: &[u8]) {
        let deadline = Instant::now() + WAIT;
        while !self
            .seen
            .windows(needle.len())
            .any(|window| window == needle)
        {
            let remaining = deadline.saturating_duration_since(Instant::now());
            match self.output.recv_timeout(remaining) {
                Ok(bytes) => self.seen.extend_from_slice(&bytes),
                Err(_) => panic!(
                    "never saw {:?} in {:?}",
                    String::from_utf8_lossy(needle),
                    String::from_utf8_lossy(&self.seen)
                ),
            }
        }
        self.seen.clear();
    }
}

impl Drop for Attachment {
    fn drop(&mut self) {
        // Closing the attachment's input detaches it without stopping the terminal.
        drop(self.child.stdin.take());
        let deadline = Instant::now() + WAIT;
        while matches!(self.child.try_wait(), Ok(None)) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(50));
        }
        if matches!(self.child.try_wait(), Ok(None)) {
            let _killed = self.child.kill();
            let _waited = self.child.wait();
        }
        if let Some(task) = self.task.take() {
            let _joined = task.join();
        }
    }
}
