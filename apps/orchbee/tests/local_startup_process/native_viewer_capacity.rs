use super::{
    native_cli_query::{run_json, validate, validators},
    native_context::WorkbenchPeer,
    *,
};
use orchbee_workbench_protocol::{
    PanelNode, WorkbenchAction, WorkbenchCommandOutcome, WorkbenchLabel, WorkbenchPanelId,
    WorkbenchPanelSplitDirection, WorkbenchTerminalGeometry,
};
use std::collections::BTreeSet;

// Two TUI Clients each rendering the same four-Pane Tab hold eight viewer
// attachments — well below the shared Worker's authorized table — so a ninth
// read-only attach through the public read path must succeed while the live
// eight stay undamaged. Exact-capacity refusal and release belong to
// c1_capacity_process_witness, which owns the authorized-table boundary.
// Four-way splits truncate pane titles at 120 columns, so titles never serve
// as pane proof here — only shell-written identity files and printed markers
// inside each pane's own viewer stream count.
const SPLIT_READY: [&str; 3] = [
    "Panel · Terminal 1 · 1",
    "Panel · Terminal 2",
    "Tab 1 · Ready · controlling",
];
const SETTLE_GRACE: Duration = Duration::from_millis(75);
const DETACH_POLL: Duration = Duration::from_millis(100);
const PREPARATION_TIMEOUT: Duration = Duration::from_secs(45);

fn rightmost_panel(node: &PanelNode) -> Option<WorkbenchPanelId> {
    match node {
        PanelNode::Panel { panel_id, .. } => Some(panel_id.clone()),
        PanelNode::Split { children, .. } => rightmost_panel(&children.last()?.node),
    }
}

fn tab_root(fixture_snapshot: &orchbee_workbench_protocol::Snapshot) -> Option<PanelNode> {
    let context_id = fixture_snapshot
        .navigation
        .standalone_terminal_context_id
        .clone();
    fixture_snapshot
        .terminal_contexts
        .iter()
        .find(|context| context.terminal_context_id == context_id)
        .and_then(|context| context.tabs.first())
        .map(|tab| tab.root.clone())
}

fn move_focus(client: &mut Client, direction: &[u8], peers: &mut [&mut Client], deadline: Instant) {
    let checkpoint = client.checkpoint();
    client.send(direction);
    client.wait_frame_text_after(
        checkpoint,
        deadline,
        &["Tab 1 · Ready · controlling"],
        &["Workbench navigator"],
        &[],
        peers,
    );
    thread::sleep(SETTLE_GRACE);
}

/// One real shell command lands in the focused Pane: it records the shell PID
/// in an identity file and prints a split-token marker that only the Pane's
/// live viewer attachment can render back into this Client's outer frame.
fn send_pane_witness(
    fixture: &Fixture,
    client: &mut Client,
    filename: &str,
    role: &str,
    marker: &str,
    assign: bool,
) -> usize {
    let command = format!(
        "{}; printf '%s%s\\n' '{marker}' 'X'\r",
        shell_identity_command(fixture, role, filename, assign).trim_end_matches('\r')
    );
    let checkpoint = client.checkpoint();
    client.send(command.as_bytes());
    checkpoint
}

