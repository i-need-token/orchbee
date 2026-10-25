//! Real-Site witnesses for the CLI query surface: every invocation runs the
//! source-pinned staged `orchbee` binary against a real `orchbee-server`
//! (and, where content matters, a real Worker/Host behind a live TUI client).
//! The query path must read shared state without creating, mutating, or
//! leaking anything on the Site.

use super::*;
use nix::poll::{PollFd, PollFlags, PollTimeout, poll};
use std::collections::BTreeMap;
use std::os::fd::AsFd;
use std::path::{Path, PathBuf};
use std::process::{Child, Output};

const SLOT_RELEASE_DEADLINE: Duration = Duration::from_secs(15);
const BLOCKED_STDOUT_DEADLINE: Duration = Duration::from_secs(25);

/// A spawned child that is always reaped: any `?` early return still kills
/// and waits it instead of leaking a zombie.
pub(super) struct Reap(Child);

impl Reap {
    pub(super) fn spawn(command: &mut Command) -> io::Result<Self> {
        command.spawn().map(Self)
    }
}

impl std::ops::Deref for Reap {
    type Target = Child;

    fn deref(&self) -> &Child {
        &self.0
    }
}

impl std::ops::DerefMut for Reap {
    fn deref_mut(&mut self) -> &mut Child {
        &mut self.0
    }
}

impl Drop for Reap {
    fn drop(&mut self) {
        if self.0.try_wait().is_ok_and(|status| status.is_none()) {
            let _result = self.0.kill();
        }
        let _result = self.0.wait();
    }
}

/// Run one query invocation of the staged `orchbee` binary. stdin is null and
/// both streams are pipes: every witness here is non-TTY.
pub(super) fn run_cli(fixture: &Fixture, arguments: &[&str]) -> Output {
    Command::new(fixture.cohort.join("orchbee"))
        .args(arguments)
        .env("ORCHBEE_RUNTIME_BASE", &fixture.base)
        .env("LANG", "en_US.UTF-8")
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .stdin(Stdio::null())
        .output()
        .unwrap_or_else(|error| panic!("spawn query CLI {arguments:?}: {error}"))
}

pub(super) fn run_json(fixture: &Fixture, arguments: &[&str]) -> (Output, serde_json::Value) {
    let output = run_cli(fixture, arguments);
    let document: serde_json::Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("{}: {error}", describe_output(arguments, &output)));
    (output, document)
}

/// Bounded one-line rendering of a finished CLI call: the arguments, exit
/// status, and both streams (each capped so a panic message stays small).
/// A failed setup or parse then carries the typed failure envelope — and its
/// private diagnostic text — instead of a bare exit code.
fn describe_output(arguments: &[&str], output: &Output) -> String {
    const STREAM_BOUND: usize = 4096;
    let bounded = |bytes: &[u8]| {
        let text = String::from_utf8_lossy(bytes);
        match text.char_indices().nth(STREAM_BOUND) {
            Some((boundary, _)) => format!("{}…<truncated>", &text[..boundary]),
            None => text.into_owned(),
        }
    };
    format!(
        "{arguments:?} exited {:?}; stdout: {}; stderr: {}",
        output.status.code(),
        bounded(&output.stdout),
        bounded(&output.stderr),
    )
}

/// Assert a captured CLI call exited 0. The panic keeps the bounded
/// arguments, status, and both streams — a setup failure reports the typed
/// failure document, not just `left: Some(1)`.
pub(super) fn expect_success(arguments: &[&str], output: &Output) {
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        describe_output(arguments, output)
    );
}

/// One `api schema` call returns every embedded schema; compile them all once.
pub(super) fn validators(fixture: &Fixture) -> BTreeMap<String, jsonschema::Validator> {
    let (output, document) = run_json(fixture, &["api", "schema", "--format", "json"]);
    expect_success(&["api", "schema", "--format", "json"], &output);
    document["result"]["schemas"]
        .as_object()
        .expect("schemas map")
        .iter()
        .map(|(id, schema)| {
            (
                id.clone(),
                jsonschema::draft202012::new(schema).expect("embedded schema compiles"),
            )
        })
        .collect()
}

