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
    collections::HashMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

#[path = "windows_cli_cohort.rs"]
mod cohort;
#[path = "windows_agent_peer/process.rs"]
pub(super) mod process;
use process::Failure as ProcessFailure;

pub(super) struct Fixture(
    pub(super) PathBuf,
    Mutex<Option<PendingPeer>>,
    Arc<cohort::Cohort>,
);
struct PendingPeer {
    launched: mpsc::Sender<Instant>,
    state: Arc<PeerState>,
}
#[derive(Default)]
struct PeerState {
    cancelled: AtomicBool,
    diagnostic: Mutex<Option<String>>,
}

pub(super) struct PeerGuard<'fixture> {
    fixture: &'fixture Fixture,
    state: Arc<PeerState>,
    thread: Option<thread::JoinHandle<()>>,
}
impl PeerGuard<'_> {
    pub(super) fn join(mut self) -> thread::Result<()> {
        self.thread.take().expect("peer joined once").join()
    }
}
impl Drop for PeerGuard<'_> {
    fn drop(&mut self) {
        if let Some(handle) = self.thread.take() {
            self.state.cancelled.store(true, Ordering::Release);
            // A cancelled pre-launch peer must not poison a later invocation.
            let mut pending = self.fixture.1.lock().unwrap_or_else(|e| e.into_inner());
            if pending
                .as_ref()
                .is_some_and(|p| Arc::ptr_eq(&p.state, &self.state))
            {
                pending.take();
            }
            drop(pending);
            let _ = handle.join(); // Never panic a second time while unwinding.
        }
    }
}
struct CancelOnFailure(Arc<PeerState>);
impl Drop for CancelOnFailure {
    fn drop(&mut self) {
        if thread::panicking() {
            self.0.cancelled.store(true, Ordering::Release);
        }
    }
}

impl Fixture {
    pub(super) fn new() -> Self {
        Self::with_cohort(cohort::Cohort::shared())
    }

    #[allow(
        dead_code,
        reason = "Only cohort-corruption suites need private artifact copies."
    )]
    pub(super) fn private_cohort() -> Self {
        Self::with_cohort(Arc::new(cohort::Cohort::private()))
    }

    fn with_cohort(cohort: Arc<cohort::Cohort>) -> Self {
        let mut nonce = [0; 16];
        getrandom::fill(&mut nonce).unwrap();
        let root = std::env::temp_dir().join(format!(
            "orchbee-cli-names-{:032x}",
            u128::from_le_bytes(nonce)
        ));
        fs::create_dir(&root).unwrap();
        Self(root, Mutex::new(None), cohort)
    }

    #[allow(
        dead_code,
        reason = "Only cohort admission and relay suites inspect artifact paths."
    )]
    pub(super) fn cohort_directory(&self) -> &Path {
        self.2.directory()
    }

    pub(super) fn executable(&self) -> PathBuf {
        self.2.executable()
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
        self.run_with_env(arguments, terminal, worker, &[])
    }

    pub(super) fn run_with_env(
        &self,
        arguments: &[&str],
        terminal: Option<&str>,
        worker: Option<&str>,
        environment: &[(&str, &std::ffi::OsStr)],
    ) -> Output {
        self.run_process(arguments, terminal, worker, environment, None)
    }

    pub(super) fn command(
        &self,
        arguments: &[&str],
        terminal: Option<&str>,
        worker: Option<&str>,
        environment: &[(&str, &std::ffi::OsStr)],
    ) -> Command {
        let mut command = Command::new(self.executable());
        command
            .args(arguments)
            .env("ORCHBEE_RUNTIME_BASE", self.0.join("runtime"))
            .env("LC_ALL", "en_US.UTF-8")
            .env("APPDATA", self.0.join("appdata"))
            .env_remove("ORCHBEE_CONFIG_FILE")
            .env_remove("ORCHBEE_RUNTIME_SCOPE")
            .env_remove("ORCHBEE_SESSION")
            .env_remove("ORCHBEE_SITE")
            .env_remove("ORCHBEE_TERMINAL_ID")
            .env_remove("ORCHBEE_WORKER_ID");
        if let Some(value) = terminal {
            command.env("ORCHBEE_TERMINAL_ID", value);
        }
        if let Some(value) = worker {
            command.env("ORCHBEE_WORKER_ID", value);
        }
        command.envs(environment.iter().copied());
        command
    }

    pub(super) fn run_process(
        &self,
        arguments: &[&str],
        terminal: Option<&str>,
        worker: Option<&str>,
        environment: &[(&str, &std::ffi::OsStr)],
        input: Option<&[u8]>,
    ) -> Output {
        self.run_command(
            self.command(arguments, terminal, worker, environment),
            input,
            // Match the generic process fixture backstop and combined output cap.
            Duration::from_secs(45),
            8 * 1024 * 1024,
        )
    }

    pub(super) fn run_command(
        &self,
        command: Command,
        input: Option<&[u8]>,
        budget: Duration,
        output_limit: usize,
    ) -> Output {
        self.try_run_command(command, input, budget, output_limit)
            .unwrap_or_else(|error| panic!("Windows CLI fixture command: {error}"))
    }

    pub(super) fn try_run_command(
        &self,
        command: Command,
        input: Option<&[u8]>,
        budget: Duration,
        output_limit: usize,
    ) -> Result<Output, ProcessFailure> {
        let pending_peer = self.1.lock().unwrap().take();
        let no_peer_cancel = AtomicBool::new(false);
        let cancelled = pending_peer
            .as_ref()
            .map_or(&no_peer_cancel, |peer| &peer.state.cancelled);
        let result = process::run(command, input, budget, output_limit, cancelled, |started| {
            if let Some(peer) = &pending_peer {
                // The peer may have accepted before CreateProcess returned.
                let _ = peer.launched.send(started);
            }
        });
        if let Some(peer) = pending_peer {
            *peer.state.diagnostic.lock().unwrap() = Some(match &result {
                Ok(output) => {
                    process::diagnostic(Some(output.status), &output.stdout, &output.stderr)
                }
                Err(error) => error.to_string(),
            });
        }
        result
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(peer) = self.1.lock().unwrap_or_else(|e| e.into_inner()).take() {
            peer.state.cancelled.store(true, Ordering::Release);
        }
        remove_fixture_directory(&self.0, "orchbee-cli-names-");
    }
}

