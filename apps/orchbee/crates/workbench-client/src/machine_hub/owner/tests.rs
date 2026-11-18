use super::super::{
    AttachFailure, Attachment, ChannelDelivery, HubClient, Notice, OpenCancel, Opening,
    RequestFailure,
};
use crate::machine_endpoint::{bind_locked, hold_folder};
use orchbee_connection_handshake::{
    AcceptorLoad, AcceptorMessage, AcceptorProfile, Build, ConnectionKind, Decision, ProcessState,
    ProductVersion, encode_acceptor_frame, read_initiator_message,
};
use orchbee_local_runtime::{DataRoot, create_private_directory, machine_hub_endpoint};
use orchbee_machine_protocol::{
    ByeReason, Connection, Count, Delivery, Frame, FrameDecoder, FrameKind, MachineId, Message,
    hub::{self, AttachOutcome, Channel, HubState, ReloadOutcome, Reply, Request},
};
use std::{
    fs,
    io::{ErrorKind, Read, Write},
    net::Shutdown,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

struct TempRoot(PathBuf);

impl TempRoot {
    fn new(name: &str) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        // Admission refuses any path through a link, and macOS's temp folder sits under `/var`.
        let temporary = std::env::temp_dir()
            .canonicalize()
            .expect("canonical temporary folder");
        let dir = temporary.join(format!(
            "orchbee-owner-{name}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        #[cfg(not(windows))]
        {
            use std::os::unix::fs::DirBuilderExt;
            fs::DirBuilder::new()
                .mode(0o700)
                .create(&dir)
                .expect("create private temp root");
        }
        #[cfg(windows)]
        fs::create_dir(&dir).expect("create temp root");
        Self(dir)
    }

    fn data_root(&self) -> DataRoot {
        DataRoot::at(self.0.clone())
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _removed = fs::remove_dir_all(&self.0);
    }
}

#[derive(Clone, Copy)]
enum ScriptedFault {
    CorruptFrame,
    ProtocolViolation,
    PeerByeViolation,
}

impl ScriptedFault {
    const fn stage_token(self) -> &'static str {
        match self {
            Self::CorruptFrame => "decode",
            Self::ProtocolViolation => "protocol",
            Self::PeerByeViolation => "peer-bye",
        }
    }

    const fn class_token(self) -> &'static str {
        match self {
            Self::CorruptFrame => "frame",
            Self::ProtocolViolation => "unmatched-pong",
            Self::PeerByeViolation => "peer-bye",
        }
    }
}

fn acceptor_profile() -> AcceptorProfile {
    AcceptorProfile::local(
        ConnectionKind::MachineHub,
        ProductVersion::new(env!("CARGO_PKG_VERSION"))
            .expect("package version is valid handshake ASCII"),
        Build::default(),
        orchbee_local_runtime::environment_name(),
    )
}

fn accept_hub_connection(
    stream: &mut crate::machine_endpoint::LocalStream,
) -> Connection<hub::Hub> {
    let profile = acceptor_profile();
    let mut reader = &*stream;
    let message = read_initiator_message(&mut reader).expect("read offer from client");
    let decision = profile.decide(
        &message,
        AcceptorLoad {
            session_slot_free: true,
            control_slot_free: true,
            state: ProcessState::Ready,
        },
    );
    let Decision::Accept(answer) = decision else {
        panic!("expected offer acceptance");
    };
    let encoded = encode_acceptor_frame(&AcceptorMessage::Answer(answer)).expect("encode answer");
    stream.write_all(&encoded).expect("write offer answer");

    let mut conn = Connection::<hub::Hub>::acceptor();
    let mut decoder = FrameDecoder::new();
    let mut buf = [0_u8; 4096];
    let attach_id = loop {
        let read = stream.read(&mut buf).expect("read attach frame");
        assert!(read > 0, "unexpected EOF waiting for attach");
        let mut offset = 0;
        let mut found = None;
        while offset < read {
            let (consumed, frame) = decoder.decode(&buf[offset..read]).expect("decode frame");
            offset += consumed;
            if let Some(frame) = frame {
                let receipt = conn.receive(&frame).expect("receive attach frame");
                if let Some(Delivery::Message(Message::Request {
                    id,
                    request: Request::Attach(_),
                    ..
                })) = receipt.delivery
                {
                    found = Some(id);
                }
            }
            if consumed == 0 {
                break;
            }
        }
        if let Some(id) = found {
            break id;
        }
    };
    conn.reply(
        attach_id,
        Reply::Attach(AttachOutcome::Attached {
            hub: HubState::Held,
        }),
    )
    .expect("queue attach reply");
    flush_frames(&mut conn, stream);
    conn
}

