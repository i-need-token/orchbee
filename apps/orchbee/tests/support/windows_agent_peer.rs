//! Shared Windows CLI/cohort and named-pipe peer fixtures for Agent process evidence.

use orchbee_agent_observation_protocol::{
    AgentRequest, AgentResult, read_agent_request, write_agent_result,
};
use orchbee_local_client_protocol::{LocalClientMessage, read_message, write_message};
use orchbee_local_ipc::{LocalListener, LocalStream};
use orchbee_local_site_runtime::{ModeCheckedLocalSiteRendezvous, SiteScope};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::{Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};

pub(super) struct Fixture(pub(super) PathBuf, Mutex<Option<mpsc::Sender<Instant>>>);
impl Fixture {
    pub(super) fn new() -> Self {
        let mut nonce = [0; 16];
        getrandom::fill(&mut nonce).unwrap();
        let root = std::env::temp_dir().join(format!(
            "orchbee-cli-names-{:032x}",
            u128::from_le_bytes(nonce)
        ));
        fs::create_dir(&root).unwrap();
        let mut artifacts = serde_json::Map::new();
        for name in [
            "orchbee",
            "orchbee-server",
            "orchbee-worker",
            "orchbee-terminal-host",
        ] {
            let file = root.join(format!("{name}.exe"));
            if name == "orchbee" {
                fs::copy(env!("CARGO_BIN_EXE_orchbee"), &file).unwrap();
            } else {
                fs::write(&file, b"unused cohort fixture").unwrap();
            }
            artifacts.insert(
                name.to_owned(),
                json!(format!("{:x}", Sha256::digest(fs::read(file).unwrap()))),
            );
        }
        artifacts.insert(
            "agent-observation-adapters".to_owned(),
            json!(format!("{:x}", Sha256::digest(b"unused adapters"))),
        );
        fs::write(
            root.join("cohort.json"),
            json!({"schemaVersion":2,"artifacts":artifacts}).to_string(),
        )
        .unwrap();
        Self(root, Mutex::new(None))
    }

    pub(super) fn rendezvous(&self) -> ModeCheckedLocalSiteRendezvous {
        let base = self.0.join("runtime");
        fs::create_dir_all(&base).unwrap();
        let sid = orchbee_local_ipc::current_user_sid().unwrap();
        let scope = format!("orchbee-native-{:x}", Sha256::digest(sid.as_bytes()))[..47].to_owned();
        ModeCheckedLocalSiteRendezvous::prepare_mode_checked(
            &base,
            SiteScope::parse(&scope).unwrap(),
        )
        .unwrap()
    }

    pub(super) fn run(&self, arguments: &[&str]) -> Output {
        self.run_with_caller(arguments, None, None)
    }

    pub(super) fn run_with_caller(
        &self,
        arguments: &[&str],
        terminal: Option<&str>,
        worker: Option<&str>,
    ) -> Output {
        let mut command = Command::new(self.0.join("orchbee.exe"));
        command
            .args(arguments)
            .env("ORCHBEE_RUNTIME_BASE", self.0.join("runtime"))
            .env("LC_ALL", "en_US.UTF-8")
            .env_remove("ORCHBEE_TERMINAL_ID")
            .env_remove("ORCHBEE_WORKER_ID")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(value) = terminal {
            command.env("ORCHBEE_TERMINAL_ID", value);
        }
        if let Some(value) = worker {
            command.env("ORCHBEE_WORKER_ID", value);
        }
        let child = command.spawn().unwrap();
        if let Some(launched) = self.1.lock().unwrap().take() {
            // The peer may already have accepted before CreateProcess returns.
            let _ = launched.send(Instant::now());
        }
        child.wait_with_output().unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // Wake an admission peer when child creation unwinds before notification.
        self.1.lock().unwrap().take();
        let target = self.0.canonicalize().unwrap();
        assert!(target.starts_with(std::env::temp_dir().canonicalize().unwrap()));
        assert!(
            self.0
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("orchbee-cli-names-")
        );
        fs::remove_dir_all(target).unwrap();
    }
}

pub(super) fn peer(
    fixture: &Fixture,
    listener: LocalListener,
    script: impl FnOnce(LocalStream) + Send + 'static,
) -> thread::JoinHandle<()> {
    listener.set_nonblocking(true).unwrap();
    let (launched, launch) = mpsc::channel();
    assert!(fixture.1.lock().unwrap().replace(launched).is_none());
    thread::spawn(move || {
        // OS process creation is fixture setup, not the CLI admission interval.
        let mut deadline = None;
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if deadline.is_none() {
                        match launch.try_recv() {
                            Ok(started) => deadline = Some(started + Duration::from_secs(15)),
                            Err(mpsc::TryRecvError::Empty) => {}
                            Err(mpsc::TryRecvError::Disconnected) => {
                                panic!("CLI launch cancelled before admission");
                            }
                        }
                    }
                    assert!(
                        deadline.is_none_or(|until| Instant::now() < until),
                        "CLI admission timeout"
                    );
                    thread::sleep(Duration::from_millis(5));
                }
                Err(error) => panic!("named-pipe accept: {error}"),
            }
        };
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        assert!(matches!(
            read_message(&mut stream).unwrap(),
            LocalClientMessage::ClientHello(_)
        ));
        write_message(
            &mut stream,
            &LocalClientMessage::ClientAccepted {
                generation: std::num::NonZeroU64::new(1).unwrap(),
            },
        )
        .unwrap();
        script(stream);
    })
}