fn remove_fixture_directory(path: &Path, prefix: &str) {
    let result = (|| {
        let target = path.canonicalize()?;
        if !target.starts_with(std::env::temp_dir().canonicalize()?)
            || !target
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(prefix))
        {
            return Err(std::io::Error::other(
                "fixture cleanup path escaped its owned temporary root",
            ));
        }
        fs::remove_dir_all(target)
    })();
    if let Err(error) = result {
        let diagnostic = format!(
            "fixture owner could not release {} after peer/command cleanup attempts: {error}; the test runner retains cleanup responsibility",
            path.display()
        );
        if thread::panicking() {
            use std::io::Write;
            let _ = writeln!(std::io::stderr().lock(), "{diagnostic}");
        } else {
            panic!("{diagnostic}");
        }
    }
}

pub(super) fn peer(
    fixture: &Fixture,
    listener: LocalListener,
    script: impl FnOnce(LocalStream) + Send + 'static,
) -> PeerGuard<'_> {
    peer_with_budget(fixture, listener, Duration::from_secs(15), script)
}

pub(super) fn peer_with_budget(
    fixture: &Fixture,
    listener: LocalListener,
    admission_budget: Duration,
    script: impl FnOnce(LocalStream) + Send + 'static,
) -> PeerGuard<'_> {
    listener.set_nonblocking(true).unwrap();
    let (launched, launch) = mpsc::channel();
    let state = Arc::new(PeerState::default());
    let pending = PendingPeer {
        launched,
        state: Arc::clone(&state),
    };
    assert!(fixture.1.lock().unwrap().replace(pending).is_none());
    let peer_state = Arc::clone(&state);
    let handle = thread::spawn(move || {
        let _cancel_on_failure = CancelOnFailure(Arc::clone(&peer_state));
        // OS process creation is fixture setup, not the CLI admission interval.
        let mut deadline = None;
        let mut stream = loop {
            if peer_state.cancelled.load(Ordering::Acquire) {
                return;
            }
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if deadline.is_none() {
                        match launch.try_recv() {
                            Ok(started) => deadline = Some(started + admission_budget),
                            Err(mpsc::TryRecvError::Empty) => {}
                            Err(mpsc::TryRecvError::Disconnected) => {
                                panic!("CLI launch cancelled before admission");
                            }
                        }
                    }
                    if deadline.is_some_and(|until| Instant::now() >= until) {
                        let diagnostic = peer_state
                            .diagnostic
                            .lock()
                            .unwrap()
                            .clone()
                            .unwrap_or_else(|| {
                                "child output has not been collected; cancelling its Job".to_owned()
                            });
                        // Release the diagnostic lock before unwinding the peer.
                        panic!("CLI admission timeout: {diagnostic}");
                    }
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
    });
    PeerGuard {
        fixture,
        state,
        thread: Some(handle),
    }
}

