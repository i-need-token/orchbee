use super::pty_client::FrameExpectation;
use super::tui_observer::WorkbenchHeader;
use super::workbench_peer::WorkbenchPeer;
use super::*;
use nix::poll::{PollFd, PollFlags, PollTimeout, poll};
use orchbee_workbench_protocol::{
    PanelNode, WorkbenchAction, WorkbenchCommandOutcome, WorkbenchDirectoryPath, WorkbenchLabel,
    WorkbenchPanelSplitDirection, WorkbenchPublicationOutcome, WorkbenchTerminalGeometry,
};
use rustix::process::{Pid, PidfdFlags, pidfd_open};
use std::os::fd::AsFd;

const EMPTY: &str = "No terminal tabs";
const CONFIGURED_EMPTY_ACTION: &str = "Ctrl+b c New Tab";
const UNCONFIGURED_EMPTY_FACT: &str = "No directory configured.";
const UNCONFIGURED_EMPTY_ACTION: &str = "Ctrl+b e Set Workspace directory";
const INITIAL_WORKSPACE_LOCATION: &str = "Workspace";
const CONFIGURED_LOCATION: &str = "Context process / Configured";
const UNCONFIGURED_LOCATION: &str = "Context process / Unconfigured";
const CLOSE_TIMEOUT: Duration = Duration::from_secs(45);

#[derive(Clone, Copy)]
enum ExpectedContext<'a> {
    Ready,
    ConfiguredEmpty(&'a str),
    UnconfiguredEmpty,
}

fn select_context(
    client: &mut Client,
    command: &[u8],
    location: &str,
    expected: ExpectedContext<'_>,
) {
    let checkpoint = client.checkpoint();
    client.send(command);
    // The sidebar always says "Workspace". Only the active header distinguishes
    // a completed context switch from a fresh draw of the previous Ready Workspace.
    let directory_fact = match expected {
        ExpectedContext::ConfiguredEmpty(path) => Some(format!("Directory: {path}")),
        _ => None,
    };
    let (header, required) = match expected {
        ExpectedContext::Ready => (
            WorkbenchHeader::active(location, 1, "1"),
            vec!["Tab 1 · Ready · controlling"],
        ),
        ExpectedContext::ConfiguredEmpty(_) => (
            WorkbenchHeader::empty(location),
            vec![
                EMPTY,
                directory_fact
                    .as_deref()
                    .expect("configured directory fact"),
                CONFIGURED_EMPTY_ACTION,
            ],
        ),
        ExpectedContext::UnconfiguredEmpty => (
            WorkbenchHeader::empty(location),
            vec![EMPTY, UNCONFIGURED_EMPTY_FACT, UNCONFIGURED_EMPTY_ACTION],
        ),
    };
    let forbidden = if matches!(expected, ExpectedContext::Ready) {
        vec!["Workbench navigator", EMPTY]
    } else {
        vec!["Workbench navigator"]
    };
    client.wait_native_frame_text_after(
        checkpoint,
        Instant::now() + SWITCH_TIMEOUT,
        FrameExpectation::native(header, &required, &forbidden, &[]),
        &mut [],
    );
}

fn observe_independent_initial_workspace(client: &mut Client, revision: u64, active: &mut Client) {
    let deadline = Instant::now() + SWITCH_TIMEOUT;
    let checkpoint = client.checkpoint();
    client.send(b"\x02w");
    let refreshed = format!("Tabs refreshed · revision {revision}");
    client.wait_native_frame_text_after(
        checkpoint,
        deadline,
        FrameExpectation::native(
            WorkbenchHeader::active(INITIAL_WORKSPACE_LOCATION, 1, "1"),
            &["Workbench navigator", &refreshed, "  * 1 Workspace"],
            &[],
            &[],
        ),
        &mut [active],
    );
    let checkpoint = client.checkpoint();
    client.send(b"\x02w");
    client.wait_native_frame_text_after(
        checkpoint,
        deadline,
        FrameExpectation::native(
            WorkbenchHeader::active(INITIAL_WORKSPACE_LOCATION, 1, "1"),
            &["Tab 1 · Ready ·", "INITIAL_WORKSPACE_PEER_BINDING"],
            &[
                "Workbench navigator",
                CONFIGURED_LOCATION,
                UNCONFIGURED_LOCATION,
                EMPTY,
            ],
            &[],
        ),
        &mut [active],
    );
}

