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
    ] {
        let output = fixture.run(&[arguments.as_slice(), &["--format", "json"]].concat());
        assert_eq!(output.status.code(), Some(2));
        assert!(!fixture.0.join("runtime").exists());
    }
}