pub(super) fn validate(document: &Value, source: &'static str) {
    static VALIDATORS: OnceLock<Mutex<HashMap<&'static str, Arc<jsonschema::Validator>>>> =
        OnceLock::new();
    let validator = {
        let mut validators = VALIDATORS.get_or_init(Mutex::default).lock().unwrap();
        Arc::clone(validators.entry(source).or_insert_with(|| {
            let schema: Value = serde_json::from_str(source).unwrap();
            Arc::new(jsonschema::validator_for(&schema).unwrap())
        }))
    };
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

#[allow(
    dead_code,
    reason = "Shared integration fixture helpers are consumed by separate test crate owners."
)]
pub(super) fn with_attachment_generation(
    mut detail: Value,
    presentation_id: &str,
    generation: &str,
) -> Value {
    let attachments = detail["snapshot"]["presentationAttachments"]
        .as_array_mut()
        .expect("canonical fixture attachments");
    let mut matched = 0;
    for attachment in attachments {
        if attachment["presentationId"].as_str() == Some(presentation_id) {
            attachment["generation"] = json!(generation);
            matched += 1;
        }
    }
    assert_eq!(
        matched, 1,
        "generation fixture needs one canonical attachment"
    );
    detail
}

pub(super) fn session_summary(detail: &Value) -> Value {
    let snapshot = &detail["snapshot"];
    let mut summary = json!({});
    for key in [
        "executionLocation",
        "lifecycle",
        "observation",
        "parentSessionId",
        "presentationAttachments",
        "providerId",
        "sessionId",
    ] {
        summary[key] = snapshot[key].clone();
    }
    summary["pendingRequestCount"] =
        json!(snapshot["pendingRequests"].as_array().map_or(0, Vec::len));
    summary["status"] = detail["status"].clone();
    summary
}

/// A strict unified-Agent source script. Every call consumes exactly the
/// canonical list, physical state, and fence requests owned by one read span.
#[allow(
    dead_code,
    reason = "Shared integration fixture methods are consumed by separate test crate owners."
)]
#[derive(Clone)]
pub(super) struct AgentProjectionFixture {
    snapshot: orchbee_workbench_protocol::Snapshot,
    canonical_revision: u64,
    report_revision: u64,
    summaries: Vec<Value>,
    states: HashMap<String, Option<Value>>,
}

#[allow(
    dead_code,
    reason = "Shared integration fixture methods are consumed by separate test crate owners."
)]
impl AgentProjectionFixture {
    pub(super) fn canonical(
        snapshot: orchbee_workbench_protocol::Snapshot,
        canonical_revision: u64,
        details: &[Value],
    ) -> Self {
        Self {
            snapshot,
            canonical_revision,
            report_revision: 7,
            summaries: details.iter().map(session_summary).collect(),
            states: HashMap::new(),
        }
    }

    pub(super) fn summaries(
        snapshot: orchbee_workbench_protocol::Snapshot,
        canonical_revision: u64,
        summaries: Vec<Value>,
    ) -> Self {
        Self {
            snapshot,
            canonical_revision,
            report_revision: 7,
            summaries,
            states: HashMap::new(),
        }
    }