#[test]
fn native_initial_workspace_panel_split_refreshes_and_moves_focus_between_live_leaves() -> TestResult
{
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

    client.send(shell_identity_command(&fixture, "source", "native-split-source", true).as_bytes());
    let source_identity = wait_file(
        &fixture.base.join("native-split-source"),
        &mut [&mut client],
        Instant::now() + OBSERVATION_TIMEOUT,
    );
    let source_pid = require_shell_identity(&source_identity, "source");

    let split_deadline = Instant::now() + CLOSE_TIMEOUT;
    let mut peer = WorkbenchPeer::connect(&fixture, split_deadline)?;
    let before_split = peer.snapshot(split_deadline)?;
    let terminal_context_id = before_split.navigation.workspaces[0]
        .terminal_context_id
        .clone();
    let context = before_split
        .terminal_contexts
        .iter()
        .find(|candidate| candidate.terminal_context_id == terminal_context_id)
        .ok_or("Split focus context missing")?;
    let tab = context.tabs.first().ok_or("Split focus Tab missing")?;
    let panel_id = match &tab.root {
        PanelNode::Panel { panel_id, .. } => panel_id.clone(),
        PanelNode::Split { .. } => return Err("fresh native split Tab was already split".into()),
    };
    let tab_id = tab.tab_id.clone();
    let split_geometry = WorkbenchTerminalGeometry::new(80, 24, 8, 16)?;
    let new_panel_id = match peer.prepare(
        WorkbenchAction::PanelSplit {
            right_click: Default::default(),
            terminal_context_id: terminal_context_id.clone(),
            tab_id,
            panel_id,
            direction: WorkbenchPanelSplitDirection::Right,
            label: WorkbenchLabel::parse("Split peer")?,
            geometry: split_geometry,
            launch: None,
            profile: None,
            options: Default::default(),
        },
        split_deadline,
    )? {
        WorkbenchCommandOutcome::PanelSplitCreated { new_panel_id, .. } => new_panel_id,
        outcome => return Err(format!("native PanelSplit did not start: {outcome:?}").into()),
    };
    let after_split = peer.snapshot(split_deadline)?;
    let split_tab = after_split
        .terminal_contexts
        .iter()
        .find(|candidate| candidate.terminal_context_id == terminal_context_id)
        .and_then(|candidate| candidate.tabs.first())
        .ok_or("native split Tab was not projected")?;
    let PanelNode::Split { children, .. } = &split_tab.root else {
        return Err("native PanelSplit did not project a split root".into());
    };
    assert!(children.iter().any(|slot| {
        matches!(&slot.node, PanelNode::Panel { panel_id, .. } if panel_id == &new_panel_id)
    }));

    let refresh_revision = after_split.revision.value();
    let checkpoint = client.checkpoint();
    client.send(b"\x02w");
    client.wait_frame_text_after(
        checkpoint,
        Instant::now() + SWITCH_TIMEOUT,
        &[
            "Workbench navigator",
            &format!("Tabs refreshed · revision {refresh_revision}"),
        ],
        &[],
        &[],
        &mut [],
    );
    let checkpoint = client.checkpoint();
    client.send(b"\x02w");
    client.wait_frame_text_after(
        checkpoint,
        Instant::now() + SWITCH_TIMEOUT,
        &[
            "Terminal 1 · 1",
            "Terminal 2 · Split peer",
            "Tab 1 · Ready · controlling",
        ],
        &["Workbench navigator"],
        &[],
        &mut [],
    );

    // The projected title and global Ready do not prove the split peer has
    // converged: the focus chord is only a valid single send once the leaf's
    // own live prompt renders inside its own region — the settled-mode
    // witness that admits the focus move.
    native_multiview::wait_leaf_live(&mut client, "Split peer", Instant::now() + SWITCH_TIMEOUT);
    let checkpoint = client.checkpoint();
    client.send(b"\x02l");
    client.wait_frame_text_after(
        checkpoint,
        Instant::now() + SWITCH_TIMEOUT,
        &[
            "Terminal 1 · 1",
            "Terminal 2 · Split peer",
            "Tab 1 · Ready · controlling",
        ],
        &["Workbench navigator"],
        &[],
        &mut [],
    );
    thread::sleep(Duration::from_millis(75));
    client.send(shell_identity_command(&fixture, "split", "native-split-right", true).as_bytes());
    let split_identity = wait_file(
        &fixture.base.join("native-split-right"),
        &mut [&mut client],
        Instant::now() + OBSERVATION_TIMEOUT,
    );
    let split_pid = require_shell_identity(&split_identity, "split");
    assert_ne!(
        source_pid, split_pid,
        "right Panel must attach the new shell"
    );

    let checkpoint = client.checkpoint();
    client.send(b"\x02h");
    client.wait_frame_text_after(
        checkpoint,
        Instant::now() + SWITCH_TIMEOUT,
        &[
            "Terminal 1 · 1",
            "Terminal 2 · Split peer",
            "Tab 1 · Ready · controlling",
        ],
        &["Workbench navigator"],
        &[],
        &mut [],
    );
    thread::sleep(Duration::from_millis(75));
    client.send(shell_identity_command(&fixture, "source", "native-split-left", false).as_bytes());
    assert_eq!(
        wait_file(
            &fixture.base.join("native-split-left"),
            &mut [&mut client],
            Instant::now() + OBSERVATION_TIMEOUT,
        ),
        source_identity,
        "left Panel must return to the original shell",
    );

    let checkpoint = client.checkpoint();
    client.send(b"\x02x");
    client.wait_close_prompt_after(
        checkpoint,
        Instant::now() + OBSERVATION_TIMEOUT,
        INITIAL_WORKSPACE_LOCATION,
        // The split Tab holds both live shells.
        &[
            "Close Tab?",
            "",
            "1",
            "",
            "Ends 2 terminals.",
            "",
            "[ Cancel  Esc ] [ Close  Enter ]",
        ],
        &mut [],
    );
    let checkpoint = client.checkpoint();
    client.send(b"\r");
    client.wait_native_frame_text_after(
        checkpoint,
        Instant::now() + CLOSE_TIMEOUT,
        FrameExpectation::native(
            WorkbenchHeader::empty(INITIAL_WORKSPACE_LOCATION),
            &["No terminal tabs", "Ctrl+b c create Tab"],
            &["Close Tab?", "Workbench navigator"],
            &[],
        ),
        &mut [],
    );
    let closed = peer.snapshot(Instant::now() + OBSERVATION_TIMEOUT)?;
    let closed_context = closed
        .terminal_contexts
        .iter()
        .find(|candidate| candidate.terminal_context_id == terminal_context_id)
        .ok_or("Split focus context missing after Close")?;
    assert!(closed_context.tabs.is_empty());
    client.close_before(Instant::now() + EXIT_TIMEOUT);
    Ok(())
}