pub(super) fn validate(
    validators: &BTreeMap<String, jsonschema::Validator>,
    id: &str,
    output: &Output,
) {
    let document: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        validators[id].is_valid(&document),
        "{id} rejected {document}"
    );
}

/// Start a real managed Server on the fixture scope and wait for its client
/// socket. No Worker or Host: a query only needs the Server surface.
pub(super) fn spawn_real_server(fixture: &Fixture) -> Reap {
    spawn_real_server_env(fixture, &[])
}

/// `spawn_real_server` with extra environment on the Server `Command` only —
/// the managed Worker inherits it (its own spawn never clears the
/// environment). Used to point the Worker's default-shell admission at a
/// fixture-private executable; nothing process-global is mutated.
pub(super) fn spawn_real_server_env(fixture: &Fixture, env: &[(&str, &Path)]) -> Reap {
    let site = fixture.site();
    site.commit_activation().expect("fixture activation");
    let mut command = Command::new(fixture.cohort.join("orchbee-server"));
    for (name, value) in env {
        command.env(name, value);
    }
    let mut server = Reap(
        command
            .arg("--local-site")
            .arg(fixture.runtime())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn orchbee-server"),
    );
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if site.client_socket_exists().unwrap_or(false) {
            break;
        }
        assert!(server.try_wait().unwrap().is_none(), "server exited early");
        assert!(Instant::now() < deadline, "server socket deadline");
        thread::yield_now();
    }
    server
}

/// Admit one real OBLC Client Session against the live Server and hold it.
/// `cohort` is an already-validated cohort identity: the frozen fixture
/// cohort is verified once and lent to every admission instead of re-hashing
/// the same immutable artifacts per session.
pub(super) fn hold_session(
    fixture: &Fixture,
    cohort: &str,
    deadline: Instant,
) -> ClientSessionConnection {
    ClientSessionConnection::connect_before(fixture.site().client_socket_path(), cohort, deadline)
        .unwrap_or_else(|error| panic!("admit held session: {error}"))
}