    /// Give one canonical attachment a current terminal incarnation without
    /// claiming that the terminal report currently identifies an Agent.
    pub(super) fn canonical_with_current_incarnation(
        snapshot: orchbee_workbench_protocol::Snapshot,
        canonical_revision: u64,
        details: &[Value],
        presentation_id: &str,
        generation: &str,
        host_epoch: &str,
        host_terminal_id: &str,
    ) -> Self {
        let mut matched = 0;
        for detail in details {
            let attachments = detail["snapshot"]["presentationAttachments"]
                .as_array()
                .expect("canonical fixture attachments");
            for attachment in attachments {
                if attachment["presentationId"].as_str() == Some(presentation_id) {
                    assert_eq!(
                        attachment["generation"].as_str(),
                        Some(generation),
                        "current incarnation must match the canonical attachment"
                    );
                    matched += 1;
                }
            }
        }
        assert_eq!(
            matched, 1,
            "current-incarnation fixture needs one canonical attachment"
        );
        Self::canonical(snapshot, canonical_revision, details).with_state(
            presentation_id,
            Some(terminal_agent_state(
                presentation_id,
                generation,
                host_epoch,
                host_terminal_id,
                None,
                "unknown",
            )),
        )
    }

    pub(super) fn with_state(mut self, presentation_id: &str, state: Option<Value>) -> Self {
        assert!(
            self.states
                .insert(presentation_id.to_owned(), state)
                .is_none()
        );
        self
    }

    pub(super) fn read(&self, stream: &mut LocalStream, include_ended: bool) {
        self.list(stream, include_ended);
        self.states(stream, false);
        self.list(stream, include_ended);
        self.states(stream, true);
        self.snapshot(stream);
    }

    pub(super) fn confirm(&self, stream: &mut LocalStream, include_ended: bool) {
        self.list(stream, include_ended);
        self.states(stream, true);
        self.snapshot(stream);
    }

    pub(super) fn canonical_drift(
        &self,
        stream: &mut LocalStream,
        include_ended: bool,
        revision: u64,
        details: &[Value],
    ) {
        let changed = Self::canonical(self.snapshot.clone(), revision, details);
        changed.list(stream, include_ended);
    }

    pub(super) fn workbench_drift(
        &self,
        stream: &mut LocalStream,
        include_ended: bool,
        revision: u64,
    ) {
        self.list(stream, include_ended);
        self.states(stream, true);
        let changed = Self {
            snapshot: orchbee_workbench_protocol::Snapshot {
                revision: orchbee_workbench_protocol::WorkbenchRevision::new(revision),
                ..self.snapshot.clone()
            },
            ..self.clone()
        };
        changed.snapshot(stream);
    }

    fn list(&self, stream: &mut LocalStream, include_ended: bool) {
        let AgentRequest::Read(request) = read_agent_request(stream).unwrap() else {
            panic!("unified projection canonical list")
        };
        assert_eq!(json!(request)["kind"], "agent.session.list.request");
        assert_eq!(json!(request)["includeEnded"], include_ended);
        assert_eq!(json!(request)["limit"], 100);
        assert!(json!(request)["cursor"].is_null());
        write_agent_result(
            stream,
            &AgentResult::Read(
                serde_json::from_value(json!({
                    "kind":"agent.session.list.result",
                    "requestId":request.request_id(),
                    "result":{"outcome":{"kind":"listed",
                        "revision":self.canonical_revision,
                        "nextCursor":null,
                        "items":self.summaries}}
                }))
                .unwrap(),
            ),
        )
        .unwrap();
    }