#[test]
fn native_initial_workspace_panel_close_collapses_the_split_and_releases_only_that_shell()
-> TestResult {
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

    client.send(shell_identity_command(&fixture, "source", "native-close-source", true).as_bytes());
    let source_identity = wait_file(
        &fixture.base.join("native-close-source"),
        &mut [&mut client],
        Instant::now() + OBSERVATION_TIMEOUT,
    );
    let source_pid = require_shell_identity(&source_identity, "source");
    let source_shell = pidfd_open(
        Pid::from_raw(i32::try_from(source_pid)?).ok_or("invalid source shell PID")?,
        PidfdFlags::empty(),
    )?;

    let split_deadline = Instant::now() + CLOSE_TIMEOUT;
    let mut peer = WorkbenchPeer::connect(&fixture, split_deadline)?;
    let before_split = peer.snapshot(split_deadline)?;
    let terminal_context_id = before_split.navigation.workspaces[0]
        .terminal_context_id
        .clone();
    let context = before_split
        .terminal_contexts
        .iter()
        .find(|candidate| candidate.terminal_context_id == terminal_context_id)
        .ok_or("Panel close context missing")?;
    let tab = context.tabs.first().ok_or("Panel close Tab missing")?;
    let source_panel_id = match &tab.root {
        PanelNode::Panel { panel_id, .. } => panel_id.clone(),
        PanelNode::Split { .. } => return Err("fresh native close Tab was already split".into()),
    };
    let tab_id = tab.tab_id.clone();
    let (split_revision, new_panel_id) = match peer.prepare(
        WorkbenchAction::PanelSplit {
            right_click: Default::default(),
            terminal_context_id: terminal_context_id.clone(),
            tab_id: tab_id.clone(),
            panel_id: source_panel_id.clone(),
            direction: WorkbenchPanelSplitDirection::Right,
            label: WorkbenchLabel::parse("Split peer")?,
            geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16)?,
            launch: None,
            profile: None,
            options: Default::default(),
        },
        split_deadline,
    )? {
        WorkbenchCommandOutcome::PanelSplitCreated {
            revision,
            new_panel_id,
            ..
        } => (revision, new_panel_id),
        outcome => return Err(format!("native PanelSplit did not start: {outcome:?}").into()),
    };

    // Rendezvous on the committed split; the Client keeps focus on the surviving left leaf.
    let checkpoint = client.checkpoint();
    client.send(b"\x02w");
    client.wait_frame_text_after(
        checkpoint,
        Instant::now() + SWITCH_TIMEOUT,
        &[
            "Workbench navigator",
            &format!("Tabs refreshed · revision {}", split_revision.value()),
        ],
        &[],
        &[],
        &mut [],
    );
    let checkpoint = client.checkpoint();
    client.send(b"\x02w");
    client.wait_frame_text_after(
        checkpoint,
        Instant::now() + SWITCH_TIMEOUT,
        &[
            "Terminal 1 · 1",
            "Terminal 2 · Split peer",
            "Tab 1 · Ready · controlling",
        ],
        &["Workbench navigator"],
        &[],
        &mut [],
    );
    // The Client's attached leaf stays live across the sibling Close; prove focus and the
    // exact shell with a fresh identity write.
    client.send(shell_identity_command(&fixture, "source", "native-close-left", false).as_bytes());
    assert_eq!(
        wait_file(
            &fixture.base.join("native-close-left"),
            &mut [&mut client],
            Instant::now() + OBSERVATION_TIMEOUT,
        ),
        source_identity,
        "focus must remain on the original left shell after the split",
    );

    // Close the unfocused leaf through the shared lifecycle; the sibling must survive.
    let close_deadline = Instant::now() + CLOSE_TIMEOUT;
    match peer.prepare(
        WorkbenchAction::PanelClose {
            terminal_context_id: terminal_context_id.clone(),
            tab_id: tab_id.clone(),
            panel_id: new_panel_id.clone(),
        },
        close_deadline,
    )? {
        WorkbenchCommandOutcome::PanelCloseAccepted {
            tab_id: accepted_tab,
            panel_id: accepted_panel,
            ..
        } => {
            assert_eq!(accepted_tab, tab_id);
            assert_eq!(accepted_panel, new_panel_id);
        }
        outcome => return Err(format!("native PanelClose was not accepted: {outcome:?}").into()),
    }
    let mut surviving_panel = None;
    while Instant::now() < close_deadline {
        let snapshot = peer.snapshot(close_deadline)?;
        let tab = snapshot
            .terminal_contexts
            .iter()
            .find(|candidate| candidate.terminal_context_id == terminal_context_id)
            .and_then(|candidate| candidate.tabs.first())
            .ok_or("Panel close Tab missing after acceptance")?;
        if let PanelNode::Panel { panel_id, .. } = &tab.root {
            surviving_panel = Some(panel_id.clone());
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }
    let surviving_panel = surviving_panel.ok_or("split root did not collapse to a leaf")?;
    assert_eq!(surviving_panel, source_panel_id);
    // Publication is only reachable after the leaf's lifecycle completed, so the collapsed
    // root proves the closed leaf's terminal was disposed. The surviving leaf's shell lives.
    let mut descriptor = [PollFd::new(source_shell.as_fd(), PollFlags::POLLIN)];
    poll(&mut descriptor, PollTimeout::ZERO)?;
    assert!(
        !descriptor[0]
            .revents()
            .is_some_and(|flags| flags.contains(PollFlags::POLLIN)),
        "surviving leaf shell exited"
    );
    // The TUI refreshes itself on the published Close without a manual poll: a single-leaf
    // Tab renders the terminal directly with no per-Panel title.
    let checkpoint = client.checkpoint();
    client.wait_native_frame_text_after(
        checkpoint,
        Instant::now() + SWITCH_TIMEOUT,
        FrameExpectation::native(
            WorkbenchHeader::active(INITIAL_WORKSPACE_LOCATION, 1, "1"),
            &["Tab 1 · Ready"],
            &["Terminal 2 · Split peer", "Terminal disconnected"],
            &[],
        ),
        &mut [],
    );
    client.send(
        shell_identity_command(&fixture, "source", "native-close-surviving", false).as_bytes(),
    );
    assert_eq!(
        wait_file(
            &fixture.base.join("native-close-surviving"),
            &mut [&mut client],
            Instant::now() + OBSERVATION_TIMEOUT,
        ),
        source_identity,
        "surviving Panel must keep the original shell",
    );

    // Closing the only remaining leaf removes the whole Tab; the attached Client loses its
    // terminal and enters the disconnected state, matching the established Close semantic.
    let sole_deadline = Instant::now() + CLOSE_TIMEOUT;
    assert!(matches!(
        peer.prepare(
            WorkbenchAction::PanelClose {
                terminal_context_id: terminal_context_id.clone(),
                tab_id: tab_id.clone(),
                panel_id: surviving_panel,
            },
            sole_deadline,
        )?,
        WorkbenchCommandOutcome::PanelCloseAccepted { .. }
    ));
    let mut emptied = false;
    while Instant::now() < sole_deadline && !emptied {
        let snapshot = peer.snapshot(sole_deadline)?;
        emptied = snapshot
            .terminal_contexts
            .iter()
            .find(|candidate| candidate.terminal_context_id == terminal_context_id)
            .is_some_and(|context| context.tabs.is_empty());
        thread::sleep(Duration::from_millis(50));
    }
    assert!(emptied, "sole leaf Close did not remove the Tab");
    let checkpoint = client.checkpoint();
    client.wait_frame_text_after(
        checkpoint,
        Instant::now() + SWITCH_TIMEOUT,
        &["Terminal disconnected", "Ctrl+b q / Ctrl+b d exit"],
        &[],
        &[],
        &mut [],
    );
    let mut disposed = false;
    while Instant::now() < sole_deadline && !disposed {
        let mut descriptor = [PollFd::new(source_shell.as_fd(), PollFlags::POLLIN)];
        poll(&mut descriptor, PollTimeout::from(100u16))?;
        disposed = descriptor[0]
            .revents()
            .is_some_and(|flags| flags.contains(PollFlags::POLLIN));
        client.collect();
    }
    assert!(disposed, "sole leaf shell did not exit");
    client.close_before(Instant::now() + EXIT_TIMEOUT);
    Ok(())
}