#[test]
fn two_clients_four_panes_leave_cli_read_capacity_then_release_admits_read() -> TestResult {
    let _process_witness = process_witness_guard();
    let fixture = Fixture::new();
    let mut primary = Client::spawn(&fixture, "en_US.UTF-8");
    primary.wait_ready_after(
        0,
        Instant::now() + STARTUP_OR_CREATE_TIMEOUT,
        1,
        "1",
        ReadyMode::Controlling,
    );

    // Three public Workbench splits grow the standalone Tab to four real panes.
    let split_deadline = Instant::now() + PREPARATION_TIMEOUT;
    let mut peer = WorkbenchPeer::connect(&fixture, split_deadline)?;
    let context_id = peer
        .snapshot(split_deadline)?
        .navigation
        .standalone_terminal_context_id
        .clone();
    for label in ["cap-b", "cap-c", "cap-d"] {
        let snapshot = peer.snapshot(split_deadline)?;
        let root = tab_root(&snapshot).ok_or("split Tab missing")?;
        let panel_id = rightmost_panel(&root).ok_or("split leaf missing")?;
        let tab_id = snapshot
            .terminal_contexts
            .iter()
            .find(|context| context.terminal_context_id == context_id)
            .and_then(|context| context.tabs.first())
            .ok_or("split Tab missing")?
            .tab_id
            .clone();
        match peer.prepare(
            WorkbenchAction::PanelSplit {
                terminal_context_id: context_id.clone(),
                tab_id,
                panel_id,
                direction: WorkbenchPanelSplitDirection::Right,
                label: WorkbenchLabel::parse(label)?,
                geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16)?,
                launch: None,
                profile: None,
                options: Default::default(),
            },
            split_deadline,
        )? {
            WorkbenchCommandOutcome::PanelSplitCreated { .. } => {}
            outcome => {
                return Err(format!("PanelSplit for {label} did not start: {outcome:?}").into());
            }
        }
    }

    // The navigator refresh is the public path that re-renders the pane layout.
    let refresh_checkpoint = primary.checkpoint();
    primary.send(b"\x02w");
    primary.wait_frame_text_after(
        refresh_checkpoint,
        Instant::now() + SWITCH_TIMEOUT,
        &["Workbench navigator", "Tabs refreshed · revision "],
        &[],
        &[],
        &mut [],
    );
    let closed_checkpoint = primary.checkpoint();
    primary.send(b"\x02w");
    primary.wait_frame_text_after(
        closed_checkpoint,
        Instant::now() + SWITCH_TIMEOUT,
        &SPLIT_READY,
        &["Workbench navigator"],
        &[],
        &mut [],
    );

    // Focus walks left-to-right; every pane proves a live attachment by
    // executing a real shell write and rendering its own marker.
    let mut pids = Vec::new();
    for index in 0..4 {
        if index > 0 {
            move_focus(
                &mut primary,
                b"\x02l",
                &mut [],
                Instant::now() + SWITCH_TIMEOUT,
            );
        }
        let filename = format!("cap-pane-{index}");
        let checkpoint = send_pane_witness(
            &fixture,
            &mut primary,
            &filename,
            &format!("cap{index}"),
            &format!("CAP_{index}"),
            true,
        );
        let deadline = Instant::now() + OBSERVATION_TIMEOUT;
        let identity = wait_file(&fixture.base.join(&filename), &mut [&mut primary], deadline);
        pids.push(require_shell_identity(&identity, &format!("cap{index}")));
        primary.wait_frame_text_after(
            checkpoint,
            deadline,
            &[&format!("CAP_{index}X")],
            &[],
            &[],
            &mut [],
        );
    }
    assert_eq!(
        pids.iter().collect::<BTreeSet<_>>().len(),
        4,
        "four panes must own four distinct shells"
    );

    // The second TUI joins view-only; its own four pane attachments bring the
    // live count to eight, proven by the same per-pane markers.
    let mut viewer = Client::spawn(&fixture, "en_US.UTF-8");
    viewer.wait_ready_after(
        0,
        Instant::now() + STARTUP_OR_CREATE_TIMEOUT,
        1,
        "1",
        ReadyMode::ViewOnly,
    );
    let attached_evidence = [
        "CAP_0X",
        "CAP_1X",
        "CAP_2X",
        "CAP_3X",
        "Tab 1 · Ready · view only",
    ];
    viewer.wait_frame_text_after(
        0,
        Instant::now() + OBSERVATION_TIMEOUT,
        &attached_evidence,
        &[],
        &[],
        &mut [&mut primary],
    );

    // The ninth attach through the public read path still fits the authorized
    // table: the read must succeed with the normal document shape.
    let (output, terminals) = run_json(&fixture, &["terminal", "list", "--format", "json"]);
    assert_eq!(output.status.code(), Some(0), "terminal list must succeed");
    let terminal_id = terminals["result"]["terminals"]
        .as_array()
        .filter(|terminals| terminals.len() == 4)
        .and_then(|terminals| terminals.first())
        .and_then(|terminal| terminal["terminalId"].as_str())
        .expect("four bound terminals listed")
        .to_owned();
    let (ninth, read) = run_json(
        &fixture,
        &["terminal", "read", &terminal_id, "--format", "json"],
    );
    assert_eq!(
        ninth.status.code(),
        Some(0),
        "ninth viewer attach must succeed: {}",
        String::from_utf8_lossy(&ninth.stderr)
    );
    assert_eq!(read["result"]["terminal"]["terminalId"], terminal_id);
    assert_eq!(read["result"]["content"]["truncated"], false);
    assert!(
        !read["result"]["content"]["rows"]
            .as_array()
            .expect("read rows")
            .is_empty(),
        "a live terminal emits grid rows"
    );
    validate(&validators(&fixture), "orchbee.terminal.read", &ninth);

    // The admitted read damages nothing: fresh output in each of the eight
    // views and the first Client's input still reaches each distinct shell.
    let mut post_pids = Vec::new();
    for index in (0..4).rev() {
        if index < 3 {
            move_focus(
                &mut primary,
                b"\x02h",
                &mut [&mut viewer],
                Instant::now() + SWITCH_TIMEOUT,
            );
        }
        let filename = format!("cap-post-{index}");
        let checkpoint = send_pane_witness(
            &fixture,
            &mut primary,
            &filename,
            &format!("post{index}"),
            &format!("POST_{index}"),
            true,
        );
        let deadline = Instant::now() + OBSERVATION_TIMEOUT;
        let identity = wait_file(
            &fixture.base.join(&filename),
            &mut [&mut primary, &mut viewer],
            deadline,
        );
        post_pids.push(require_shell_identity(&identity, &format!("post{index}")));
        primary.wait_frame_text_after(
            checkpoint,
            deadline,
            &[&format!("POST_{index}X"), "Tab 1 · Ready · controlling"],
            &[],
            &[],
            &mut [&mut viewer],
        );
    }
    post_pids.reverse();
    assert_eq!(
        pids, post_pids,
        "post-read input must reach the same four shells in pane order"
    );
    viewer.wait_frame_text_after(
        0,
        Instant::now() + OBSERVATION_TIMEOUT,
        &["POST_0X", "POST_1X", "POST_2X", "POST_3X"],
        &[],
        &[],
        &mut [&mut primary],
    );

    // Releasing the second Client frees its four attachments; the identical
    // read still succeeds while the original shells keep their identity.
    viewer.close_before(Instant::now() + EXIT_TIMEOUT);
    let read_deadline = Instant::now() + SWITCH_TIMEOUT;
    let read = loop {
        primary.collect();
        let (output, document) = run_json(
            &fixture,
            &["terminal", "read", &terminal_id, "--format", "json"],
        );
        if output.status.code() == Some(0) {
            break document;
        }
        assert_eq!(
            document["failure"]["kind"], "service-unavailable",
            "capacity is the only admissible transient failure"
        );
        assert!(
            Instant::now() < read_deadline,
            "released viewer slots never freed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        thread::sleep(DETACH_POLL);
    };
    assert_eq!(read["result"]["terminal"]["terminalId"], terminal_id);
    assert!(
        !read["result"]["content"]["rows"]
            .as_array()
            .expect("read rows")
            .is_empty(),
        "a live terminal emits grid rows"
    );

    let checkpoint =
        send_pane_witness(&fixture, &mut primary, "cap-final", "final", "FINAL_", true);
    let deadline = Instant::now() + OBSERVATION_TIMEOUT;
    let final_identity = wait_file(
        &fixture.base.join("cap-final"),
        &mut [&mut primary],
        deadline,
    );
    let final_pid = require_shell_identity(&final_identity, "final");
    assert_eq!(
        final_pid, pids[0],
        "the focused shell identity survives the release and read"
    );
    primary.wait_frame_text_after(checkpoint, deadline, &["FINAL_X"], &[], &[], &mut []);
    primary.close_before(Instant::now() + EXIT_TIMEOUT);
    Ok(())
}