/// The Site query surface coexists with a live controlling TUI and reports
/// real state: reads, empty and filtered results, label resolution, local
/// commands, failure envelopes, and schema-validated documents.
#[test]
fn cli_query_reads_real_state_while_a_tui_keeps_the_site() -> TestResult {
    let _process_witness = process_witness_guard();
    let fixture = Fixture::with_workspace();
    let mut client = Client::spawn(&fixture, "en_US.UTF-8");
    client.wait_ready_after(
        0,
        Instant::now() + STARTUP_OR_CREATE_TIMEOUT,
        1,
        "1",
        ReadyMode::Controlling,
    );
    let checkpoint = client.checkpoint();
    client.send(b"\x02c");
    client.wait_ordered_ready_after(
        checkpoint,
        Instant::now() + STARTUP_OR_CREATE_TIMEOUT,
        &["1", "2"],
        2,
        ReadyMode::Controlling,
    );

    let validators = validators(&fixture);

    // status: text and JSON both read the real snapshot.
    let output = run_cli(&fixture, &["status"]);
    assert_eq!(output.status.code(), Some(0));
    let text = String::from_utf8(output.stdout)?;
    assert!(text.contains("revision"), "{text}");
    let (output, document) = run_json(&fixture, &["status", "--format", "json"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(document["schema"], "orchbee.status");
    validate(&validators, "orchbee.status", &output);

    // The live TUI's tabs are visible: two initial_workspace-context tabs.
    let (output, tabs) = run_json(&fixture, &["tab", "list", "--format", "json"]);
    assert_eq!(output.status.code(), Some(0));
    validate(&validators, "orchbee.tab.list", &output);
    let items = tabs["result"]["tabs"].as_array().expect("tab list items");
    assert_eq!(items.len(), 2, "real tabs: {items:?}");

    // The explicit fixture Workspace owns both canonical Tabs.
    let (output, initial_workspace) = run_json(
        &fixture,
        &[
            "tab",
            "list",
            "--workspace",
            "Workspace",
            "--format",
            "json",
        ],
    );
    assert_eq!(output.status.code(), Some(0));
    let initial_workspace_items = initial_workspace["result"]["tabs"].as_array().unwrap();
    assert_eq!(initial_workspace_items.len(), 2);
    for row in initial_workspace_items {
        assert_eq!(row["context"]["kind"], "workspace", "{row}");
    }

    // Empty result: no projects exist on a fresh Site.
    let (output, projects) = run_json(&fixture, &["project", "list", "--format=json"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(projects["result"]["projects"].as_array().unwrap().len(), 0);
    validate(&validators, "orchbee.project.list", &output);

    // Label resolution against the real state.
    let (output, tab) = run_json(&fixture, &["tab", "get", "2", "--format=json"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(tab["result"]["tab"]["label"], "2");
    validate(&validators, "orchbee.tab.get", &output);

    // Bound terminals from the two live shells.
    let (output, terminals) = run_json(&fixture, &["terminal", "list", "--format", "json"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        terminals["result"]["terminals"].as_array().unwrap().len(),
        2
    );
    validate(&validators, "orchbee.terminal.list", &output);

    // `terminal read` observes one live grid through a short-lived read
    // viewer while the controlling TUI keeps its own attachment.
    let terminal_id = terminals["result"]["terminals"][0]["terminalId"]
        .as_str()
        .expect("bound terminal id");
    let (output, read) = run_json(
        &fixture,
        &["terminal", "read", terminal_id, "--format", "json"],
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "terminal read: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(read["result"]["terminal"]["terminalId"], terminal_id);
    assert_eq!(read["result"]["observed"]["capacityOmittedRows"], "0");
    assert!(
        !read["result"]["content"]["text"]
            .as_str()
            .expect("rows")
            .is_empty(),
        "a live terminal emits grid rows"
    );
    validate(&validators, "orchbee.terminal.output", &output);

    // `terminal get` answers the Server's own launch-read join on a live
    // binding: v2 envelope, `terminal-launch-read` source, and the retained
    // fact nested inside `terminal` — never a sibling `result.launch`.
    let (output, get) = run_json(
        &fixture,
        &["terminal", "get", terminal_id, "--format", "json"],
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "terminal get: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(get["version"], 3);
    assert_eq!(get["source"]["kind"], "terminal-launch-read");
    let get_terminal = &get["result"]["terminal"];
    assert_eq!(get_terminal["terminalId"], terminal_id);
    assert!(
        get["result"].get("launch").is_none(),
        "sibling result.launch must not exist: {get}"
    );
    let launch = &get_terminal["launch"];
    assert_eq!(launch["state"], "started", "a live binding retains started");
    assert!(
        launch["workerGeneration"].as_str().is_some(),
        "a tracked launch reports its generation: {launch}"
    );
    assert_eq!(launch["createOutcome"]["kind"], "accepted");
    validate(&validators, "orchbee.terminal.get", &output);

    // The same absent typed id is not-found on the launch read too.
    let (output, failure) = run_json(
        &fixture,
        &[
            "terminal",
            "get",
            "ffffffffffffffffffffffffffffffff",
            "--format",
            "json",
        ],
    );
    assert_eq!(output.status.code(), Some(3));
    assert_eq!(failure["failure"]["kind"], "not-found");
    validate(&validators, "orchbee.cli.failure", &output);

    // An absent typed id is not-found even while the Site is live.
    let (output, failure) = run_json(
        &fixture,
        &[
            "terminal",
            "read",
            "ffffffffffffffffffffffffffffffff",
            "--format",
            "json",
        ],
    );
    assert_eq!(output.status.code(), Some(3));
    assert_eq!(failure["failure"]["kind"], "not-found");
    validate(&validators, "orchbee.cli.failure", &output);

    // The machine surface: raw snapshot + embedded schema + config.
    let (output, snapshot) = run_json(&fixture, &["api", "snapshot", "--format", "json"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(snapshot["result"]["snapshot"]["kind"], "workbench.snapshot");
    validate(&validators, "orchbee.api.snapshot", &output);
    let (output, _) = run_json(&fixture, &["api", "schema", "orchbee.status"]);
    assert_eq!(output.status.code(), Some(0));
    validate(&validators, "orchbee.api.schema", &output);
    let (output, _) = run_json(&fixture, &["config", "check", "--format", "json"]);
    assert_eq!(output.status.code(), Some(0));
    validate(&validators, "orchbee.config.check", &output);

    // Real failure envelopes against live state.
    let (output, failure) = run_json(
        &fixture,
        &["project", "get", "no-such-project", "--format", "json"],
    );
    assert_eq!(output.status.code(), Some(3));
    assert_eq!(failure["failure"]["kind"], "not-found");
    validate(&validators, "orchbee.cli.failure", &output);

    // A text failure keeps stdout empty on the real path too.
    let output = run_cli(&fixture, &["project", "get", "no-such-project"]);
    assert_eq!(output.status.code(), Some(3));
    assert!(output.stdout.is_empty());

    // Fresh post-query observation: a tab creation issued only after all the
    // query traffic must land in a newly observed ordered frame — never a
    // stale frame collected before the queries ran.
    let checkpoint = client.checkpoint();
    client.send(b"\x02c");
    client.wait_ordered_ready_after(
        checkpoint,
        Instant::now() + STARTUP_OR_CREATE_TIMEOUT,
        &["1", "2", "3"],
        3,
        ReadyMode::Controlling,
    );
    client.close_before(Instant::now() + EXIT_TIMEOUT);
    Ok(())
}

/// Thirty-two real admitted Client Sessions saturate the Server; the
/// thirty-third query is refused with `session-capacity`, and a released slot
/// is usable again without assuming synchronous EOF teardown.
#[test]
fn cli_query_session_capacity_and_release_on_a_real_server() -> TestResult {
    let _process_witness = process_witness_guard();
    let fixture = Fixture::new();
    let _server = spawn_real_server(&fixture);
    let admission_deadline = Instant::now() + Duration::from_secs(40);
    let cohort = NativeCohort::load(&fixture.cohort, admission_deadline)
        .unwrap_or_else(|error| panic!("cohort identity: {error}"));
    let mut held = Vec::new();
    for index in 0..32 {
        held.push(hold_session(
            &fixture,
            cohort.identity(),
            admission_deadline,
        ));
        eprintln!("held session {index}");
    }

    // The thirty-third session request — a real query — is refused at admission.
    let (output, failure) = run_json(&fixture, &["status", "--format", "json"]);
    assert_eq!(output.status.code(), Some(4));
    assert_eq!(failure["failure"]["kind"], "session-capacity");
    assert_eq!(failure["failure"]["details"]["stage"], "admission");

    // Release one slot: the Server notices the disconnect on its own schedule
    // (not a synchronous EOF guarantee), so availability is polled within a
    // bounded window rather than assumed immediate.
    drop(held.pop());
    let release_deadline = Instant::now() + SLOT_RELEASE_DEADLINE;
    let mut code = run_cli(&fixture, &["status", "--format", "json"])
        .status
        .code();
    while code != Some(0) && Instant::now() < release_deadline {
        thread::sleep(Duration::from_millis(150));
        code = run_cli(&fixture, &["status", "--format", "json"])
            .status
            .code();
    }
    assert_eq!(code, Some(0), "released slot must admit a new session");
    // The query path itself leaks nothing: back-to-back invocations each get
    // a fresh transient session and succeed.
    for _ in 0..2 {
        let output = run_cli(&fixture, &["status", "--format", "json"]);
        assert_eq!(output.status.code(), Some(0), "query sessions do not leak");
    }

    drop(held);
    Ok(())
}

/// A blocked stdout does not keep the transport alive: while the CLI is
/// parked mid-write against a full pipe, its /proc descriptor set holds no
/// socket — the session was dropped before output — and the Server admits a
/// fresh query afterwards.
#[test]
fn blocked_stdout_keeps_no_transport_and_releases_the_real_server() -> TestResult {
    let _process_witness = process_witness_guard();
    let fixture = Fixture::with_workspace();
    // Real content: a TUI with eight tabs makes the machine snapshot exceed a
    // minimal pipe, so the emit boundary genuinely blocks.
    let mut client = Client::spawn(&fixture, "en_US.UTF-8");
    client.wait_ready_after(
        0,
        Instant::now() + STARTUP_OR_CREATE_TIMEOUT,
        1,
        "1",
        ReadyMode::Controlling,
    );
    for index in 1..8 {
        let checkpoint = client.checkpoint();
        client.send(b"\x02c");
        let labels: Vec<String> = (1..=index + 1).map(|value| value.to_string()).collect();
        let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
        client.wait_ordered_ready_after(
            checkpoint,
            Instant::now() + STARTUP_OR_CREATE_TIMEOUT,
            &labels,
            index + 1,
            ReadyMode::Controlling,
        );
    }

    use nix::fcntl::{FcntlArg, fcntl};
    let (read_end, write_end) = nix::unistd::pipe()?;
    // Minimum legal pipe capacity keeps the witness small and deterministic.
    fcntl(&read_end, FcntlArg::F_SETPIPE_SZ(4096))?;
    let mut child = Reap::spawn(
        Command::new(fixture.cohort.join("orchbee"))
            .args(["api", "snapshot", "--format", "json"])
            .env("ORCHBEE_RUNTIME_BASE", &fixture.base)
            .env("LANG", "en_US.UTF-8")
            .stdin(Stdio::null())
            .stdout(Stdio::from(fs::File::from(write_end)))
            .stderr(Stdio::piped()),
    )?;
    let child_id = child.id() as i32;

    // Wait until the child has produced bytes and is still alive: with the
    // document fully prepared before the first write, alive + pipe-full means
    // parked inside the emit boundary.
    let deadline = Instant::now() + BLOCKED_STDOUT_DEADLINE;
    loop {
        assert!(
            child.try_wait()?.is_none(),
            "CLI exited before backpressure"
        );
        let mut descriptors = [PollFd::new(read_end.as_fd(), PollFlags::POLLIN)];
        if poll(
            &mut descriptors,
            PollTimeout::try_from(Duration::from_millis(200)).unwrap(),
        )
        .is_ok_and(|count| count > 0)
        {
            // Bytes reached the pipe while the process stays alive: with the
            // document fully prepared before the first write, the remaining
            // work can only be the blocked write itself.
            thread::sleep(Duration::from_millis(400));
            assert!(child.try_wait()?.is_none(), "CLI must block on stdout");
            break;
        }
        assert!(Instant::now() < deadline, "CLI never reached blocked emit");
    }

    // /proc witness: while stdout blocks, the descriptor table holds no
    // socket — transport owners were dropped before projection/output.
    let fd_dir = PathBuf::from(format!("/proc/{child_id}/fd"));
    let mut socket_fds = 0;
    for entry in fs::read_dir(&fd_dir)? {
        let entry = entry?;
        if let Ok(target) = fs::read_link(entry.path())
            && target.to_string_lossy().contains("socket:")
        {
            socket_fds += 1;
        }
    }
    assert_eq!(socket_fds, 0, "blocked CLI still owns transport sockets");

    // Drain the pipe; the prepared document completes and the process exits 0.
    let mut reader = fs::File::from(read_end);
    let mut stdout = Vec::new();
    io::Read::read_to_end(&mut reader, &mut stdout)?;
    let status = child.wait()?;
    assert_eq!(status.code(), Some(0));
    let document: serde_json::Value = serde_json::from_slice(&stdout)?;
    assert_eq!(document["result"]["snapshot"]["kind"], "workbench.snapshot");

    // The real Server released the transient session: a fresh query admits.
    let output = run_cli(&fixture, &["status", "--format", "json"]);
    assert_eq!(output.status.code(), Some(0), "server slot released");

    client.close_before(Instant::now() + EXIT_TIMEOUT);
    Ok(())
}