#[test]
fn native_initial_workspace_panel_move_relocates_a_leaf_without_touching_either_shell() -> TestResult
{
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

    client.send(shell_identity_command(&fixture, "source", "native-move-source", true).as_bytes());
    let source_identity = wait_file(
        &fixture.base.join("native-move-source"),
        &mut [&mut client],
        Instant::now() + OBSERVATION_TIMEOUT,
    );
    let source_pid = require_shell_identity(&source_identity, "source");
    let source_shell = pidfd_open(
        Pid::from_raw(i32::try_from(source_pid)?).ok_or("invalid source shell PID")?,
        PidfdFlags::empty(),
    )?;

    let split_deadline = Instant::now() + CLOSE_TIMEOUT;
    let mut peer = WorkbenchPeer::connect(&fixture, split_deadline)?;
    let before_split = peer.snapshot(split_deadline)?;
    let terminal_context_id = before_split.navigation.workspaces[0]
        .terminal_context_id
        .clone();
    let context = before_split
        .terminal_contexts
        .iter()
        .find(|candidate| candidate.terminal_context_id == terminal_context_id)
        .ok_or("Panel move context missing")?;
    let tab = context.tabs.first().ok_or("Panel move Tab missing")?;
    let source_panel_id = match &tab.root {
        PanelNode::Panel { panel_id, .. } => panel_id.clone(),
        PanelNode::Split { .. } => return Err("fresh native move Tab was already split".into()),
    };
    let tab_id = tab.tab_id.clone();
    let (split_revision, new_panel_id) = match peer.prepare(
        WorkbenchAction::PanelSplit {
            right_click: Default::default(),
            terminal_context_id: terminal_context_id.clone(),
            tab_id: tab_id.clone(),
            panel_id: source_panel_id.clone(),
            direction: WorkbenchPanelSplitDirection::Right,
            label: WorkbenchLabel::parse("Split peer")?,
            geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16)?,
            launch: None,
            profile: None,
            options: Default::default(),
        },
        split_deadline,
    )? {
        WorkbenchCommandOutcome::PanelSplitCreated {
            revision,
            new_panel_id,
            ..
        } => (revision, new_panel_id),
        outcome => return Err(format!("native PanelSplit did not start: {outcome:?}").into()),
    };

    // Rendezvous on the committed split before relocating its left leaf.
    let checkpoint = client.checkpoint();
    client.send(b"\x02w");
    client.wait_frame_text_after(
        checkpoint,
        Instant::now() + SWITCH_TIMEOUT,
        &[
            "Workbench navigator",
            &format!("Tabs refreshed · revision {}", split_revision.value()),
        ],
        &[],
        &[],
        &mut [],
    );
    let checkpoint = client.checkpoint();
    client.send(b"\x02w");
    client.wait_frame_text_after(
        checkpoint,
        Instant::now() + SWITCH_TIMEOUT,
        &[
            "Terminal 1 · 1",
            "Terminal 2 · Split peer",
            "Tab 1 · Ready · controlling",
        ],
        &["Workbench navigator"],
        &[],
        &mut [],
    );

    let move_deadline = Instant::now() + CLOSE_TIMEOUT;
    let moved_revision = match peer.prepare(
        WorkbenchAction::PanelMove {
            terminal_context_id: terminal_context_id.clone(),
            tab_id: tab_id.clone(),
            panel_id: source_panel_id.clone(),
            target_panel_id: new_panel_id.clone(),
            direction: WorkbenchPanelSplitDirection::Right,
        },
        move_deadline,
    )? {
        WorkbenchCommandOutcome::PanelMoved {
            revision,
            terminal_context_id: moved_context,
            tab_id: moved_tab,
            panel_id: moved_panel,
            target_panel_id: moved_target,
        } => {
            assert_eq!(moved_context, terminal_context_id);
            assert_eq!(moved_tab, tab_id);
            assert_eq!(moved_panel, source_panel_id);
            assert_eq!(moved_target, new_panel_id);
            revision
        }
        outcome => return Err(format!("native PanelMove did not commit: {outcome:?}").into()),
    };

    // The peer observes Split(new, source): leaves swapped order under a fresh Split, each leaf
    // keeps its Panel identity and terminal binding.
    let moved = peer.snapshot(move_deadline)?;
    assert_eq!(moved.revision, moved_revision);
    let moved_tab = moved
        .terminal_contexts
        .iter()
        .find(|candidate| candidate.terminal_context_id == terminal_context_id)
        .and_then(|candidate| candidate.tabs.first())
        .ok_or("Panel move Tab missing after commit")?;
    let PanelNode::Split { children, .. } = &moved_tab.root else {
        return Err("PanelMove did not retain a split root".into());
    };
    let [first, second] = children.as_ref();
    let PanelNode::Panel {
        panel_id: first_id, ..
    } = &first.node
    else {
        return Err("first leaf must stay a Panel".into());
    };
    let PanelNode::Panel {
        panel_id: second_id,
        ..
    } = &second.node
    else {
        return Err("second leaf must stay a Panel".into());
    };
    assert_eq!(first_id, &new_panel_id);
    assert_eq!(second_id, &source_panel_id);

    // A pure relocation never starts or ends a terminal: the source shell stays alive.
    let mut descriptor = [PollFd::new(source_shell.as_fd(), PollFlags::POLLIN)];
    poll(&mut descriptor, PollTimeout::ZERO)?;
    assert!(
        !descriptor[0]
            .revents()
            .is_some_and(|flags| flags.contains(PollFlags::POLLIN)),
        "source shell exited during PanelMove"
    );

    // The attached Client refreshes on the published move: the leaves swap positions while each
    // keeps its own label, and the Client's leaf keeps its shell binding — focus input still
    // reaches the original source shell even though the leaf changed position.
    let checkpoint = client.checkpoint();
    client.wait_frame_text_after(
        checkpoint,
        Instant::now() + SWITCH_TIMEOUT,
        &[
            "Terminal 1 · Split peer",
            "Terminal 2 · 1",
            "Tab 1 · Ready · controlling",
        ],
        &["Terminal disconnected"],
        &[],
        &mut [],
    );
    client.send(shell_identity_command(&fixture, "source", "native-move-after", false).as_bytes());
    assert_eq!(
        wait_file(
            &fixture.base.join("native-move-after"),
            &mut [&mut client],
            Instant::now() + OBSERVATION_TIMEOUT,
        ),
        source_identity,
        "moved Panel must keep its original shell",
    );

    client.close_before(Instant::now() + EXIT_TIMEOUT);
    Ok(())
}

