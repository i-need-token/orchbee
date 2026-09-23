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
    thread,
    time::{Duration, Instant},
};

pub(super) struct Fixture(pub(super) PathBuf);
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
        Self(root)
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
        Command::new(self.0.join("orchbee.exe"))
            .args(arguments)
            .env("ORCHBEE_RUNTIME_BASE", self.0.join("runtime"))
            .env("LC_ALL", "en_US.UTF-8")
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
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
    listener: LocalListener,
    script: impl FnOnce(LocalStream) + Send + 'static,
) -> thread::JoinHandle<()> {
    listener.set_nonblocking(true).unwrap();
    thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline, "CLI admission timeout");
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
            "providerId": "acme.agent",
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