    fn states(&self, stream: &mut LocalStream, fence: bool) {
        use orchbee_workbench_protocol::*;
        let WorkbenchRequest::PanelAgent(request) = read_workbench_request(stream).unwrap() else {
            panic!("unified projection physical states")
        };
        let PanelAgentAction::States { presentation_ids } = request.action else {
            panic!("unified projection never performs a Panel Agent effect")
        };
        assert_eq!(presentation_ids.is_empty(), fence);
        if !fence {
            fn collect(node: &PanelNode, result: &mut Vec<TerminalPresentationId>) {
                match node {
                    PanelNode::Panel {
                        primary_presentation,
                        child_presentations,
                        ..
                    } => result.extend(
                        std::iter::once(primary_presentation)
                            .chain(child_presentations)
                            .map(|member| member.presentation_id.clone()),
                    ),
                    PanelNode::Split { children, .. } => {
                        for child in children.iter() {
                            collect(&child.node, result);
                        }
                    }
                }
            }
            let mut expected = Vec::new();
            for tab in self
                .snapshot
                .terminal_contexts
                .iter()
                .flat_map(|context| &context.tabs)
            {
                collect(&tab.root, &mut expected);
            }
            assert_eq!(presentation_ids, expected);
        }
        let presentations = presentation_ids
            .into_iter()
            .map(|presentation_id| TerminalAgentStateRow {
                snapshot: self
                    .states
                    .get(presentation_id.as_str())
                    .cloned()
                    .flatten()
                    .map(serde_json::from_value)
                    .transpose()
                    .unwrap(),
                presentation_id,
            })
            .collect();
        write_workbench_response(
            stream,
            &WorkbenchResponse::PanelAgent(PanelAgentResult {
                protocol_version: ProtocolVersion,
                request_id: request.request_id,
                site_id: request.site_id,
                outcome: PanelAgentOutcome::States {
                    workbench_revision: self.snapshot.revision,
                    report_revision: WorkbenchRevision::new(self.report_revision),
                    presentations,
                },
            }),
        )
        .unwrap();
    }

    fn snapshot(&self, stream: &mut LocalStream) {
        use orchbee_workbench_protocol::*;
        let WorkbenchRequest::SnapshotRead(request) = read_workbench_request(stream).unwrap()
        else {
            panic!("unified projection final Workbench fence")
        };
        write_workbench_response(
            stream,
            &WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
                protocol_version: ProtocolVersion,
                request_id: request.request_id,
                outcome: WorkbenchSnapshotOutcome::Available {
                    snapshot: self.snapshot.clone(),
                },
            }),
        )
        .unwrap();
    }
}

