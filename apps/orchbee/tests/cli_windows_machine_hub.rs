//! The machine hub `--machine-hub-v1` as real processes under scratch data roots, reached through
//! the client owner's diagnostic entries `--machine-hub-state-v1` and `--machine-hub-hold-v1` and
//! through the codec on its endpoint: eight attachments at once reach one hub; `presence` holds it,
//! its leave starts the expiry that a 20-second override ends with `closing` and `bye`, and the next
//! attachment starts a new hub; a hub killed by its process id after a start-time check is replaced
//! by its holder's re-attachment, which restores `presence`; a holder whose hub is asked to stop
//! starts no hub; an endpoint that stays closing fails an attachment as `closing`; a full hub is
//! retried and then reported; a catalog of two machines and its reload; a real Session holds the hub
//! through its stop and a new start; and uninstall's background step ends a hub a Session holds.
//! Through the `ssh` stand-in, the Client under test run as the far end against a real scratch
//! Session, `machine connect` brings a Machine online over one carrier child, which reconnects after
//! it is killed and ends with `machine disconnect` and with the hub; a child receives no `ORCHBEE_*`
//! or terminal variable; and a far end that closes at once is the Machine's state. Every hub runs a
//! stand-in, so no test starts `ssh` or names a reachable machine.
#![cfg(windows)]

#[allow(dead_code)]
#[path = "support/windows_tui_session.rs"]
mod session;