fn flush_frames(
    conn: &mut Connection<hub::Hub>,
    stream: &mut crate::machine_endpoint::LocalStream,
) {
    let mut bytes = Vec::new();
    while let Some(frame) = conn.poll_transmit() {
        bytes.extend_from_slice(&frame.encode());
    }
    if !bytes.is_empty() {
        stream.write_all(&bytes).expect("write hub frames");
    }
}

fn next_delivery(
    conn: &mut Connection<hub::Hub>,
    decoder: &mut FrameDecoder,
    stream: &mut crate::machine_endpoint::LocalStream,
) -> Option<Delivery<hub::Hub>> {
    let mut buf = [0_u8; 4096];
    loop {
        let read = match stream.read(&mut buf) {
            Ok(0) => return None,
            Ok(read) => read,
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(_) => return None,
        };
        let mut offset = 0;
        while offset < read {
            let (consumed, frame) = decoder.decode(&buf[offset..read]).expect("decode frame");
            offset += consumed;
            if let Some(frame) = frame
                && let Some(delivery) = conn.receive(&frame).expect("receive frame").delivery
            {
                return Some(delivery);
            }
            if consumed == 0 {
                break;
            }
        }
    }
}

fn bind_test_endpoint(
    root: &DataRoot,
) -> (
    crate::machine_endpoint::FolderHold,
    crate::machine_endpoint::LocalListener,
) {
    create_private_directory(&root.machines()).expect("create private machines directory");
    let endpoint = machine_hub_endpoint(root).expect("derive hub endpoint");
    let folder = hold_folder().expect("hold socket folder");
    let listener = bind_locked(&endpoint).expect("bind test hub endpoint");
    (folder, listener)
}