#[test]
fn native_initial_workspace_refresh_catches_up_across_two_committed_peer_splits() -> TestResult {
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

    let deadline = Instant::now() + CLOSE_TIMEOUT;
    let mut peer = WorkbenchPeer::connect(&fixture, deadline)?;
    let baseline = peer.snapshot(deadline)?;
    let terminal_context_id = baseline.navigation.workspaces[0]
        .terminal_context_id
        .clone();
    let context = baseline
        .terminal_contexts
        .iter()
        .find(|candidate| candidate.terminal_context_id == terminal_context_id)
        .ok_or("catch-up context missing")?;
    let tab = context.tabs.first().ok_or("catch-up Tab missing")?;
    let panel_id = match &tab.root {
        PanelNode::Panel { panel_id, .. } => panel_id.clone(),
        PanelNode::Split { .. } => return Err("fresh catch-up Tab was already split".into()),
    };
    let tab_id = tab.tab_id.clone();
    let split_geometry = WorkbenchTerminalGeometry::new(80, 24, 8, 16)?;

    // The peer commits two PanelSplit revisions while the TUI retains the baseline revision; the
    // next refresh must fold both committed publications into one final snapshot.
    let first_panel_id = match peer.prepare(
        WorkbenchAction::PanelSplit {
            right_click: Default::default(),
            terminal_context_id: terminal_context_id.clone(),
            tab_id: tab_id.clone(),
            panel_id,
            direction: WorkbenchPanelSplitDirection::Right,
            label: WorkbenchLabel::parse("Peer one")?,
            geometry: split_geometry,
            launch: None,
            profile: None,
            options: Default::default(),
        },
        deadline,
    )? {
        WorkbenchCommandOutcome::PanelSplitCreated { new_panel_id, .. } => new_panel_id,
        outcome => {
            return Err(format!("first catch-up PanelSplit did not start: {outcome:?}").into());
        }
    };
    match peer.prepare(
        WorkbenchAction::PanelSplit {
            right_click: Default::default(),
            terminal_context_id: terminal_context_id.clone(),
            tab_id,
            panel_id: first_panel_id.clone(),
            direction: WorkbenchPanelSplitDirection::Right,
            label: WorkbenchLabel::parse("Peer two")?,
            geometry: split_geometry,
            launch: None,
            profile: None,
            options: Default::default(),
        },
        deadline,
    )? {
        WorkbenchCommandOutcome::PanelSplitCreated { .. } => {}
        outcome => {
            return Err(format!("second catch-up PanelSplit did not start: {outcome:?}").into());
        }
    };
    let after_splits = peer.snapshot(deadline)?;
    assert_eq!(
        after_splits.revision.value(),
        baseline.revision.value() + 2,
        "two committed PanelSplits must advance the journal by exactly +2"
    );
    let split_tab = after_splits
        .terminal_contexts
        .iter()
        .find(|candidate| candidate.terminal_context_id == terminal_context_id)
        .and_then(|candidate| candidate.tabs.first())
        .ok_or("catch-up Tab was not projected")?;
    fn leaf_count(node: &PanelNode) -> usize {
        match node {
            PanelNode::Panel { .. } => 1,
            PanelNode::Split { children, .. } => {
                children.iter().map(|slot| leaf_count(&slot.node)).sum()
            }
        }
    }
    assert_eq!(
        leaf_count(&split_tab.root),
        3,
        "folded splits must project three leaves"
    );

    let refresh_revision = after_splits.revision.value();
    let checkpoint = client.checkpoint();
    client.send(b"\x02w");
    client.wait_frame_text_after(
        checkpoint,
        Instant::now() + SWITCH_TIMEOUT,
        &[
            "Workbench navigator",
            &format!("Tabs refreshed · revision {refresh_revision}"),
        ],
        &[],
        &[],
        &mut [],
    );
    let checkpoint = client.checkpoint();
    client.send(b"\x02w");
    // Three Panels share the fixed row width, so the leaf labels clip; the numbered headers are
    // the visible membership evidence.
    client.wait_frame_text_after(
        checkpoint,
        Instant::now() + SWITCH_TIMEOUT,
        &[
            "Terminal 1 · 1",
            "Terminal 2",
            "Terminal 3",
            "Tab 1 · Ready · controlling",
        ],
        &["Workbench navigator"],
        &[],
        &mut [],
    );

    // The refreshed selection still drives the original shell.
    let checkpoint = client.checkpoint();
    client.send(b"\x02h");
    client.wait_frame_text_after(
        checkpoint,
        Instant::now() + SWITCH_TIMEOUT,
        &["Terminal 1 · 1", "Terminal 2", "Terminal 3"],
        &["Workbench navigator"],
        &[],
        &mut [],
    );
    client.close_before(Instant::now() + EXIT_TIMEOUT);
    Ok(())
}

