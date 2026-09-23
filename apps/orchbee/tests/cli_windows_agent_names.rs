//! Real Windows CLI + same-user named pipe, OBLC admission, and OBAP names
//! evidence. The scripted peer never starts a Site or a provider process.
#![cfg(windows)]

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

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
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

    fn rendezvous(&self) -> ModeCheckedLocalSiteRendezvous {
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

    fn run(&self, arguments: &[&str]) -> Output {
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

fn peer(
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

fn validate(document: &Value, source: &str) {
    let schema: Value = serde_json::from_str(source).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    assert!(validator.is_valid(document), "{document}");
}

#[test]
fn names_commands_exchange_correlated_typed_results_on_windows() {
    let fixture = Fixture::new();
    let rendezvous = fixture.rendezvous();
    let cases: &[(&[&str], &str, Value, &str)] = &[
        (
            &["agent", "names"],
            "names.list",
            json!({"kind":"listed","bindings":[{"name":"alpha","sessionId":"s1","lifecycle":"live"}]}),
            include_str!("../src/cli/schemas/orchbee.agent.names.json"),
        ),
        (
            &["agent", "resolve", "--name", "alpha"],
            "resolve",
            json!({"kind":"resolved","sessionId":"s1","resolvedBy":"name"}),
            include_str!("../src/cli/schemas/orchbee.agent.resolve.json"),
        ),
        (
            &["agent", "rename", "--session", "s1", "alpha"],
            "name.assign",
            json!({"kind":"assigned","sessionId":"s1","name":"alpha","previousName":null}),
            include_str!("../src/cli/schemas/orchbee.agent.rename.json"),
        ),
        (
            &["agent", "rename", "alpha", "--clear"],
            "name.release",
            json!({"kind":"released","sessionId":"s1","name":"alpha"}),
            include_str!("../src/cli/schemas/orchbee.agent.rename.json"),
        ),
    ];
    for (arguments, operation, outcome, schema) in cases {
        let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
        let operation = (*operation).to_owned();
        let outcome = outcome.clone();
        let expected = outcome.clone();
        let handler = peer(listener, move |mut stream| {
            let AgentRequest::Names(request) = read_agent_request(&mut stream).unwrap() else {
                panic!("names request");
            };
            let request_document = json!(request);
            assert_eq!(
                request_document["kind"],
                format!("agent.session.{operation}.request")
            );
            validate(
                &request_document,
                include_str!("../../../schemas/agent-observation/v1/agent-observation.schema.json"),
            );
            let response = json!({"kind":format!("agent.session.{operation}.result"),"requestId":request.request_id(),"result":{"outcome":outcome}});
            validate(
                &response,
                include_str!("../../../schemas/agent-observation/v1/agent-observation.schema.json"),
            );
            let result = AgentResult::Names(serde_json::from_value(response).unwrap());
            write_agent_result(&mut stream, &result).unwrap();
            let mut byte = [0];
            assert_eq!(
                stream.read(&mut byte).unwrap(),
                0,
                "one request only, then close"
            );
        });
        let arguments = [*arguments, &["--format", "json"]].concat();
        let output = fixture.run(&arguments);
        handler.join().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(&document, schema);
        assert_eq!(document["result"], expected);
    }
}

#[test]
fn lost_and_mismatched_mutation_replies_are_unknown_without_replay() {
    let fixture = Fixture::new();
    let rendezvous = fixture.rendezvous();
    for wrong_reply in [false, true] {
        let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
        let handler = peer(listener, move |mut stream| {
            let AgentRequest::Names(request) = read_agent_request(&mut stream).unwrap() else {
                panic!("names request");
            };
            if wrong_reply {
                let result = AgentResult::Names(serde_json::from_value(json!({"kind":"agent.session.names.list.result","requestId":request.request_id(),"result":{"outcome":{"kind":"listed","bindings":[]}}})).unwrap());
                write_agent_result(&mut stream, &result).unwrap();
                let mut byte = [0];
                assert_eq!(stream.read(&mut byte).unwrap(), 0);
            }
        });
        let output = fixture.run(&["agent", "rename", "s1", "alpha", "--format", "json"]);
        handler.join().unwrap();
        assert_eq!(output.status.code(), Some(8));
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(
            &document,
            include_str!("../src/cli/schemas/orchbee.cli.failure.agent-names.json"),
        );
        assert_eq!(document["failure"]["kind"], "outcome-unknown");
        assert!(document.get("source").is_none());
    }
}

#[test]
fn invalid_names_and_conflicting_selectors_fail_before_runtime_creation() {
    let fixture = Fixture::new();
    for arguments in [
        vec!["agent", "rename", "s1", "Upper"],
        vec!["agent", "resolve", "s1", "--name", "alpha"],
        vec!["agent", "rename", "--clear"],
        vec!["agent", "names", "--clear"],
        vec!["agent", "get", "s1", "--name", "alpha"],
        vec!["agent", "wait", "--name", "Upper"],
        vec!["agent", "explain", "--name", "alpha", "--session", "s1"],
    ] {
        let output = fixture.run(&[arguments.as_slice(), &["--format", "json"]].concat());
        assert_eq!(output.status.code(), Some(2));
        assert!(!fixture.0.join("runtime").exists());
    }
}

fn session_detail(session_id: &str) -> serde_json::Value {
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

fn explanation() -> serde_json::Value {
    serde_json::json!({
        "actionPath": "navigate-only",
        "capabilityCeiling": ["questions", "activity"],
        "contentAvailable": false,
        "fidelity": "rich",
        "freshness": "fresh",
        "freshnessMeaning": "current-observation",
        "generation": 3,
        "sequence": 11,
        "source": "native",
        "status": {"kind": "needs-attention", "count": 1},
        "statusBasis": "attention-request"
    })
}

fn resolve_reply(stream: &mut LocalStream, outcome: Value) -> String {
    let AgentRequest::Names(request) = read_agent_request(stream).unwrap() else {
        panic!("resolve first");
    };
    assert_eq!(json!(request)["kind"], "agent.session.resolve.request");
    let id = request.request_id().as_str().to_owned();
    let reply = AgentResult::Names(serde_json::from_value(json!({"kind":"agent.session.resolve.result", "requestId":id, "result":{"outcome":outcome}})).unwrap());
    write_agent_result(stream, &reply).unwrap();
    id
}

fn read_reply(stream: &mut LocalStream, verb: &str, outcome: Value) -> String {
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

fn assert_closed(stream: &mut LocalStream) {
    assert_eq!(
        stream.read(&mut [0]).unwrap(),
        0,
        "no extra exchange or replay"
    );
}

#[test]
fn named_get_and_explain_share_one_admission_and_publish_only_the_read_source() {
    let fixture = Fixture::new();
    let rendezvous = fixture.rendezvous();
    for verb in ["get", "explain"] {
        let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
        let handler = peer(listener, move |mut stream| {
            let resolution = resolve_reply(
                &mut stream,
                json!({"kind":"resolved","sessionId":"sess-alpha","resolvedBy":"name"}),
            );
            let mut outcome =
                json!({"kind":"found","revision":7,"session":session_detail("sess-alpha")});
            if verb == "explain" {
                outcome["explanation"] = explanation();
            }
            let read = read_reply(&mut stream, verb, outcome);
            assert_ne!(resolution, read);
            assert_closed(&mut stream);
        });
        let output = fixture.run(&["agent", verb, "alpha", "--format", "json"]);
        handler.join().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(document["source"]["kind"], "agent-read");
        assert_eq!(
            document["result"]["session"]["snapshot"]["sessionId"],
            "sess-alpha"
        );
        validate(
            &document,
            if verb == "get" {
                include_str!("../src/cli/schemas/orchbee.agent.get.json")
            } else {
                include_str!("../src/cli/schemas/orchbee.agent.explain.json")
            },
        );
    }
}

#[test]
fn named_wait_resolves_once_and_keeps_polling_the_original_session() {
    let fixture = Fixture::new();
    let rendezvous = fixture.rendezvous();
    let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
    let handler = peer(listener, move |mut stream| {
        resolve_reply(
            &mut stream,
            json!({"kind":"resolved","sessionId":"sess-alpha","resolvedBy":"name"}),
        );
        // A quiet first observation keeps the wait alive. After this response
        // a reused alias could represent another session. The pinned-ID contract
        // forbids a second Resolve; read_reply rejects it before answering.
        let mut first = session_detail("sess-alpha");
        first["snapshot"]["pendingRequests"] = json!([]);
        first["status"] = json!({"kind":"working","activity":"tool","phase":"working"});
        read_reply(
            &mut stream,
            "get",
            json!({"kind":"found","revision":7,"session":first}),
        );
        read_reply(
            &mut stream,
            "get",
            json!({"kind":"found","revision":8,"session":session_detail("sess-alpha")}),
        );
        assert_closed(&mut stream);
    });
    let output = fixture.run(&[
        "agent",
        "wait",
        "--name",
        "alpha",
        "--timeout",
        "2000",
        "--format",
        "json",
    ]);
    handler.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    validate(
        &document,
        include_str!("../src/cli/schemas/orchbee.agent.wait.json"),
    );
    assert_eq!(
        document["result"]["session"]["snapshot"]["sessionId"],
        "sess-alpha"
    );
    assert_eq!(document["result"]["wait"]["observations"], 2);
}

#[test]
fn unresolved_name_publishes_its_own_failure_without_get_or_wait_poll() {
    let fixture = Fixture::new();
    let rendezvous = fixture.rendezvous();
    for verb in ["get", "explain", "wait"] {
        let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
        let handler = peer(listener, move |mut stream| {
            resolve_reply(&mut stream, json!({"kind":"rejected","reason":"not-found"}));
            assert_closed(&mut stream);
        });
        let output = fixture.run(&["agent", verb, "--name", "missing", "--format", "json"]);
        handler.join().unwrap();
        assert_eq!(output.status.code(), Some(3));
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        validate(
            &document,
            include_str!("../src/cli/schemas/orchbee.cli.failure.agent-names.json"),
        );
        assert_eq!(document["source"]["kind"], "agent-names");
        assert_eq!(document["failure"]["kind"], "not-found");
    }
}

#[test]
fn explicit_session_bypasses_name_resolution_even_for_a_valid_alias_spelling() {
    let fixture = Fixture::new();
    let rendezvous = fixture.rendezvous();
    let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
    let handler = peer(listener, move |mut stream| {
        read_reply(
            &mut stream,
            "get",
            json!({"kind":"found","revision":7,"session":session_detail("sess-alpha")}),
        );
        assert_closed(&mut stream);
    });
    let output = fixture.run(&[
        "agent",
        "get",
        "--session",
        "sess-alpha",
        "--format",
        "json",
    ]);
    handler.join().unwrap();
    assert!(output.status.success());
}

#[test]
fn named_get_resolution_and_read_share_the_original_five_second_deadline() {
    let fixture = Fixture::new();
    let rendezvous = fixture.rendezvous();
    let listener = LocalListener::bind(rendezvous.client_socket_path()).unwrap();
    let handler = peer(listener, move |mut stream| {
        // Spending most of the command budget on resolution must leave only
        // the remainder for Get; renewing another five seconds would fail.
        let AgentRequest::Names(request) = read_agent_request(&mut stream).unwrap() else {
            panic!("resolve");
        };
        let exchange_started = Instant::now();
        thread::sleep(Duration::from_millis(3200));
        let reply = AgentResult::Names(serde_json::from_value(json!({"kind":"agent.session.resolve.result","requestId":request.request_id(),"result":{"outcome":{"kind":"resolved","sessionId":"sess-alpha","resolvedBy":"name"}}})).unwrap());
        write_agent_result(&mut stream, &reply).unwrap();
        assert!(matches!(
            read_agent_request(&mut stream).unwrap(),
            AgentRequest::Read(_)
        ));
        assert_closed(&mut stream);
        let elapsed = exchange_started.elapsed();
        // Measure from the peer-observed Resolve, excluding process creation,
        // Defender scanning and output finalization outside this wire budget.
        assert!(
            elapsed < Duration::from_millis(7250),
            "exchange budget renewed: {elapsed:?}"
        );
    });
    let output = fixture.run(&["agent", "get", "--name", "alpha", "--format", "json"]);
    handler.join().unwrap();
    assert_eq!(output.status.code(), Some(7));
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    validate(
        &document,
        include_str!("../src/cli/schemas/orchbee.cli.failure.agent.json"),
    );
    assert_eq!(document["failure"]["kind"], "timeout");
    assert!(document.get("source").is_none());
}