#[allow(
    dead_code,
    reason = "Shared integration fixture helpers are consumed by separate test crate owners."
)]
pub(super) fn terminal_agent_state(
    presentation_id: &str,
    generation: &str,
    host_epoch: &str,
    host_terminal_id: &str,
    provider: Option<&str>,
    state: &str,
) -> Value {
    json!({
        "binding": {
            "presentationId": presentation_id,
            "generation": generation,
            "hostEpoch": host_epoch,
            "hostTerminalId": host_terminal_id,
        },
        "sequence": "4",
        "agent": provider,
        "state": state,
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
    assert_closed_for(stream, "peer");
}

pub(super) fn assert_closed_for(stream: &mut LocalStream, context: &str) {
    assert_eq!(
        stream.read(&mut [0]).unwrap(),
        0,
        "{context}: no extra exchange or replay"
    );
}

pub(super) fn bound_snapshot_reply(stream: &mut LocalStream) {
    bound_snapshot_at(stream, 9);
}
pub(super) fn bound_snapshot(revision: u64) -> orchbee_workbench_protocol::Snapshot {
    use orchbee_workbench_protocol::parse_snapshot;
    let document = json!({
        "kind":"workbench.snapshot", "protocolVersion":1, "revision":revision.to_string(), "siteId":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "navigation":{"projects":[], "directories":[], "workspaces":[{"workspaceId":"00000000000000000000000000000001","terminalContextId":"00000000000000000000000000000001","projectId":null,"primaryDirectoryId":null,"label":"Workspace"}]},
        "terminalContexts":[{"terminalContextId":"00000000000000000000000000000001", "groups":[], "tabs":[{
            "tabId":"00000000000000000000000000000002", "label":"tab", "pinned":false, "groupId":null,
            "root":{"kind":"panel", "panelId":"22222222222222222222222222222222",
                "primaryPresentation":{"customLabel":null,"presentationId":"11111111111111111111111111111111", "label":"primary",
                    "binding":{"terminalId":"33333333333333333333333333333333", "workerId":"6f7263686265652d6c6f63616c2d7631"}},
                "childPresentations":[{"customLabel":null,"presentationId":"44444444444444444444444444444444", "label":"child",
                    "binding":{"terminalId":"55555555555555555555555555555555", "workerId":"6f7263686265652d6c6f63616c2d7631"}}]
            }
        }]}]
    });
    parse_snapshot(&serde_json::to_vec(&document).unwrap()).unwrap()
}
pub(super) fn bound_snapshot_at(stream: &mut LocalStream, revision: u64) {
    use orchbee_workbench_protocol::*;
    let snapshot = bound_snapshot(revision);
    let intent = snapshot_after_metadata(stream, &bound_snapshot(9));
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

pub(super) fn management_reply(stream: &mut LocalStream, revision: u64, management: Value) {
    let AgentRequest::Launch(request) = read_agent_request(stream).unwrap() else {
        panic!("startup status read")
    };
    assert_eq!(json!(request)["kind"], "agent.launch.status.request");
    assert_eq!(json!(request)["sessionId"], "sess-alpha");
    write_agent_result(stream,&AgentResult::Launch(serde_json::from_value(json!({"kind":"agent.launch.status.result","requestId":request.request_id(),"result":{"outcome":{
        "kind":"status","sessionId":"sess-alpha","revision":revision,"management":management
    }}})).unwrap())).unwrap();
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
    process_sample_for(stream, sample, "33333333333333333333333333333333");
}
pub(super) fn process_sample_for(
    stream: &mut LocalStream,
    sample: Result<Vec<orchbee_terminal_protocol::TerminalProcess>, &str>,
    expected_terminal: &str,
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
                host_terminal_id: orchbee_client_server_protocol::HostTerminalId::parse(
                    &epoch.to_text(),
                )
                .unwrap(),
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
                assert_eq!(terminal_id.as_str(), expected_terminal)
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

/// Only correlated read batches, then a final empty revision fence; writes always fail this fixture.
pub(super) fn snapshot_after_metadata(
    stream: &mut LocalStream,
    snapshot: &orchbee_workbench_protocol::Snapshot,
) -> orchbee_workbench_protocol::SnapshotReadIntent {
    use orchbee_workbench_protocol::*;
    fn binding(node: &PanelNode, id: &WorkbenchPanelId) -> Option<Option<PresentationBinding>> {
        match node {
            PanelNode::Panel {
                panel_id,
                primary_presentation,
                ..
            } if panel_id == id => Some(primary_presentation.binding.clone()),
            PanelNode::Panel { .. } => None,
            PanelNode::Split { children, .. } => {
                children.iter().find_map(|child| binding(&child.node, id))
            }
        }
    }
    fn presentation(
        node: &PanelNode,
        id: &TerminalPresentationId,
    ) -> Option<(WorkbenchPanelId, Option<PresentationBinding>)> {
        match node {
            PanelNode::Panel {
                panel_id,
                primary_presentation,
                child_presentations,
                ..
            } => std::iter::once(primary_presentation)
                .chain(child_presentations)
                .find(|member| &member.presentation_id == id)
                .map(|member| (panel_id.clone(), member.binding.clone())),
            PanelNode::Split { children, .. } => children
                .iter()
                .find_map(|slot| presentation(&slot.node, id)),
        }
    }
    let exact = |id: &TerminalPresentationId| {
        snapshot
            .terminal_contexts
            .iter()
            .flat_map(|context| &context.tabs)
            .find_map(|tab| presentation(&tab.root, id))
            .expect("known member")
    };
    let mut reads = 0;
    let mut fenced = false;
    let mut report_reads = 0;
    let mut report_fenced = false;
    loop {
        match read_workbench_request(stream).unwrap() {
            WorkbenchRequest::SnapshotRead(request) => {
                assert!(
                    reads == 0 || (fenced && report_fenced),
                    "missing metadata/report fence"
                );
                return request;
            }
            WorkbenchRequest::PanelMetadata(request) => {
                assert!(!fenced, "no reads after metadata fence");
                assert_eq!(request.site_id, snapshot.site_id);
                if let PanelMetadataAction::ReadPresentations { presentation_ids } = &request.action
                {
                    assert!(!presentation_ids.is_empty());
                    reads += 1;
                    let presentations = presentation_ids
                        .iter()
                        .map(|id| {
                            let (panel_id, binding) = exact(id);
                            PresentationMetadataRow {
                                presentation_id: id.clone(),
                                panel: PanelMetadataRow {
                                    panel_id,
                                    binding,
                                    tokens: Default::default(),
                                    presentations: vec![],
                                },
                            }
                        })
                        .collect();
                    write_workbench_response(
                        stream,
                        &WorkbenchResponse::PanelMetadata(PanelMetadataResult {
                            protocol_version: ProtocolVersion,
                            request_id: request.request_id,
                            site_id: request.site_id,
                            outcome: PanelMetadataOutcome::PresentationsRead {
                                workbench_revision: snapshot.revision,
                                metadata_revision: WorkbenchRevision::new(0),
                                presentations,
                            },
                        }),
                    )
                    .unwrap();
                    continue;
                }
                let PanelMetadataAction::Read { panel_ids } = &request.action else {
                    panic!("query must never report metadata")
                };
                fenced = panel_ids.is_empty();
                reads += 1;
                let panels = panel_ids
                    .iter()
                    .map(|id| PanelMetadataRow {
                        panel_id: id.clone(),
                        binding: snapshot
                            .terminal_contexts
                            .iter()
                            .flat_map(|context| &context.tabs)
                            .find_map(|tab| binding(&tab.root, id))
                            .expect("known panel"),
                        tokens: Default::default(),
                        presentations: vec![],
                    })
                    .collect();
                write_workbench_response(
                    stream,
                    &WorkbenchResponse::PanelMetadata(PanelMetadataResult {
                        protocol_version: ProtocolVersion,
                        request_id: request.request_id,
                        site_id: request.site_id,
                        outcome: PanelMetadataOutcome::Read {
                            workbench_revision: snapshot.revision,
                            metadata_revision: WorkbenchRevision::new(0),
                            panels,
                        },
                    }),
                )
                .unwrap();
            }
            WorkbenchRequest::PanelAgent(request) => {
                assert!(!report_fenced, "no reads after report fence");
                assert_eq!(request.site_id, snapshot.site_id);
                if let PanelAgentAction::ReadPresentations { presentation_ids } = &request.action {
                    assert!(!presentation_ids.is_empty());
                    report_reads += 1;
                    let presentations = presentation_ids
                        .iter()
                        .map(|id| {
                            let (panel_id, binding) = exact(id);
                            let report = binding.as_ref().map(|_| PanelAgentView {
                                agent: None,
                                state: PanelAgentState::Unknown,
                                authority_source: None,
                                message: None,
                            });
                            PresentationAgentRow {
                                presentation_id: id.clone(),
                                panel: PanelAgentRow {
                                    panel_id,
                                    binding,
                                    report,
                                },
                            }
                        })
                        .collect();
                    write_workbench_response(
                        stream,
                        &WorkbenchResponse::PanelAgent(PanelAgentResult {
                            protocol_version: ProtocolVersion,
                            request_id: request.request_id,
                            site_id: request.site_id,
                            outcome: PanelAgentOutcome::PresentationsRead {
                                workbench_revision: snapshot.revision,
                                report_revision: WorkbenchRevision::new(0),
                                presentations,
                            },
                        }),
                    )
                    .unwrap();
                    continue;
                }
                let PanelAgentAction::Read { panel_ids } = &request.action else {
                    panic!("query must never report Agent state")
                };
                report_fenced = report_reads > 0 && panel_ids.is_empty();
                report_reads += 1;
                let panels = panel_ids
                    .iter()
                    .map(|id| {
                        let binding = snapshot
                            .terminal_contexts
                            .iter()
                            .flat_map(|context| &context.tabs)
                            .find_map(|tab| binding(&tab.root, id))
                            .expect("known panel");
                        let report = binding.as_ref().map(|_| PanelAgentView {
                            agent: None,
                            state: PanelAgentState::Unknown,
                            authority_source: None,
                            message: None,
                        });
                        PanelAgentRow {
                            panel_id: id.clone(),
                            binding,
                            report,
                        }
                    })
                    .collect();
                write_workbench_response(
                    stream,
                    &WorkbenchResponse::PanelAgent(PanelAgentResult {
                        protocol_version: ProtocolVersion,
                        request_id: request.request_id,
                        site_id: request.site_id,
                        outcome: PanelAgentOutcome::Read {
                            workbench_revision: snapshot.revision,
                            report_revision: WorkbenchRevision::new(0),
                            panels,
                        },
                    }),
                )
                .unwrap();
            }
            _ => panic!("snapshot or read-only Panel metadata"),
        }
    }
}