#[test]
fn native_workspace_selection_creates_only_explicitly_and_retains_exact_shells() -> TestResult {
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
    client.send(
        shell_identity_command(
            &fixture,
            "initial_workspace",
            "initial_workspace-before",
            true,
        )
        .as_bytes(),
    );
    let initial_workspace = wait_file(
        &fixture.base.join("initial_workspace-before"),
        &mut [&mut client],
        Instant::now() + OBSERVATION_TIMEOUT,
    );
    let initial_workspace_pid = require_shell_identity(&initial_workspace, "initial_workspace");
    let deadline = Instant::now() + STARTUP_OR_CREATE_TIMEOUT;
    let mut peer = WorkbenchPeer::connect(&fixture, deadline)?;
    let WorkbenchCommandOutcome::ProjectCreated { project_id, .. } = peer.prepare(
        WorkbenchAction::ProjectCreate {
            label: WorkbenchLabel::parse("Context process")?,
        },
        deadline,
    )?
    else {
        return Err("Project preparation failed".into());
    };
    let WorkbenchCommandOutcome::WorkspaceCreated {
        workspace_id,
        terminal_context_id: configured_context,
        ..
    } = peer.prepare(
        WorkbenchAction::WorkspaceCreate {
            project_id: Some(project_id.clone()),
            label: WorkbenchLabel::parse("Configured")?,
        },
        deadline,
    )?
    else {
        return Err("configured Workspace preparation failed".into());
    };
    let directory = fixture.base.join("workspace with spaces");
    fs::create_dir(&directory)?;
    let directory = fs::canonicalize(directory)?;
    let directory_text = directory.to_str().ok_or("directory UTF-8")?;
    assert!(matches!(
        peer.prepare(
            WorkbenchAction::WorkspacePrimaryDirectoryAccept {
                workspace_id,
                path: WorkbenchDirectoryPath::parse(directory_text)?,
            },
            deadline,
        )?,
        WorkbenchCommandOutcome::WorkspacePrimaryDirectoryAccepted { .. }
    ));
    assert!(matches!(
        peer.prepare(
            WorkbenchAction::WorkspaceCreate {
                project_id: Some(project_id),
                label: WorkbenchLabel::parse("Unconfigured")?,
            },
            deadline,
        )?,
        WorkbenchCommandOutcome::WorkspaceCreated { .. }
    ));
    let baseline = peer.snapshot(deadline)?;
    let checkpoint = client.checkpoint();
    client.send(b"\x02w");
    let refreshed = format!("Tabs refreshed · revision {}", baseline.revision.value());
    client.wait_frame_text_after(
        checkpoint,
        Instant::now() + SWITCH_TIMEOUT,
        &[
            "Workbench navigator",
            &refreshed,
            "Configured",
            "Unconfigured",
        ],
        &[],
        &[],
        &mut [],
    );
    let checkpoint = client.checkpoint();
    client.send(b"\x02w");
    client.wait_frame_text_after(
        checkpoint,
        Instant::now() + SWITCH_TIMEOUT,
        &["Tab 1 · Ready · controlling"],
        &["Workbench navigator"],
        &[],
        &mut [],
    );

    // A second native Client observes the retained binding without acquiring control.
    // The joined output token does not occur in the input command's shell echo.
    let checkpoint = client.checkpoint();
    client.send(b"printf '%s%s\\n' INITIAL_WORKSPACE _PEER_BINDING\r");
    client.wait_terminal_output_after(
        checkpoint,
        Instant::now() + OBSERVATION_TIMEOUT,
        "INITIAL_WORKSPACE_PEER_BINDING",
    );
    let mut independent = Client::spawn(&fixture, "en_US.UTF-8");
    independent.wait_ready_after(
        0,
        Instant::now() + STARTUP_OR_CREATE_TIMEOUT,
        1,
        "1",
        ReadyMode::ViewOnly,
    );
    select_context(
        &mut client,
        b"\x02]",
        CONFIGURED_LOCATION,
        ExpectedContext::ConfiguredEmpty(directory_text),
    );
    observe_independent_initial_workspace(&mut independent, baseline.revision.value(), &mut client);
    assert_eq!(
        peer.snapshot(Instant::now() + OBSERVATION_TIMEOUT)?,
        baseline
    );
    // Leaving and returning to Empty must neither mutate navigation nor start a shell.
    select_context(
        &mut client,
        b"\x02[",
        INITIAL_WORKSPACE_LOCATION,
        ExpectedContext::Ready,
    );
    select_context(
        &mut client,
        b"\x02]",
        CONFIGURED_LOCATION,
        ExpectedContext::ConfiguredEmpty(directory_text),
    );
    assert_eq!(
        peer.snapshot(Instant::now() + OBSERVATION_TIMEOUT)?,
        baseline
    );
    let checkpoint = client.checkpoint();
    client.send(b"\x02c");
    client.wait_native_frame_text_after(
        checkpoint,
        Instant::now() + STARTUP_OR_CREATE_TIMEOUT,
        FrameExpectation::native(
            WorkbenchHeader::active(CONFIGURED_LOCATION, 1, "1"),
            &["Tab 1 · Ready · controlling"],
            &[EMPTY],
            &[],
        ),
        &mut [],
    );
    client.send(shell_identity_command(&fixture, "workspace", "workspace-before", true).as_bytes());
    let workspace = wait_file(
        &fixture.base.join("workspace-before"),
        &mut [&mut client],
        Instant::now() + OBSERVATION_TIMEOUT,
    );
    let workspace_pid = require_shell_identity(&workspace, "workspace");
    assert_ne!(workspace_pid, initial_workspace_pid);
    let workspace_shell = pidfd_open(
        Pid::from_raw(i32::try_from(workspace_pid)?).ok_or("invalid Workspace shell PID")?,
        PidfdFlags::empty(),
    )?;
    client.send(format!("pwd -P > {}/workspace-cwd\r", fixture.base.display()).as_bytes());
    assert_eq!(
        wait_file(
            &fixture.base.join("workspace-cwd"),
            &mut [&mut client],
            Instant::now() + OBSERVATION_TIMEOUT,
        )
        .trim_end(),
        directory.to_str().ok_or("directory UTF-8")?
    );
    let created = peer.snapshot(Instant::now() + OBSERVATION_TIMEOUT)?;
    let mut expected = baseline.clone();
    let created_context = created
        .terminal_contexts
        .iter()
        .find(|context| context.terminal_context_id == configured_context)
        .ok_or("configured context missing")?;
    assert_eq!(created_context.tabs.len(), 1);
    assert!(created.revision > baseline.revision);
    expected.revision = created.revision;
    expected
        .terminal_contexts
        .iter_mut()
        .find(|context| context.terminal_context_id == configured_context)
        .ok_or("baseline configured context missing")?
        .tabs = created_context.tabs.clone();
    assert_eq!(
        created, expected,
        "Create changes only active Workspace membership"
    );
    observe_independent_initial_workspace(&mut independent, created.revision.value(), &mut client);

    select_context(
        &mut client,
        b"\x02[",
        INITIAL_WORKSPACE_LOCATION,
        ExpectedContext::Ready,
    );
    client.send(shell_identity_command(&fixture, "", "initial_workspace-after", false).as_bytes());
    assert_eq!(
        wait_file(
            &fixture.base.join("initial_workspace-after"),
            &mut [&mut client],
            Instant::now() + OBSERVATION_TIMEOUT,
        ),
        initial_workspace
    );
    select_context(
        &mut client,
        b"\x02]",
        CONFIGURED_LOCATION,
        ExpectedContext::Ready,
    );
    client.send(shell_identity_command(&fixture, "", "workspace-after", false).as_bytes());
    assert_eq!(
        wait_file(
            &fixture.base.join("workspace-after"),
            &mut [&mut client],
            Instant::now() + OBSERVATION_TIMEOUT,
        ),
        workspace
    );

    // The native confirmation must target the active Workspace's retained Tab.
    let checkpoint = client.checkpoint();
    client.send(b"\x02x");
    client.wait_close_prompt_after(
        checkpoint,
        Instant::now() + OBSERVATION_TIMEOUT,
        CONFIGURED_LOCATION,
        &[
            "Close Tab?",
            "",
            "1",
            "",
            "Ends 1 terminal.",
            "",
            "[ Cancel  Esc ] [ Close  Enter ]",
        ],
        &mut [&mut independent],
    );
    let checkpoint = client.checkpoint();
    let deadline = Instant::now() + CLOSE_TIMEOUT;
    client.send(b"\r");
    let directory_fact = format!("Directory: {directory_text}");
    client.wait_native_frame_text_after(
        checkpoint,
        deadline,
        FrameExpectation::native(
            WorkbenchHeader::empty(CONFIGURED_LOCATION),
            &[EMPTY, &directory_fact, CONFIGURED_EMPTY_ACTION],
            &["Close Tab?", "Workbench navigator"],
            &[],
        ),
        &mut [&mut independent],
    );
    let closed = peer.snapshot(deadline)?;
    assert!(closed.revision > created.revision);
    let mut expected_closed = created.clone();
    expected_closed.revision = closed.revision;
    expected_closed
        .terminal_contexts
        .iter_mut()
        .find(|context| context.terminal_context_id == configured_context)
        .ok_or("configured context missing after Close")?
        .tabs
        .clear();
    assert_eq!(
        closed, expected_closed,
        "Close removes only the Workspace Tab and preserves initial_workspace membership"
    );
    loop {
        let mut descriptor = [PollFd::new(workspace_shell.as_fd(), PollFlags::POLLIN)];
        poll(&mut descriptor, PollTimeout::ZERO)?;
        assert!(
            Instant::now() < deadline,
            "Workspace shell exit arrived late"
        );
        if descriptor[0]
            .revents()
            .is_some_and(|flags| flags.contains(PollFlags::POLLIN))
        {
            break;
        }
        client.collect();
        independent.collect();
        thread::yield_now();
    }
    observe_independent_initial_workspace(&mut independent, closed.revision.value(), &mut client);

    select_context(
        &mut client,
        b"\x02]",
        UNCONFIGURED_LOCATION,
        ExpectedContext::UnconfiguredEmpty,
    );
    observe_independent_initial_workspace(&mut independent, closed.revision.value(), &mut client);
    assert_eq!(peer.snapshot(Instant::now() + OBSERVATION_TIMEOUT)?, closed);
    let checkpoint = client.checkpoint();
    client.send(b"\x02c");
    client.wait_native_frame_text_after(
        checkpoint,
        Instant::now() + STARTUP_OR_CREATE_TIMEOUT,
        FrameExpectation::native(
            WorkbenchHeader::empty(UNCONFIGURED_LOCATION),
            &[
                EMPTY,
                UNCONFIGURED_EMPTY_FACT,
                UNCONFIGURED_EMPTY_ACTION,
                "Tab not created",
            ],
            &[],
            &[],
        ),
        &mut [],
    );
    assert_eq!(
        peer.snapshot(Instant::now() + OBSERVATION_TIMEOUT)?,
        closed,
        "directory rejection must not create a fallback terminal"
    );
    select_context(
        &mut client,
        b"\x02]",
        INITIAL_WORKSPACE_LOCATION,
        ExpectedContext::Ready,
    );
    client.send(
        shell_identity_command(&fixture, "", "initial_workspace-after-close", false).as_bytes(),
    );
    assert_eq!(
        wait_file(
            &fixture.base.join("initial_workspace-after-close"),
            &mut [&mut client, &mut independent],
            Instant::now() + OBSERVATION_TIMEOUT,
        ),
        initial_workspace,
        "Workspace Close preserves the exact initial_workspace shell and its variable"
    );
    assert_eq!(peer.snapshot(Instant::now() + OBSERVATION_TIMEOUT)?, closed);
    client.close_before(Instant::now() + EXIT_TIMEOUT);
    independent.close_before(Instant::now() + EXIT_TIMEOUT);
    Ok(())
}