#[test]
fn a_violation_ends_only_its_connection_and_the_owner_reattaches() {
    for fault in [
        ScriptedFault::CorruptFrame,
        ScriptedFault::ProtocolViolation,
        ScriptedFault::PeerByeViolation,
    ] {
        let temp = TempRoot::new("isolate");
        let root = temp.data_root();
        let (_folder, listener) = bind_test_endpoint(&root);
        let machine = MachineId::new("0123456789abcdef0123456789abcdef").expect("valid machine id");

        let (conn1_ready_tx, conn1_ready_rx) = mpsc::channel();
        let (trigger_fault_tx, trigger_fault_rx) = mpsc::channel();

        let server = thread::spawn(move || {
            // Connection 1: attach, open channel + hold reload in flight, then inject fault.
            let (mut stream1, ()) = listener.accept().expect("accept connection 1");
            let mut conn1 = accept_hub_connection(&mut stream1);
            let mut dec1 = FrameDecoder::new();
            let mut saw_open = false;
            let mut saw_reload = false;
            while !(saw_open && saw_reload) {
                match next_delivery(&mut conn1, &mut dec1, &mut stream1)
                    .expect("expected open and reload on connection 1")
                {
                    Delivery::Open { channel, .. } => {
                        conn1.accept(channel).expect("accept channel");
                        flush_frames(&mut conn1, &mut stream1);
                        saw_open = true;
                    }
                    Delivery::Message(Message::Request {
                        request: Request::Reload,
                        ..
                    }) => {
                        saw_reload = true;
                    }
                    _ => {}
                }
            }
            conn1_ready_tx.send(()).expect("signal conn1 ready");
            trigger_fault_rx.recv().expect("wait to inject fault");

            match fault {
                ScriptedFault::CorruptFrame => {
                    stream1
                        .write_all(b"NOPE000000000000")
                        .expect("write corrupt frame");
                }
                ScriptedFault::ProtocolViolation => {
                    let pong = Frame::new(FrameKind::Pong, 0, vec![7; 8])
                        .expect("build unmatched pong")
                        .encode();
                    stream1.write_all(&pong).expect("write unmatched pong");
                }
                ScriptedFault::PeerByeViolation => {
                    conn1
                        .bye(ByeReason::Violation)
                        .expect("queue bye violation");
                    flush_frames(&mut conn1, &mut stream1);
                }
            }
            drop(stream1.shutdown(Shutdown::Both));

            // Connection 2: accept re-attachment and keep serving requests and channels.
            let (mut stream2, ()) = listener.accept().expect("accept connection 2");
            let mut conn2 = accept_hub_connection(&mut stream2);
            let mut dec2 = FrameDecoder::new();
            while let Some(delivery) = next_delivery(&mut conn2, &mut dec2, &mut stream2) {
                match delivery {
                    Delivery::Message(Message::Request {
                        id,
                        request: Request::Reload,
                        ..
                    }) => {
                        conn2
                            .reply(
                                id,
                                Reply::Reload(ReloadOutcome::Reloaded {
                                    revision: Count::new(42).expect("valid count"),
                                }),
                            )
                            .expect("reply reload on conn2");
                        flush_frames(&mut conn2, &mut stream2);
                    }
                    Delivery::Open { channel, .. } => {
                        conn2.accept(channel).expect("accept channel on conn2");
                        flush_frames(&mut conn2, &mut stream2);
                    }
                    Delivery::Message(Message::Bye { .. }) => break,
                    _ => {}
                }
            }
        });

        let deadline = Instant::now() + Duration::from_secs(10);
        let (client, _) = HubClient::open_with_in(
            root,
            Attachment {
                presence: true,
                asks: false,
                watch: true,
            },
            deadline,
            Opening::Join,
            &OpenCancel::default(),
        )
        .expect("attach connection 1");

        let channel1 = client
            .open_channel(
                &Channel::Session {
                    machine: machine.clone(),
                },
                deadline,
            )
            .expect("open channel on conn1");
        assert_eq!(
            client.receive(channel1, deadline),
            Some(ChannelDelivery::Accepted)
        );

        let inflight = thread::scope(|scope| {
            let req = scope.spawn(|| client.request(Request::Reload, deadline));
            conn1_ready_rx.recv().expect("wait for conn1 requests");
            trigger_fault_tx.send(()).expect("trigger fault");
            req.join().expect("join request thread")
        });
        assert_eq!(inflight, Err(RequestFailure::Lost));
        assert_eq!(
            client.receive(channel1, deadline),
            Some(ChannelDelivery::Lost),
            "channel opened on broken connection must end Lost"
        );

        let notice = client
            .next(Instant::now() + Duration::from_secs(10))
            .expect("expected reattachment notice instead of owner end");
        assert!(
            matches!(notice, Notice::Reattached(_)),
            "expected Notice::Reattached, got {notice:?}"
        );

        let reply2 = client
            .request(Request::Reload, Instant::now() + Duration::from_secs(5))
            .expect("connection 2 must keep serving requests");
        assert_eq!(
            reply2,
            Reply::Reload(ReloadOutcome::Reloaded {
                revision: Count::new(42).expect("valid count"),
            })
        );

        let channel2 = client
            .open_channel(
                &Channel::Session { machine },
                Instant::now() + Duration::from_secs(5),
            )
            .expect("connection 2 must open new channels");
        assert_eq!(
            client.receive(channel2, Instant::now() + Duration::from_secs(5)),
            Some(ChannelDelivery::Accepted)
        );

        let faults = client.fault_records();
        assert!(
            faults
                .iter()
                .any(|record| record.event == "machine-hub-loss"
                    && record.field == "stage"
                    && record.value == fault.stage_token()),
            "expected structured stage fault record for {}, got {faults:?}",
            fault.stage_token()
        );
        assert!(
            faults
                .iter()
                .any(|record| record.event == "machine-hub-loss"
                    && record.field == "class"
                    && record.value == fault.class_token()),
            "expected structured class fault record for {}, got {faults:?}",
            fault.class_token()
        );
        assert!(
            !faults
                .iter()
                .any(|record| record.event == "machine-hub-ended"),
            "single connection violation must not end owner: {faults:?}"
        );

        client.close();
        server.join().expect("server thread finished");
    }
}