pub(super) fn validate(document: &Value, source: &str) {
    let schema: Value = serde_json::from_str(source).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let errors: Vec<_> = validator
        .iter_errors(document)
        .map(|error| error.to_string())
        .collect();
    assert!(errors.is_empty(), "{document}: {errors:?}");
}

pub(super) fn session_detail(session_id: &str) -> serde_json::Value {
    serde_json::json!({
        "snapshot": {
            "sessionId": session_id,
            "providerId": "claude-code",
            "lifecycle": "live",
            "executionLocation": {"executionLocationId": "host:7"},
            "observation": {
                "fidelity": "rich",
                "freshness": "fresh",
                "generation": 3,
                "sequence": 11,
                "source": "native"
            },
            "parentSessionId": null,
            "activityHint": null,
            "capabilities": ["questions", "activity"],
            "lastOutcome": null,
            "pendingRequests": [{
                "actionPath": "navigate-only",
                "kind": "question",
                "requestId": "req-42",
                "turnId": "turn-9"
            }],
            "presentationAttachments": [{
                "generation": "gen-2",
                "navigable": true,
                "presentationId": "11111111111111111111111111111111",
                "visibility": "visible"
            }],
            "turn": {"kind": "active", "activity": "tool", "phase": "working", "turnId": "turn-9"}
        },
        "status": {"kind": "needs-attention", "count": 1}
    })
}

pub(super) fn resolve_reply(stream: &mut LocalStream, outcome: Value) -> String {
    let AgentRequest::Names(request) = read_agent_request(stream).unwrap() else {
        panic!("resolve first");
    };
    assert_eq!(json!(request)["kind"], "agent.session.resolve.request");
    let id = request.request_id().as_str().to_owned();
    let reply = AgentResult::Names(serde_json::from_value(json!({"kind":"agent.session.resolve.result", "requestId":id, "result":{"outcome":outcome}})).unwrap());
    write_agent_result(stream, &reply).unwrap();
    id
}

pub(super) fn read_reply(stream: &mut LocalStream, verb: &str, outcome: Value) -> String {
    let AgentRequest::Read(request) = read_agent_request(stream).unwrap() else {
        panic!("read only after resolution; never retarget a waiter");
    };
    assert_eq!(
        json!(request)["kind"],
        format!("agent.session.{verb}.request")
    );
    assert_eq!(json!(request)["sessionId"], "sess-alpha");
    let id = request.request_id().as_str().to_owned();
    let reply = AgentResult::Read(serde_json::from_value(json!({"kind":format!("agent.session.{verb}.result"), "requestId":id, "result":{"outcome":outcome}})).unwrap());
    write_agent_result(stream, &reply).unwrap();
    id
}

pub(super) fn assert_closed(stream: &mut LocalStream) {
    assert_eq!(
        stream.read(&mut [0]).unwrap(),
        0,
        "no extra exchange or replay"
    );
}

pub(super) fn bound_snapshot_reply(stream: &mut LocalStream) {
    use orchbee_workbench_protocol::*;
    let WorkbenchRequest::SnapshotRead(intent) = read_workbench_request(stream).unwrap() else {
        panic!("snapshot first");
    };
    let document = json!({
        "kind":"workbench.snapshot", "protocolVersion":1, "revision":"9", "siteId":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "navigation":{"projects":[],"standaloneTerminalContextId":"00000000000000000000000000000001"},
        "terminalContexts":[{"terminalContextId":"00000000000000000000000000000001", "groups":[], "tabs":[{
            "tabId":"00000000000000000000000000000002", "label":"tab", "pinned":false, "groupId":null,
            "root":{"kind":"panel", "panelId":"22222222222222222222222222222222",
                "primaryPresentation":{"presentationId":"11111111111111111111111111111111", "label":"primary",
                    "binding":{"terminalId":"33333333333333333333333333333333", "workerId":"6f7263686265652d6c6f63616c2d7631"}},
                "childPresentations":[{"presentationId":"44444444444444444444444444444444", "label":"child",
                    "binding":{"terminalId":"55555555555555555555555555555555", "workerId":"6f7263686265652d6c6f63616c2d7631"}}]
            }
        }]}]
    });
    let snapshot = parse_snapshot(&serde_json::to_vec(&document).unwrap()).unwrap();
    write_workbench_response(
        stream,
        &WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
            protocol_version: ProtocolVersion,
            request_id: intent.request_id,
            outcome: WorkbenchSnapshotOutcome::Available { snapshot },
        }),
    )
    .unwrap();
}