use orchbee_connection_handshake::{
    AcceptorMessage, AcceptorProfile, Build, ConnectionKind, ControlRequest, Extensions,
    InitiatorMessage, Offer, ProductVersion, RejectionReason, RequestKind, encode_acceptor_frame,
    encode_initiator_frame, read_acceptor_message, read_initiator_message,
};
use orchbee_local_ipc::{LocalListener, LocalStream};
use orchbee_local_runtime::{CohortArtifact, DataRoot, PrivateCache, machine_hub_endpoint};
use orchbee_machine_protocol::{
    ByeReason, Connection, Delivery, FrameDecoder, Id, MachineId, Message, ResetReason,
    SessionName, Target,
    hub::{
        self, Attach, AttachOutcome, Channel, ClaimOutcome, Connect, ConnectFailure,
        ConnectOutcome, Draft, Event, HubState, MachineState, MachineStateName, Origin,
        ReloadOutcome, Reply, Request, StopSessionOutcome,
    },
};
use serde_json::json;
use std::{
    ffi::OsString,
    fs,
    io::{self, BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{ChildStdout, Command, Output, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use winsafe::{HPROCESS, HPROCESSLIST, co, guard::CloseHandleGuard};

/// Bounds each wait for the hub.
const WAIT: Duration = Duration::from_secs(20);
/// The expiry override of the expiry exercise.
const SHORT_EXPIRY: &str = "20000";
/// The shortest expiry override, which a hub without `presence` would outlive by little.
const SHORTEST_EXPIRY: &str = "1000";
const FIRST: &str = "11111111111111111111111111111111";
const SECOND: &str = "22222222222222222222222222222222";
const ABSENT: &str = "33333333333333333333333333333333";
/// The development and test program that replaces `ssh`.
const STAND_IN: &str = "ORCHBEE_MACHINE_HUB_SSH_STAND_IN";
/// What a carrier child must never receive beside `ORCHBEE_*`.
const TERMINAL: [&str; 6] = [
    "TERM",
    "COLORTERM",
    "TERM_PROGRAM",
    "TERM_PROGRAM_VERSION",
    "WT_SESSION",
    "WT_PROFILE_ID",
];

#[test]
fn eight_attachments_at_once_reach_one_hub() {
    let scratch = Scratch::new("mh4-eight-", None);
    let started = Instant::now();
    let printers: Vec<_> = (0..8)
        .map(|_| {
            scratch
                .printer()
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect();
    // Every printer ends before any is judged, so a failure leaves none running; the scratch root
    // ends any hub they started.
    let outputs: Vec<Output> = printers
        .into_iter()
        .map(|printer| printer.wait_with_output().unwrap())
        .collect();
    let elapsed = started.elapsed();
    let states: Vec<State> = outputs.iter().map(State::parse).collect();
    let pid = states[0].pid;
    assert!(states.iter().all(|state| state.pid == pid), "{states:?}");
    assert_eq!(scratch.query_pid(), pid);
    let hub = Hub::observe(pid);
    assert!(
        hub.image().ends_with(&CohortArtifact::Client.file_name()),
        "the hub runs as the staged client"
    );
    eprintln!("eight attachments reached hub {pid} in {elapsed:?}");
    scratch.exit_hub();
    hub.wait_exit(WAIT);
}

#[test]
fn presence_holds_and_its_leave_expires_into_closing_and_a_new_hub() {
    let scratch = Scratch::new("mh4-expiry-", Some(SHORT_EXPIRY));
    let first = scratch.print();
    // The printer holds nothing and no Session lives, so the new hub is already expiring.
    assert!(first.hub.starts_with("expiring"), "{first:?}");
    let hub = Hub::observe(first.pid);
    let mut watcher = Client::connect(&scratch.endpoint());
    let attached = watcher.attach(false, true);
    assert!(
        matches!(attached, HubState::Expiring { .. }),
        "{attached:?}"
    );

    let attaching = Instant::now();
    let mut holder = Client::connect(&scratch.endpoint());
    assert_eq!(holder.attach(true, false), HubState::Held);
    watcher.until(WAIT, "held", |client| {
        client.hub_event(|state| *state == HubState::Held)
    });
    let held = attaching.elapsed();
    thread::sleep(Duration::from_secs(1));
    drop(holder);
    let left = Instant::now();
    let expires_at = watcher.until(WAIT, "expiring", |client| {
        client.hub_event(|state| matches!(state, HubState::Expiring { .. }))
    });
    let learned = left.elapsed();
    let HubState::Expiring { expires_at } = expires_at else {
        unreachable!()
    };
    let remaining = expires_at.get().saturating_sub(wall_now());
    assert!(
        (18_000..=20_100).contains(&remaining),
        "expiresAt is the leave plus 20 s: {remaining} ms"
    );
    watcher.until(WAIT + WAIT, "closing", |client| {
        client.hub_event(|state| *state == HubState::Closing)
    });
    let closing = left.elapsed();
    watcher.until(WAIT, "bye", |client| {
        client.bye().filter(|reason| *reason == ByeReason::Stopping)
    });
    watcher.until(WAIT, "the end", |client| client.ended.then_some(()));
    assert_eq!(hub.wait_exit(WAIT), 0);
    let exited = left.elapsed();
    eprintln!(
        "held {held:?} after the attach; expiring {learned:?} after the leave with {remaining} ms \
         left; closing at {closing:?}; exited at {exited:?}"
    );

    let restarted = Instant::now();
    let second = scratch.print();
    assert_ne!(second.pid, first.pid);
    eprintln!(
        "a new hub {} answered in {:?}",
        second.pid,
        restarted.elapsed()
    );
    let next = Hub::observe(second.pid);
    scratch.exit_hub();
    next.wait_exit(WAIT);
}

#[test]
fn a_killed_hub_is_replaced_by_its_holder_whose_presence_returns() {
    let scratch = Scratch::new("mh4-reattach-", Some(SHORTEST_EXPIRY));
    let mut hold = scratch
        .client("--machine-hub-hold-v1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let lines = lines(hold.stdout.take().unwrap());
    let first = attachment(&lines, "attached");
    assert_eq!(first.state, "held", "{first:?}");
    let hub = Hub::observe(first.pid);
    let killed = Instant::now();
    // Stop only the process this test's holder started, and only while it is that process.
    hub.terminate_checked();
    assert_eq!(hub.wait_exit(WAIT), 1);
    let second = attachment(&lines, "reattached");
    let restored = killed.elapsed();
    assert_ne!(second.pid, first.pid);
    assert_eq!(second.state, "held", "{second:?}");
    let next = Hub::observe(second.pid);
    // Past the one-second expiry the new hub is still held: `presence` came back with the holder.
    thread::sleep(Duration::from_secs(2));
    let state = scratch.print();
    assert_eq!((state.hub.as_str(), state.pid), ("held", second.pid));
    eprintln!(
        "killed {}; the holder attached to hub {} with presence in {restored:?}",
        first.pid, second.pid
    );
    drop(hold.stdin.take());
    let held = hold.wait_with_output().unwrap();
    assert_eq!(
        held.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&held.stderr)
    );
    // The holder's leave starts the expiry, which ends the hub on its own.
    assert_eq!(next.wait_exit(WAIT), 0);
}

#[test]
fn a_holder_whose_hub_is_asked_to_stop_starts_no_hub() {
    let scratch = Scratch::new("mh4-stopped-", None);
    let mut hold = scratch
        .client("--machine-hub-hold-v1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let lines = lines(hold.stdout.take().unwrap());
    let first = attachment(&lines, "attached");
    let hub = Hub::observe(first.pid);
    // As uninstall's `end` begins: the hub closes and sends `bye` `stopping` to its holder.
    assert!(matches!(
        request(&scratch.endpoint(), RequestKind::Drain).unwrap(),
        AcceptorMessage::DrainResult(_)
    ));
    assert_eq!(hub.wait_exit(WAIT), 0);
    let deadline = Instant::now() + WAIT + WAIT;
    let status = loop {
        if let Some(status) = hold.try_wait().unwrap() {
            break status;
        }
        assert!(Instant::now() < deadline, "the holder did not end");
        thread::sleep(Duration::from_millis(100));
    };
    let mut printed = Vec::new();
    while let Ok(line) = lines.recv_timeout(WAIT) {
        printed.push(line);
    }
    assert!(!status.success(), "{printed:?}");
    assert!(printed.iter().any(|line| line == "ended"), "{printed:?}");
    assert!(
        !printed.iter().any(|line| line.starts_with("reattached")),
        "{printed:?}"
    );
    assert!(
        !orchbee_local_ipc::endpoint_exists(&scratch.endpoint()).unwrap(),
        "a hub serves the root again"
    );
}

#[test]
fn an_endpoint_that_stays_closing_fails_the_attachment_as_closing() {
    let scratch = Scratch::new("mh4-closing-", None);
    let listener = LocalListener::bind(scratch.endpoint()).unwrap();
    listener.set_nonblocking(true).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let answers = Arc::new(AtomicUsize::new(0));
    // A stand-in for a hub in `closing`: it answers every offer `stopping`, as a closing hub does.
    let server = {
        let (stop, answers) = (stop.clone(), answers.clone());
        thread::spawn(move || {
            let profile = AcceptorProfile::local(
                ConnectionKind::MachineHub,
                ProductVersion::new(env!("CARGO_PKG_VERSION")).unwrap(),
                Build::default(),
                orchbee_local_runtime::environment_name(),
            );
            let refusal = encode_acceptor_frame(&AcceptorMessage::Rejection(
                profile.reject(RejectionReason::Stopping),
            ))
            .unwrap();
            while !stop.load(Ordering::Acquire) {
                let Ok((stream, ())) = listener.accept() else {
                    thread::sleep(Duration::from_millis(10));
                    continue;
                };
                stream.set_nonblocking(false).unwrap();
                stream.set_read_timeout(Some(WAIT)).unwrap();
                if matches!(
                    read_initiator_message(&mut &stream),
                    Ok(InitiatorMessage::Offer(_))
                ) && (&stream).write_all(&refusal).is_ok()
                {
                    answers.fetch_add(1, Ordering::Relaxed);
                }
            }
        })
    };
    let started = Instant::now();
    let output = scratch.printer().output().unwrap();
    let elapsed = started.elapsed();
    stop.store(true, Ordering::Release);
    server.join().unwrap();
    // The state printer's status for `closing`, which `timeout` does not share.
    assert_eq!(
        output.status.code(),
        Some(4),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    eprintln!(
        "closing after {elapsed:?} and {} stopping answers",
        answers.load(Ordering::Relaxed)
    );
}

#[test]
fn a_full_hub_is_retried_and_then_reported() {
    let scratch = Scratch::new("mh4-capacity-", None);
    let first = scratch.print();
    let hub = Hub::observe(first.pid);
    let endpoint = scratch.endpoint();
    let mut held = Vec::new();
    while let Some(client) = Client::admitted(&endpoint) {
        held.push(client);
        assert!(held.len() < 4_096, "the hub never answered capacity");
    }
    // With every slot held past the retry, the printer gives up with the status of `unavailable`,
    // which `timeout` does not share.
    let started = Instant::now();
    let refused = scratch.printer().output().unwrap();
    let gave_up = started.elapsed();
    assert_eq!(
        refused.status.code(),
        Some(4),
        "{}",
        String::from_utf8_lossy(&refused.stderr)
    );
    // A slot freed while the next printer retries lets it attach to the same hub.
    let printer = scratch
        .printer()
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let retrying = Instant::now();
    thread::sleep(Duration::from_millis(300));
    drop(held.pop());
    let state = State::parse(&printer.wait_with_output().unwrap());
    let attached = retrying.elapsed();
    assert_eq!(state.pid, first.pid);
    eprintln!("gave up after {gave_up:?}; attached {attached:?} into the retry");
    drop(held);
    scratch.exit_hub();
    hub.wait_exit(WAIT);
}

#[test]
fn the_catalog_of_two_machines_serves_events_requests_and_a_reload() {
    let scratch = Scratch::new("mh4-catalog-", None);
    scratch.write_catalog(1, true);
    let state = scratch.print();
    assert_eq!(state.revision, 1);
    assert_eq!(
        state.machines,
        [
            (FIRST.to_owned(), "offline".to_owned()),
            (SECOND.to_owned(), "disabled".to_owned()),
        ]
    );
    let hub = Hub::observe(state.pid);
    let mut client = Client::connect(&scratch.endpoint());
    client.attach(false, true);
    let catalog = client.until(WAIT, "catalog", |client| {
        client.take(|delivery| match delivery {
            Delivery::Message(Message::Event {
                event: Event::Catalog(catalog),
                ..
            }) => Some(catalog.clone()),
            _ => None,
        })
    });
    assert_eq!(catalog.revision().get(), 1);
    assert_eq!(catalog.machines(), [machine(FIRST), machine(SECOND)]);
    assert_eq!(
        client.machine_state(WAIT, FIRST).name(),
        MachineStateName::Offline
    );
    assert_eq!(client.machine_state(WAIT, SECOND), MachineState::Disabled);

    let connect = |id: &str, draft: bool| {
        Request::Connect(Connect {
            machine: machine(id),
            origin: Origin::Command { answers: false },
            draft: draft.then(|| Draft {
                target: Target::new("user@host.invalid".to_owned()).unwrap(),
                session: SessionName::new("default".to_owned()).unwrap(),
            }),
        })
    };
    let not_online = |failure, state| {
        Answer::Reply(Reply::Connect(hub::ConnectResult {
            outcome: ConnectOutcome::NotOnline(failure),
            state,
        }))
    };
    for (request, expected) in [
        (
            connect(ABSENT, false),
            not_online(ConnectFailure::NotFound, None),
        ),
        (
            connect(SECOND, false),
            not_online(
                ConnectFailure::DisabledMachine,
                Some(MachineStateName::Disabled),
            ),
        ),
        (connect(ABSENT, true), Answer::Unsupported),
        (
            Request::Disconnect {
                machine: machine(ABSENT),
            },
            Answer::Reply(Reply::Disconnect(hub::DisconnectOutcome::NotFound)),
        ),
        (
            Request::StopSession {
                machine: machine(FIRST),
            },
            Answer::Reply(Reply::StopSession(StopSessionOutcome::NotOnline)),
        ),
        (
            Request::StopSession {
                machine: machine(ABSENT),
            },
            Answer::Reply(Reply::StopSession(StopSessionOutcome::NotFound)),
        ),
        (
            Request::Claim { login: Id::MIN },
            Answer::Reply(Reply::Claim(ClaimOutcome::Ended)),
        ),
    ] {
        let id = client.send(request);
        assert_eq!(client.answer(WAIT, id), expected);
    }
    for (open, reason) in [
        (
            Channel::Session {
                machine: machine(FIRST),
            },
            ResetReason::NotOnline,
        ),
        (
            Channel::Route {
                machine: machine(ABSENT),
            },
            ResetReason::NotFound,
        ),
    ] {
        let channel = client.open(open);
        let reset = client.until(WAIT, "reset", |client| {
            client.take(|delivery| match delivery {
                Delivery::Reset {
                    channel: reset,
                    reason,
                    ..
                } if *reset == channel => Some(*reason),
                _ => None,
            })
        });
        assert_eq!(reset, reason);
    }

    // Revision 2 disables the first machine and keeps the second as it was.
    scratch.write_catalog(2, false);
    let reload = client.send(Request::Reload);
    assert_eq!(
        client.answer(WAIT, reload),
        Answer::Reply(Reply::Reload(ReloadOutcome::Reloaded {
            revision: orchbee_machine_protocol::Count::new(2).unwrap(),
        }))
    );
    let catalog = client.until(WAIT, "the new catalog", |client| {
        client.take(|delivery| match delivery {
            Delivery::Message(Message::Event {
                event: Event::Catalog(catalog),
                ..
            }) => Some(catalog.clone()),
            _ => None,
        })
    });
    assert_eq!(catalog.revision().get(), 2);
    assert_eq!(client.machine_state(WAIT, FIRST), MachineState::Disabled);
    thread::sleep(Duration::from_millis(300));
    client.pump_until(Instant::now() + Duration::from_millis(200));
    assert!(
        !client.deliveries.iter().any(|delivery| matches!(
            delivery,
            Delivery::Message(Message::Event { event: Event::Machine(event), .. })
                if event.machine.as_str() == SECOND
        )),
        "an unchanged machine sends no event"
    );
    assert_eq!(scratch.print().revision, 2);
    drop(client);
    scratch.exit_hub();
    hub.wait_exit(WAIT);
}

#[test]
fn a_live_session_holds_the_hub_through_its_stop_and_a_new_start() {
    let fixture = session::Fixture::new("mh4-session-", &[]);
    let printer = || {
        let output = fixture
            .command(&["--machine-hub-state-v1"])
            .env("ORCHBEE_MACHINE_HUB_EXPIRY_MS", "120000")
            .output()
            .unwrap();
        State::parse(&output)
    };
    let root = DataRoot::resolve(fixture.state_environment()).unwrap();
    let state = printer();
    assert_eq!(state.hub, "held", "two live Sessions hold the new hub");
    let endpoint = machine_hub_endpoint(&root).unwrap();
    let guard = HubGuard {
        endpoint: endpoint.clone(),
        hub: Hub::observe(state.pid),
    };
    let mut watcher = Client::connect(&endpoint);
    assert_eq!(watcher.attach(false, true), HubState::Held);
    let mut holder = Client::connect(&endpoint);
    assert_eq!(holder.attach(true, false), HubState::Held);
    drop(holder);
    thread::sleep(Duration::from_secs(2));
    assert_eq!(
        watcher.attach(false, true),
        HubState::Held,
        "the Sessions still hold it"
    );

    let stopping = Instant::now();
    for name in ["host", "default"] {
        let stopped = fixture.run(&["server", "stop", "--confirm", "--session", name]);
        assert_eq!(stopped.code, Some(0), "{stopped}");
    }
    let stopped = stopping.elapsed();
    watcher.until(
        Duration::from_secs(40),
        "expiring after the stops",
        |client| client.hub_event(|state| matches!(state, HubState::Expiring { .. })),
    );
    let expiring = stopping.elapsed();
    let starting = Instant::now();
    let started = fixture.run(&["server", "start", "--session", "default"]);
    assert_eq!(started.code, Some(0), "{started}");
    watcher.until(Duration::from_secs(40), "held after the start", |client| {
        client.hub_event(|state| *state == HubState::Held)
    });
    let held = starting.elapsed();
    eprintln!(
        "stops took {stopped:?}; expiring {expiring:?} after the first stop; held {held:?} after \
         the start began"
    );
    // The fixture stops both Sessions when it drops.
    let started = fixture.run(&["server", "start", "--session", "host"]);
    assert_eq!(started.code, Some(0), "{started}");
    drop(watcher);
    drop(guard);
}

#[test]
fn uninstall_ends_the_hub_a_session_holds() {
    let fixture = session::Fixture::new("mh4-uninstall-", &["tmp"]);
    let output = fixture
        .command(&["--machine-hub-state-v1"])
        .env("ORCHBEE_MACHINE_HUB_EXPIRY_MS", "120000")
        .output()
        .unwrap();
    let state = State::parse(&output);
    assert_eq!(state.hub, "held", "two live Sessions hold the new hub");
    let hub = Hub::observe(state.pid);
    let mut nonce = [0; 8];
    getrandom::fill(&mut nonce).unwrap();
    let key = format!(
        r"Software\ob-test-scratch\mh4-uninstall-{:016x}",
        u64::from_le_bytes(nonce)
    );
    // Only the fixture's own variables and the system's: no Agent program on `PATH` and no
    // Agent's configuration folder outside the fixture.
    let mut command = fixture.command(&["uninstall", "--confirm"]);
    let own: Vec<(OsString, OsString)> = command
        .get_envs()
        .filter_map(|(name, value)| Some((name.to_owned(), value?.to_owned())))
        .collect();
    command.env_clear().envs(own);
    for name in ["SystemRoot", "windir", "SystemDrive", "ComSpec", "PATHEXT"] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    let system = PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32");
    command
        .env("PATH", &system)
        .env("TEMP", fixture.path("tmp"))
        .env("TMP", fixture.path("tmp"))
        .env("ORCHBEE_INSTALL_DIR", fixture.path("store"))
        .env("ORCHBEE_TEST_REGISTRY_KEY", &key);
    let started = Instant::now();
    let output = command.output().unwrap();
    let elapsed = started.elapsed();
    let _ = Command::new(system.join("reg.exe"))
        .args(["delete", &format!(r"HKCU\{key}"), "/f"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    // The background step is the subject: whatever the later steps do (the fixture's cohort
    // declares no real Agent adapters), the hub has ended by the time the run returns.
    assert_eq!(
        hub.exited(Duration::ZERO),
        Some(0),
        "the hub ended: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    eprintln!(
        "uninstall stopped two Sessions and ended hub {} in {elapsed:?}",
        state.pid
    );
    // The fixture stops both Sessions when it drops.
    for name in ["default", "host"] {
        let started = fixture.run(&["server", "start", "--session", name]);
        assert_eq!(started.code, Some(0), "{started}");
    }
}

#[test]
fn connect_carries_one_child_that_reconnects_and_ends_with_disconnect_and_the_hub() {
    let carried = Carried::new("mh4-carry-", false);
    let connected = carried.run(&["machine", "connect", FIRST]);
    assert_eq!(connected.code, Some(0), "{connected}");
    assert_eq!(
        connected.document["result"]["remote"], "ready",
        "{connected}"
    );
    assert!(
        connected.document["result"]["remoteVersion"].is_string(),
        "{connected}"
    );
    let endpoint = carried.endpoint();
    let hub = Hub::observe(query_pid(&endpoint));
    assert_eq!(
        carried.state().machines,
        [(FIRST.to_owned(), "online".to_owned())]
    );
    let first = one_child(&hub, None);
    assert!(
        first.image().ends_with(&CohortArtifact::Client.file_name()),
        "{}",
        first.image()
    );

    // A carrier that dies loses the link; the Machine reconnects by itself with one new child.
    let mut watcher = Client::connect(&endpoint);
    watcher.attach(false, true);
    assert_eq!(watcher.machine_state(WAIT, FIRST), MachineState::Online);
    first.terminate_checked();
    first.wait_exit(WAIT);
    let lost = watcher.machine_state(WAIT, FIRST);
    assert_eq!(lost.name(), MachineStateName::Reconnecting, "{lost:?}");
    let deadline = Instant::now() + WAIT;
    loop {
        let state =
            watcher.machine_state(deadline.saturating_duration_since(Instant::now()), FIRST);
        if state == MachineState::Online {
            break;
        }
        assert_eq!(state.name(), MachineStateName::Reconnecting, "{state:?}");
    }
    let second = one_child(&hub, Some(first.pid));
    assert_eq!(
        carried.state().machines,
        [(FIRST.to_owned(), "online".to_owned())]
    );
    drop(watcher);

    // Disconnect ends the carrier and leaves the Machine offline, retrying nothing.
    let disconnected = carried.run(&["machine", "disconnect", FIRST]);
    assert_eq!(disconnected.code, Some(0), "{disconnected}");
    second.wait_exit(WAIT);
    assert_eq!(
        carried.state().machines,
        [(FIRST.to_owned(), "offline".to_owned())]
    );

    // The hub's exit ends the carrier it holds.
    let connected = carried.run(&["machine", "connect", FIRST]);
    assert_eq!(connected.code, Some(0), "{connected}");
    let third = one_child(&hub, None);
    exit_hub(&endpoint);
    hub.wait_exit(WAIT);
    assert!(third.exited(WAIT).is_some(), "the carrier outlived the hub");

    // With no hub there is no connection to end, and disconnect starts none.
    let disconnected = carried.run(&["machine", "disconnect", FIRST]);
    assert_eq!(disconnected.code, Some(0), "{disconnected}");
    assert_eq!(disconnected.document["result"]["remote"], "not-contacted");
    assert!(!orchbee_local_ipc::endpoint_exists(&endpoint).unwrap());
}

#[test]
fn a_carrier_child_receives_no_orchbee_or_terminal_variable() {
    let carried = Carried::new("mh4-environment-", true);
    // The client that starts the hub carries every excluded kind: the runtime base and the two hub
    // variables the hub's view keeps, an `ORCHBEE_*` variable it drops, and the terminal's own.
    let mut command = carried.command(&["machine", "connect", FIRST, "--format=json"]);
    for name in TERMINAL {
        command.env(name, "mh4");
    }
    command
        .env("ORCHBEE_TEST_CARRIER_MARK", "mh4")
        .env("MH4_CARRIER_MARK", "mh4");
    let output = command.output().unwrap();
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let recorded = fs::read_to_string(carried.fixture.path("standin/environment.txt")).unwrap();
    let names: Vec<&str> = recorded
        .lines()
        .filter_map(|line| line.split_once('=').map(|(name, _)| name))
        .collect();
    assert!(names.iter().any(|name| name.eq_ignore_ascii_case("PATH")));
    assert!(names.contains(&"MH4_CARRIER_MARK"), "other variables pass");
    let excluded: Vec<&&str> = names
        .iter()
        .filter(|name| {
            let name = name.to_ascii_uppercase();
            name.starts_with("ORCHBEE_") || TERMINAL.contains(&name.as_str())
        })
        .collect();
    assert!(excluded.is_empty(), "the child received {excluded:?}");
}

#[test]
fn a_far_end_that_closes_at_once_is_a_machine_state_not_a_hub_failure() {
    // The scratch stand-in exits at once with an error, as a far end that closes would.
    let scratch = Scratch::new("mh4-closes-", None);
    scratch.write_catalog(1, true);
    let output = scratch
        .client("machine")
        .args(["connect", FIRST, "--format=json"])
        .output()
        .unwrap();
    let document: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_ne!(output.status.code(), Some(0));
    assert_eq!(document["result"]["error"], "remote-protocol", "{document}");
    let state = scratch.print();
    let hub = Hub::observe(state.pid);
    let mut watcher = Client::connect(&scratch.endpoint());
    watcher.attach(false, true);
    let reported = watcher.machine_state(WAIT, FIRST);
    assert!(
        matches!(reported, MachineState::Offline { cause: Some(_), .. }),
        "{reported:?}"
    );
    drop(watcher);
    scratch.exit_hub();
    hub.wait_exit(WAIT);
}

/// A real local runtime whose saved machine `FIRST`, named `user@host.invalid` with the remote
/// Session `far`, the hub carries through the stand-in: the Client under test as the far end, so no
/// `ssh` starts and nothing is contacted. The stand-in receives no `ORCHBEE_*` variable, so the far
/// Session runs in the data root's default runtime base, apart from the fixture's own Sessions.
struct Carried {
    fixture: session::Fixture,
    root: DataRoot,
    stand_in: PathBuf,
}

impl Carried {
    /// With `record`, the stand-in is a script that records its environment and then runs the
    /// Client.
    fn new(prefix: &'static str, record: bool) -> Self {
        assert!(
            !orchbee_local_runtime::is_release(),
            "a release build reads no stand-in and would run ssh"
        );
        let fixture = session::Fixture::new(prefix, &["standin"]);
        let root = DataRoot::resolve(fixture.state_environment()).unwrap();
        let client = Path::new(&fixture.path("cohort")).join(CohortArtifact::Client.file_name());
        let stand_in = if record {
            let script = PathBuf::from(fixture.path("standin/record.cmd"));
            fs::write(
                &script,
                format!(
                    "@echo off\r\nset > \"{}\"\r\n\"{}\" %*\r\n",
                    fixture.path("standin/environment.txt"),
                    client.display()
                ),
            )
            .unwrap();
            script
        } else {
            client
        };
        let document = json!({"version": 1, "revision": 1, "profiles": [{
            "id": FIRST, "label": "First",
            "connection": {"kind": "ssh", "target": "user@host.invalid"},
            "session": "far", "enabled": true, "remoteInstall": "manual"}]});
        PrivateCache::acquire(&root.state(), "machines")
            .unwrap()
            .replace("machines.json", &serde_json::to_vec(&document).unwrap())
            .unwrap();
        Self {
            fixture,
            root,
            stand_in,
        }
    }

    /// The CLI under test with the stand-in set, so a hub it starts runs no `ssh`.
    fn command(&self, words: &[&str]) -> Command {
        let mut command = self.fixture.command(words);
        command
            .env(STAND_IN, &self.stand_in)
            .env("ORCHBEE_MACHINE_HUB_EXPIRY_MS", "120000");
        command
    }

    fn run(&self, words: &[&str]) -> session::Run {
        let started = Instant::now();
        let output = self.command(words).arg("--format=json").output().unwrap();
        session::Run {
            words: words.join(" "),
            code: output.status.code(),
            elapsed: started.elapsed(),
            document: serde_json::from_slice(&output.stdout).unwrap_or_default(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }

    fn state(&self) -> State {
        State::parse(&self.command(&["--machine-hub-state-v1"]).output().unwrap())
    }

    fn endpoint(&self) -> PathBuf {
        machine_hub_endpoint(&self.root).unwrap()
    }
}

impl Drop for Carried {
    fn drop(&mut self) {
        // The hub runs from the fixture's cohort and the far Session lives in its root, so both end
        // before the fixture stops its own Sessions and removes the root.
        let endpoint = self.endpoint();
        if orchbee_local_ipc::endpoint_exists(&endpoint).unwrap_or(false)
            && let Ok(AcceptorMessage::QueryResult(result)) = request(&endpoint, RequestKind::Query)
            && let Some(process) = result.processes().first()
            && let Ok(hub) =
                HPROCESS::OpenProcess(co::PROCESS::SYNCHRONIZE, false, process.pid.get())
        {
            let _ = request(&endpoint, RequestKind::Exit);
            let _ = hub.WaitForSingleObject(Some(u32::try_from(WAIT.as_millis()).unwrap()));
        }
        let _ = self
            .fixture
            .command(&["server", "stop", "--confirm", "--session", "far"])
            .env_remove("ORCHBEE_RUNTIME_BASE")
            .output();
    }
}

/// The live processes `hub` started: those whose parent id is the hub's and which started after
/// it, so a process whose earlier parent had the same id never counts.
fn children(hub: &Hub) -> Vec<Hub> {
    let mut list = HPROCESSLIST::CreateToolhelp32Snapshot(co::TH32CS::SNAPPROCESS, None).unwrap();
    let ids: Vec<u32> = list
        .iter_processes()
        .filter_map(Result::ok)
        .filter(|entry| entry.th32ParentProcessID == hub.pid)
        .map(|entry| entry.th32ProcessID)
        .collect();
    ids.into_iter()
        .filter_map(|pid| {
            let process = HPROCESS::OpenProcess(
                co::PROCESS::QUERY_LIMITED_INFORMATION
                    | co::PROCESS::SYNCHRONIZE
                    | co::PROCESS::TERMINATE,
                false,
                pid,
            )
            .ok()?;
            let created = creation(&process);
            let child = Hub {
                pid,
                process,
                created,
            };
            (created >= hub.created && child.exited(Duration::ZERO).is_none()).then_some(child)
        })
        .collect()
}

/// The one live child of `hub`, other than `old`, once it has exactly one.
fn one_child(hub: &Hub, old: Option<u32>) -> Hub {
    let deadline = Instant::now() + WAIT;
    loop {
        let mut live = children(hub);
        if live.len() == 1 && Some(live[0].pid) != old {
            return live.remove(0);
        }
        assert!(
            Instant::now() < deadline,
            "the hub has {} live children",
            live.len()
        );
        thread::sleep(Duration::from_millis(100));
    }
}

/// A scratch data root with its own staged client, whose hub the test ends before the root goes.
struct Scratch {
    root: PathBuf,
    expiry: Option<&'static str>,
}

impl Scratch {
    fn new(prefix: &str, expiry: Option<&'static str>) -> Self {
        let mut nonce = [0; 8];
        getrandom::fill(&mut nonce).unwrap();
        let root = std::env::temp_dir().join(format!("{prefix}{:016x}", u64::from_le_bytes(nonce)));
        for folder in ["cohort", "home", "profile", "local", "roaming"] {
            fs::create_dir_all(root.join(folder)).unwrap();
        }
        fs::copy(
            env!("CARGO_BIN_EXE_orchbee"),
            root.join("cohort").join(CohortArtifact::Client.file_name()),
        )
        .unwrap();
        Self { root, expiry }
    }

    fn home(&self) -> PathBuf {
        self.root.join("home")
    }

    fn data_root(&self) -> DataRoot {
        self.resolved().unwrap()
    }

    /// The data root without panicking, for the drop.
    fn resolved(&self) -> Option<DataRoot> {
        let home = self.home().into_os_string();
        DataRoot::resolve(|name| (name == "ORCHBEE_HOME").then(|| home.clone())).ok()
    }

    fn endpoint(&self) -> PathBuf {
        machine_hub_endpoint(&self.data_root()).unwrap()
    }

    fn printer(&self) -> Command {
        self.client("--machine-hub-state-v1")
    }

    /// The staged client's `entry` in this root only.
    fn client(&self, entry: &str) -> Command {
        let mut command = Command::new(
            self.root
                .join("cohort")
                .join(CohortArtifact::Client.file_name()),
        );
        // No `ORCHBEE_*` variable of the shell that runs Cargo reaches the hub.
        for (name, _) in std::env::vars_os() {
            if name
                .to_str()
                .and_then(|name| name.get(.."ORCHBEE_".len()))
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case("ORCHBEE_"))
            {
                command.env_remove(name);
            }
        }
        command
            .arg(entry)
            // Every hub of these roots runs a stand-in that exits at once, so no test can run `ssh`.
            .env(STAND_IN, session::system_program("where.exe"))
            .env("ORCHBEE_HOME", self.home())
            .env("USERPROFILE", self.root.join("profile"))
            .env("HOME", self.root.join("profile"))
            .env("APPDATA", self.root.join("roaming"))
            .env("LOCALAPPDATA", self.root.join("local"))
            .env("LC_ALL", "en-US")
            .current_dir(&self.root)
            .stdin(Stdio::null());
        if let Some(expiry) = self.expiry {
            command.env("ORCHBEE_MACHINE_HUB_EXPIRY_MS", expiry);
        }
        command
    }

    fn print(&self) -> State {
        State::parse(&self.printer().output().unwrap())
    }

    fn query_pid(&self) -> u32 {
        query_pid(&self.endpoint())
    }

    fn exit_hub(&self) {
        exit_hub(&self.endpoint());
    }

    /// Writes `machines.json` at `revision` with two `user@host.invalid` machines, the first
    /// enabled as `first_enabled` says and the second disabled.
    fn write_catalog(&self, revision: u64, first_enabled: bool) {
        let profile = |id: &str, label: &str, enabled: bool| {
            json!({"id": id, "label": label,
                "connection": {"kind": "ssh", "target": "user@host.invalid"},
                "session": "default", "enabled": enabled, "remoteInstall": "manual"})
        };
        let document = json!({"version": 1, "revision": revision, "profiles": [
            profile(FIRST, "First", first_enabled),
            profile(SECOND, "Second", false),
        ]});
        let mut cache = PrivateCache::acquire(&self.data_root().state(), "machines").unwrap();
        cache
            .replace("machines.json", &serde_json::to_vec(&document).unwrap())
            .unwrap();
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let endpoint = self
            .resolved()
            .and_then(|root| machine_hub_endpoint(&root).ok());
        let deadline = Instant::now() + WAIT;
        loop {
            // A hub of this root runs from the staged client, so the root cannot go while one runs,
            // and one a failed client started may bind after the first look.
            if let Some(endpoint) = &endpoint
                && orchbee_local_ipc::endpoint_exists(endpoint).unwrap_or(false)
            {
                let _ = request(endpoint, RequestKind::Exit);
            }
            let Err(error) = fs::remove_dir_all(&self.root) else {
                return;
            };
            if Instant::now() >= deadline {
                if !thread::panicking() {
                    panic!("remove {}: {error}", self.root.display());
                }
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }
    }
}

/// Ends the fixture's hub before the fixture stops its Sessions and removes its root.
struct HubGuard {
    endpoint: PathBuf,
    hub: Hub,
}

impl Drop for HubGuard {
    fn drop(&mut self) {
        let _ = request(&self.endpoint, RequestKind::Exit);
        if self.hub.exited(WAIT).is_none() && !thread::panicking() {
            panic!("hub {} did not exit", self.hub.pid);
        }
    }
}

/// One `attached` or `reattached` line of the hold entry.
#[derive(Debug)]
struct Attachment {
    pid: u32,
    state: String,
}

/// The lines `output` prints, as they arrive.
fn lines(output: ChildStdout) -> mpsc::Receiver<String> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(output).lines() {
            let Ok(line) = line else { return };
            if sender.send(line).is_err() {
                return;
            }
        }
    });
    receiver
}

/// The next line that begins with `word`, skipping events.
fn attachment(lines: &mpsc::Receiver<String>, word: &str) -> Attachment {
    let deadline = Instant::now() + WAIT;
    loop {
        let line = lines
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap_or_else(|_| panic!("no {word} line within {WAIT:?}"));
        let fields: Vec<&str> = line.split(' ').collect();
        if let [first, pid, _starts, state] = fields[..]
            && first == word
        {
            return Attachment {
                pid: pid.parse().unwrap(),
                state: state.to_owned(),
            };
        }
    }
}

/// What the state printer printed.
#[derive(Debug)]
struct State {
    hub: String,
    pid: u32,
    revision: u64,
    machines: Vec<(String, String)>,
}

impl State {
    fn parse(output: &Output) -> Self {
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success(),
            "the printer failed: {:?} {stdout} {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );
        let field = |name: &str| {
            stdout
                .lines()
                .find_map(|line| line.strip_prefix(name))
                .unwrap_or_else(|| panic!("no {name} in {stdout}"))
                .to_owned()
        };
        Self {
            hub: field("Machine hub: "),
            pid: field("Process ID: ").parse().unwrap(),
            revision: field("Catalog revision: ").parse().unwrap(),
            machines: stdout
                .lines()
                .filter_map(|line| line.split_once("  "))
                .map(|(id, state)| (id.to_owned(), state.to_owned()))
                .collect(),
        }
    }
}

/// A hub process this test observes through its own handle, so a later process with the same id
/// can never be mistaken for it.
struct Hub {
    pid: u32,
    process: CloseHandleGuard<HPROCESS>,
    created: u64,
}

impl Hub {
    fn observe(pid: u32) -> Self {
        let process = HPROCESS::OpenProcess(
            co::PROCESS::QUERY_LIMITED_INFORMATION
                | co::PROCESS::SYNCHRONIZE
                | co::PROCESS::TERMINATE,
            false,
            pid,
        )
        .unwrap();
        let created = creation(&process);
        Self {
            pid,
            process,
            created,
        }
    }

    fn image(&self) -> String {
        self.process
            .QueryFullProcessImageName(co::PROCESS_NAME::WIN32)
            .unwrap()
    }

    /// Terminates the hub after checking that the id still names the process observed, by its
    /// creation time through a fresh handle.
    fn terminate_checked(&self) {
        let fresh =
            HPROCESS::OpenProcess(co::PROCESS::QUERY_LIMITED_INFORMATION, false, self.pid).unwrap();
        assert_eq!(
            creation(&fresh),
            self.created,
            "process {} was replaced",
            self.pid
        );
        self.process.TerminateProcess(1).unwrap();
    }

    /// Waits for the hub to exit and returns its status.
    fn wait_exit(&self, bound: Duration) -> u32 {
        self.exited(bound)
            .unwrap_or_else(|| panic!("hub {} did not exit within {bound:?}", self.pid))
    }

    fn exited(&self, bound: Duration) -> Option<u32> {
        let waited = self
            .process
            .WaitForSingleObject(Some(u32::try_from(bound.as_millis()).unwrap_or(u32::MAX)))
            .ok()?;
        (waited == co::WAIT::OBJECT_0)
            .then(|| self.process.GetExitCodeProcess().ok())
            .flatten()
    }
}

fn creation(process: &HPROCESS) -> u64 {
    let (created, ..) = process.GetProcessTimes().unwrap();
    (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime)
}

fn wall_now() -> u64 {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis(),
    )
    .unwrap()
}

fn machine(id: &str) -> MachineId {
    MachineId::new(id.to_owned()).unwrap()
}

fn request(endpoint: &Path, kind: RequestKind) -> io::Result<AcceptorMessage> {
    let mut stream = LocalStream::connect_before(endpoint, Instant::now() + WAIT)?;
    stream.set_read_timeout(Some(WAIT))?;
    let message = InitiatorMessage::Request(ControlRequest {
        kind,
        environment: orchbee_local_runtime::environment_name(),
        extensions: Extensions::new(),
    });
    stream.write_all(&encode_initiator_frame(&message).unwrap())?;
    read_acceptor_message(&mut stream).map_err(io::Error::other)
}

fn query_pid(endpoint: &Path) -> u32 {
    match request(endpoint, RequestKind::Query).unwrap() {
        AcceptorMessage::QueryResult(result) => result.processes()[0].pid.get(),
        other => panic!("query answered {other:?}"),
    }
}

fn exit_hub(endpoint: &Path) {
    assert!(matches!(
        request(endpoint, RequestKind::Exit).unwrap(),
        AcceptorMessage::ExitResult(_)
    ));
}

/// What a request was answered with.
#[derive(Debug, PartialEq)]
enum Answer {
    Reply(Reply),
    Unsupported,
}

/// A `machine-hub` client speaking the codec on the hub's endpoint.
struct Client {
    stream: LocalStream,
    connection: Connection<hub::Hub>,
    decoder: FrameDecoder,
    deliveries: Vec<Delivery<hub::Hub>>,
    ended: bool,
}

impl Client {
    fn connect(endpoint: &Path) -> Self {
        Self::admitted(endpoint).expect("the hub answered capacity")
    }

    /// Connects and offers `machine-hub`; `None` when the hub answers `capacity`.
    fn admitted(endpoint: &Path) -> Option<Self> {
        let mut stream = LocalStream::connect_before(endpoint, Instant::now() + WAIT).unwrap();
        stream.set_read_timeout(Some(WAIT)).unwrap();
        let offer = Offer::local(
            ConnectionKind::MachineHub,
            ProductVersion::new(env!("CARGO_PKG_VERSION")).unwrap(),
            Build::default(),
            orchbee_local_runtime::environment_name(),
        );
        stream
            .write_all(&encode_initiator_frame(&InitiatorMessage::Offer(offer)).unwrap())
            .unwrap();
        match read_acceptor_message(&mut stream).unwrap() {
            AcceptorMessage::Answer(_) => {}
            AcceptorMessage::Rejection(rejection)
                if rejection.reason == RejectionReason::Capacity =>
            {
                return None;
            }
            other => panic!("the offer was answered {other:?}"),
        }
        stream
            .set_read_timeout(Some(Duration::from_millis(50)))
            .unwrap();
        Some(Self {
            stream,
            connection: Connection::<hub::Hub>::initiator(),
            decoder: FrameDecoder::new(),
            deliveries: Vec::new(),
            ended: false,
        })
    }

    fn flush(&mut self) {
        while let Some(frame) = self.connection.poll_transmit() {
            (&self.stream).write_all(&frame.encode()).unwrap();
        }
    }

    fn send(&mut self, request: Request) -> Id {
        let id = self.connection.request(request).unwrap();
        self.flush();
        id
    }

    fn open(&mut self, channel: Channel) -> u32 {
        let channel = self.connection.open(channel).unwrap();
        self.flush();
        channel
    }

    /// Sends `attach` and returns the state its reply carries.
    fn attach(&mut self, presence: bool, watch: bool) -> HubState {
        let id = self.send(Request::Attach(Attach {
            presence,
            asks: false,
            watch,
        }));
        match self.answer(WAIT, id) {
            Answer::Reply(Reply::Attach(AttachOutcome::Attached { hub })) => hub,
            other => panic!("attach answered {other:?}"),
        }
    }

    /// Reads until `deadline` or the end, keeping every delivery.
    fn pump_until(&mut self, deadline: Instant) {
        let mut buffer = [0; 16 * 1024];
        while !self.ended && Instant::now() < deadline {
            let read = match (&self.stream).read(&mut buffer) {
                Ok(read) => read,
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                    ) =>
                {
                    return;
                }
                Err(_) => 0,
            };
            if read == 0 {
                self.ended = true;
                return;
            }
            let mut offset = 0;
            while offset < read {
                let (consumed, frame) = self.decoder.decode(&buffer[offset..read]).unwrap();
                offset += consumed;
                if let Some(frame) = frame {
                    let receipt = self.connection.receive(&frame).unwrap();
                    self.deliveries.extend(receipt.delivery);
                }
                if consumed == 0 {
                    break;
                }
            }
            self.flush();
        }
    }

    fn until<T>(
        &mut self,
        bound: Duration,
        what: &str,
        mut found: impl FnMut(&mut Self) -> Option<T>,
    ) -> T {
        let deadline = Instant::now() + bound;
        loop {
            if let Some(value) = found(self) {
                return value;
            }
            assert!(Instant::now() < deadline, "no {what} within {bound:?}");
            self.pump_until(deadline);
        }
    }

    /// Removes and returns the first delivery `pick` accepts.
    fn take<T>(&mut self, mut pick: impl FnMut(&Delivery<hub::Hub>) -> Option<T>) -> Option<T> {
        let (index, value) = self
            .deliveries
            .iter()
            .enumerate()
            .find_map(|(index, delivery)| pick(delivery).map(|value| (index, value)))?;
        self.deliveries.remove(index);
        Some(value)
    }

    fn answer(&mut self, bound: Duration, id: Id) -> Answer {
        self.until(bound, "answer", |client| {
            client.take(|delivery| match delivery {
                Delivery::Message(Message::Reply {
                    id: answered,
                    reply,
                    ..
                }) if *answered == id => Some(Answer::Reply(reply.clone())),
                Delivery::Message(Message::Unsupported { id: answered, .. }) if *answered == id => {
                    Some(Answer::Unsupported)
                }
                _ => None,
            })
        })
    }

    fn hub_event(&mut self, accept: impl Fn(&HubState) -> bool) -> Option<HubState> {
        self.take(|delivery| match delivery {
            Delivery::Message(Message::Event {
                event: Event::Hub(state),
                ..
            }) if accept(state) => Some(*state),
            _ => None,
        })
    }

    fn machine_state(&mut self, bound: Duration, id: &str) -> MachineState {
        self.until(bound, "machine", |client| {
            client.take(|delivery| match delivery {
                Delivery::Message(Message::Event {
                    event: Event::Machine(event),
                    ..
                }) if event.machine.as_str() == id => Some(event.state.clone()),
                _ => None,
            })
        })
    }

    fn bye(&mut self) -> Option<ByeReason> {
        self.take(|delivery| match delivery {
            Delivery::Message(Message::Bye { reason, .. }) => Some(*reason),
            _ => None,
        })
    }
}