#[test]
fn native_bare_reentry_replays_the_settled_anchor_across_a_split_leaf() -> TestResult {
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
    client
        .send(shell_identity_command(&fixture, "anchor", "native-reentry-anchor", true).as_bytes());
    let anchor_identity = wait_file(
        &fixture.base.join("native-reentry-anchor"),
        &mut [&mut client],
        Instant::now() + OBSERVATION_TIMEOUT,
    );
    require_shell_identity(&anchor_identity, "anchor");

    // Commit a second leaf through the shared Workbench protocol; the anchor stays settled.
    let deadline = Instant::now() + CLOSE_TIMEOUT;
    let mut peer = WorkbenchPeer::connect(&fixture, deadline)?;
    let before_split = peer.snapshot(deadline)?;
    let terminal_context_id = before_split.navigation.workspaces[0]
        .terminal_context_id
        .clone();
    let context = before_split
        .terminal_contexts
        .iter()
        .find(|candidate| candidate.terminal_context_id == terminal_context_id)
        .ok_or("reentry context missing")?;
    let tab = context.tabs.first().ok_or("reentry Tab missing")?;
    let source_panel_id = match &tab.root {
        PanelNode::Panel { panel_id, .. } => panel_id.clone(),
        PanelNode::Split { .. } => return Err("fresh reentry Tab was already split".into()),
    };
    let tab_id = tab.tab_id.clone();
    let new_panel_id = match peer.prepare(
        WorkbenchAction::PanelSplit {
            right_click: Default::default(),
            terminal_context_id: terminal_context_id.clone(),
            tab_id,
            panel_id: source_panel_id.clone(),
            direction: WorkbenchPanelSplitDirection::Right,
            label: WorkbenchLabel::parse("Split peer")?,
            geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16)?,
            launch: None,
            profile: None,
            options: Default::default(),
        },
        deadline,
    )? {
        WorkbenchCommandOutcome::PanelSplitCreated { new_panel_id, .. } => new_panel_id,
        outcome => return Err(format!("reentry PanelSplit did not start: {outcome:?}").into()),
    };
    let split = peer.snapshot(deadline)?;
    let split_tab = split
        .terminal_contexts
        .iter()
        .find(|candidate| candidate.terminal_context_id == terminal_context_id)
        .and_then(|candidate| candidate.tabs.first())
        .ok_or("reentry split Tab was not projected")?;
    let PanelNode::Split { children, .. } = &split_tab.root else {
        return Err("reentry PanelSplit did not project a split root".into());
    };
    let [anchor_slot, split_slot] = children.as_ref();
    assert!(matches!(
        &anchor_slot.node,
        PanelNode::Panel { panel_id, .. } if panel_id == &source_panel_id
    ));
    assert!(matches!(
        &split_slot.node,
        PanelNode::Panel { panel_id, .. } if panel_id == &new_panel_id
    ));

    let process_names = ["orchbee-server", "orchbee-worker", "orchbee-terminal-host"];
    let retained = process_names.map(|name| one_process(&fixture, name));
    client.close_before(Instant::now() + EXIT_TIMEOUT);

    // Bare reentry must replay the settled anchor reference across the live multi-leaf tree:
    // the same shell, the same managed processes, and no new Workbench revision.
    let mut reopened = Client::spawn(&fixture, "en_US.UTF-8");
    reopened.wait_ready_after(
        0,
        Instant::now() + STARTUP_OR_CREATE_TIMEOUT,
        1,
        "1",
        ReadyMode::Controlling,
    );
    for (name, expected) in process_names.into_iter().zip(retained) {
        assert_process_retained(&fixture, name, &expected);
    }
    reopened.send(shell_identity_command(&fixture, "", "native-reentry-after", false).as_bytes());
    assert_eq!(
        wait_file(
            &fixture.base.join("native-reentry-after"),
            &mut [&mut reopened],
            Instant::now() + OBSERVATION_TIMEOUT,
        ),
        anchor_identity,
        "bare reentry must return the original anchor shell and its marker",
    );
    let observe = Instant::now() + OBSERVATION_TIMEOUT;
    let after = peer.snapshot(observe)?;
    assert_eq!(
        after.revision, split.revision,
        "bare reentry must not write a Workbench revision"
    );
    assert!(matches!(
        peer.publication(split.revision, observe)?,
        WorkbenchPublicationOutcome::Current { .. } | WorkbenchPublicationOutcome::Published { .. }
    ));
    reopened.close_before(Instant::now() + EXIT_TIMEOUT);
    Ok(())
}