pub(super) fn bound_agent_reply(stream: &mut LocalStream, detail: Value) {
    bound_snapshot_reply(stream);
    resolve_reply(
        stream,
        json!({"kind":"resolved", "sessionId":"sess-alpha", "resolvedBy":"name"}),
    );
    read_reply(
        stream,
        "get",
        json!({"kind":"found", "revision":1, "session":detail}),
    );
}

pub(super) fn runtime_preflight(stream: &mut LocalStream, detail: Value) {
    let provider = detail["snapshot"]["providerId"].as_str().unwrap();
    process_reply(stream, vec![process(10, None, &format!("{provider}.exe"))]);
    read_reply(
        stream,
        "get",
        json!({"kind":"found","revision":2,"session":detail}),
    );
}

pub(super) fn process(
    pid: u32,
    parent_pid: Option<u32>,
    image: &str,
) -> orchbee_terminal_protocol::TerminalProcess {
    orchbee_terminal_protocol::TerminalProcess {
        pid,
        parent_pid,
        creation_time: u64::from(pid),
        image: Some(image.into()),
        command_line: Some(image.into()),
        current_directory: Some("C:\\工作 😀".into()),
    }
}

pub(super) fn process_reply(
    stream: &mut LocalStream,
    processes: Vec<orchbee_terminal_protocol::TerminalProcess>,
) {
    process_sample_reply(stream, Ok(processes));
}

pub(super) fn process_sample_reply(
    stream: &mut LocalStream,
    sample: Result<Vec<orchbee_terminal_protocol::TerminalProcess>, &str>,
) {
    use orchbee_client_server_protocol::{
        ClientMessage, ServerMessage, TerminalViewerAction, TerminalViewerOutcome,
        TerminalViewerResult, ViewerId, read_client_message, write_server_message,
    };
    use orchbee_terminal_protocol::{
        AttachmentId, CURRENT_PROTOCOL_VERSION, CorrelationId, Envelope, HostEpoch, MessageRole,
        PayloadEncoding, ProcessInfoControl, TerminalId, TerminalProcessInfo,
        encode_process_info_control, encode_transport_envelope,
    };
    let attachment = AttachmentId::from_bytes([8; 16]).unwrap();
    let viewer = ViewerId::parse(&attachment.to_text()).unwrap();
    let epoch = HostEpoch::from_bytes([6; 16]).unwrap();
    let (kind, payload) = match sample {
        Ok(processes) => {
            let control = encode_process_info_control(&ProcessInfoControl::ProcessInfo(
                TerminalProcessInfo {
                    attachment_id: attachment,
                    terminal_id: TerminalId::from_bytes([7; 16]).unwrap(),
                    root_pid: 10,
                    processes,
                },
            ))
            .unwrap();
            (control.kind, control.payload)
        }
        Err(code) => {
            let control = orchbee_terminal_protocol::encode_connection_control(
                &orchbee_terminal_protocol::ConnectionControl::TypedFailure(
                    orchbee_terminal_protocol::TypedFailure {
                        code: code.into(),
                        message: "untrusted diagnostic".into(),
                        retryable: false,
                    },
                ),
            )
            .unwrap();
            (control.kind, control.payload)
        }
    };
    let record = encode_transport_envelope(&Envelope {
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Response,
        kind,
        optional_message: false,
        payload_encoding: PayloadEncoding::ControlJson,
        correlation_id: Some(CorrelationId::from_bytes([9; 16]).unwrap()),
        host_epoch: Some(epoch),
        payload: &payload,
    })
    .unwrap();
    for (stage, outcome) in [
        (
            "attach",
            TerminalViewerOutcome::Attached {
                attachment_id: viewer.clone(),
                host_epoch: orchbee_client_server_protocol::HostEpoch::parse(&epoch.to_text())
                    .unwrap(),
            },
        ),
        ("query", TerminalViewerOutcome::Record(record)),
        ("detach", TerminalViewerOutcome::Detached {}),
    ] {
        let ClientMessage::TerminalViewer(request) = read_client_message(stream).unwrap() else {
            panic!("read-only process {stage} before any input");
        };
        match (stage, request.action()) {
            ("attach", TerminalViewerAction::AttachBinding { terminal_id, .. }) => {
                assert_eq!(terminal_id.as_str(), "33333333333333333333333333333333")
            }
            ("query", TerminalViewerAction::ReadProcessInfo { attachment_id })
            | ("detach", TerminalViewerAction::Detach { attachment_id }) => {
                assert_eq!(attachment_id, &viewer)
            }
            _ => panic!("unexpected preflight operation"),
        }
        write_server_message(
            stream,
            &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                request.request_id().clone(),
                outcome,
            )),
        )
        .unwrap();
    }
}