#[test]
fn repeated_violations_reach_the_bound_and_preserve_protocol_failure() {
    let temp = TempRoot::new("bound");
    let root = temp.data_root();
    let (_folder, listener) = bind_test_endpoint(&root);
    let (ready_tx, ready_rx) = mpsc::channel();

    let server = thread::spawn(move || {
        for _attempt in 0..super::LOSSES {
            let (mut stream, ()) = listener.accept().expect("accept repeated connection");
            let mut conn = accept_hub_connection(&mut stream);
            let mut dec = FrameDecoder::new();
            ready_tx.send(()).expect("signal connection ready");
            loop {
                match next_delivery(&mut conn, &mut dec, &mut stream) {
                    Some(Delivery::Message(Message::Request {
                        request: Request::Reload,
                        ..
                    })) => break,
                    Some(_) => {}
                    None => break,
                }
            }
            let pong = Frame::new(FrameKind::Pong, 0, vec![9; 8])
                .expect("build unmatched pong")
                .encode();
            stream.write_all(&pong).expect("write repeated violation");
            drop(stream.shutdown(Shutdown::Both));
        }
    });

    let deadline = Instant::now() + Duration::from_secs(15);
    let (client, _) = HubClient::open_with_in(
        root,
        Attachment::default(),
        deadline,
        Opening::Join,
        &OpenCancel::default(),
    )
    .expect("initial attach");

    let mut ended = None;
    let until = Instant::now() + Duration::from_secs(15);
    let client_ref = &client;
    thread::scope(|scope| {
        let driver = scope.spawn(move || {
            while ready_rx.recv_timeout(Duration::from_secs(10)).is_ok() {
                drop(client_ref.request(Request::Reload, Instant::now() + Duration::from_secs(5)));
            }
        });
        while Instant::now() < until {
            match client_ref.next(until) {
                Some(Notice::Ended(failure)) => {
                    ended = Some(failure);
                    break;
                }
                Some(Notice::Reattached(_)) | Some(Notice::Event(_)) => {}
                None => break,
            }
        }
        drop(driver.join());
    });
    assert_eq!(ended, Some(AttachFailure::Lost));

    let faults = client.fault_records();
    let stage_count = faults
        .iter()
        .filter(|record| {
            record.event == "machine-hub-loss"
                && record.field == "stage"
                && record.value == "protocol"
        })
        .count();
    assert_eq!(stage_count, super::LOSSES, "{faults:?}");
    let class_count = faults
        .iter()
        .filter(|record| {
            record.event == "machine-hub-loss"
                && record.field == "class"
                && record.value == "unmatched-pong"
        })
        .count();
    assert_eq!(class_count, super::LOSSES, "{faults:?}");
    assert!(
        faults
            .iter()
            .any(|record| record.event == "machine-hub-ended" && record.value == "lost"),
        "expected preserved lost failure record: {faults:?}"
    );

    client.close();
    server.join().expect("server thread finished");
}
