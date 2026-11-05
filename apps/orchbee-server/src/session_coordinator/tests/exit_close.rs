//! A Shell that exits with code 0 closes its Panel only through the canonical Close admission;
//! programs, stale incarnations and shared Panels keep their settled output. Lifecycle replies
//! are the labeled synthetic seam of `drive_close_to_publication`.
use super::super::target_lifecycle::target_binding;
use super::multileaf_membership::drive_close_to_publication;
use super::panel_split::{dispatch_split, split_command, workspace_leaf};
use super::tab_create::{dispatch_tab_with, immediate_completion, tab_command};
use super::*;
use crate::terminal_coordinator::BindingViewerTarget;
use orchbee_workbench_protocol::{
    LaunchSelection, PresentationBinding, WorkbenchAction, WorkbenchCommand, WorkbenchLabel,
    WorkbenchPanelSplitDirection, WorkbenchPanelTerminalTarget, WorkbenchTerminalGeometry,
};
use std::time::Instant;

fn target(binding: &PresentationBinding) -> BindingViewerTarget {
    BindingViewerTarget {
        terminal_id: orchbee_capability_node_protocol::TerminalId::from_bytes(
            u128::from_str_radix(binding.terminal_id.as_str(), 16)
                .unwrap()
                .to_be_bytes(),
        )
        .unwrap(),
        identity: identity(),
        worker: worker(1),
    }
}

/// Primary Presentation targets of every leaf, in tree order.
fn leaves(node: &PanelNode) -> Vec<BindingViewerTarget> {
    match node {
        PanelNode::Panel {
            primary_presentation,
            ..
        } => vec![target(primary_presentation.binding.as_ref().unwrap())],
        PanelNode::Split { children, .. } => children
            .iter()
            .flat_map(|slot| leaves(&slot.node))
            .collect(),
    }
}

/// Opens a started Tab whose terminal used `launch` and returns its viewer target.
fn open_tab(
    coordinator: &mut SessionCoordinator,
    request: u128,
    launch: Option<LaunchSelection>,
) -> BindingViewerTarget {
    open_tab_with(coordinator, request, launch, &LaunchCapture::absent())
}

/// Opens a started Tab whose terminal used `launch` with `capture`.
fn open_tab_with(
    coordinator: &mut SessionCoordinator,
    request: u128,
    launch: Option<LaunchSelection>,
    capture: &LaunchCapture,
) -> BindingViewerTarget {
    let revision = snapshot(coordinator).revision.value();
    let mut command = tab_command(coordinator, request, revision, "shell");
    let WorkbenchAction::TabCreate {
        launch: selected, ..
    } = &mut command.action
    else {
        unreachable!("tab.create command")
    };
    *selected = launch;
    let pending = dispatch_tab_with(coordinator, command, request + 1, capture, None);
    let completion = immediate_completion(pending.command());
    coordinator
        .complete_terminal_tab_create(pending, completion, Some(worker(1)))
        .unwrap();
    let committed = snapshot(coordinator);
    leaves(&committed.terminal_contexts[0].tabs.last().unwrap().root)[0]
}

fn exit_normally(
    coordinator: &mut SessionCoordinator,
    target: BindingViewerTarget,
    request: u128,
) -> bool {
    coordinator.close_after_normal_exit(
        target,
        || WorkbenchRequestId::parse(&format!("{request:032x}")).ok(),
        Some(worker(1)),
        true,
        Instant::now(),
    )
}

/// The Close that owns `target`. The shared driver follows a Close only by its request identity,
/// so the action here carries no claim.
fn admitted_close(
    coordinator: &mut SessionCoordinator,
    target: BindingViewerTarget,
) -> WorkbenchCommand {
    let request_id = coordinator
        .close_authority()
        .unwrap()
        .terminal_tab_close_request_for_binding(&target_binding(target).unwrap())
        .expect("admitted Close")
        .clone();
    let committed = snapshot(coordinator);
    let context = &committed.terminal_contexts[0];
    WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id,
        session_id: committed.session_id.clone(),
        expected_revision: committed.revision,
        action: WorkbenchAction::TabClose {
            terminal_context_id: context.terminal_context_id.clone(),
            tab_id: context.tabs[0].tab_id.clone(),
        },
    }
}

fn tab_leaves(coordinator: &mut SessionCoordinator) -> Vec<Vec<BindingViewerTarget>> {
    snapshot(coordinator).terminal_contexts[0]
        .tabs
        .iter()
        .map(|tab| leaves(&tab.root))
        .collect()
}

#[test]
fn a_normal_shell_exit_closes_its_sole_panel_tab_once() {
    let mut coordinator = session();
    let exited = open_tab(&mut coordinator, 10, None);
    let survivor = open_tab(&mut coordinator, 20, None);
    let revision = snapshot(&mut coordinator).revision;
    let other_creation = HostTerminalIdentity::new(
        HostCreationId::from_bytes([7; 16]).unwrap(),
        exited.identity.host_epoch(),
        exited.identity.terminal_id(),
    );
    for stale in [
        BindingViewerTarget {
            worker: worker(2),
            ..exited
        },
        BindingViewerTarget {
            identity: other_creation,
            ..exited
        },
    ] {
        assert!(!exit_normally(&mut coordinator, stale, 30));
    }
    assert_eq!(snapshot(&mut coordinator).revision, revision);

    assert!(exit_normally(&mut coordinator, exited, 31));
    // Every later viewer of the same exit finds the Close already owning the target.
    assert_eq!(
        coordinator.target_lifecycle(exited),
        TargetLifecycle::Fenced
    );
    assert!(!exit_normally(&mut coordinator, exited, 32));
    let close = admitted_close(&mut coordinator, exited);
    assert_eq!(
        close.request_id,
        WorkbenchRequestId::parse(&format!("{:032x}", 31)).unwrap()
    );
    drive_close_to_publication(&mut coordinator, &close);

    assert_eq!(tab_leaves(&mut coordinator), [vec![survivor]]);
}

#[test]
fn a_normal_exit_in_a_split_closes_only_that_leaf() {
    let mut coordinator = session();
    let kept = open_tab(&mut coordinator, 10, None);
    let (context, tab, panel) = workspace_leaf(&mut coordinator);
    let split = split_command(
        &mut coordinator,
        20,
        context,
        tab,
        panel,
        WorkbenchPanelSplitDirection::Right,
    );
    let pending = dispatch_split(&mut coordinator, split, 21);
    let completion = immediate_completion(pending.command());
    coordinator
        .complete_terminal_panel_create(pending, completion, Some(worker(1)))
        .unwrap();
    let [members] = tab_leaves(&mut coordinator).try_into().unwrap();
    let [first, exited] = members.try_into().unwrap();
    assert_eq!(first, kept);

    assert!(exit_normally(&mut coordinator, exited, 30));
    let close = admitted_close(&mut coordinator, exited);
    drive_close_to_publication(&mut coordinator, &close);

    assert_eq!(tab_leaves(&mut coordinator), [vec![kept]]);
}

#[test]
fn only_interactive_shell_launches_close_after_a_normal_exit() {
    // The Host's own classification vectors (`process_test.zig`) are the reference; a launch
    // selection always names an absolute executable.
    let pwsh = r"C:\Program Files\PowerShell\7\pwsh.exe";
    let cmd = r"C:\Windows\System32\cmd.exe";
    let cases: [(&str, &[&str], bool); 12] = [
        (pwsh, &[], true),
        (
            r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe",
            &["-NoLogo", "-NoProfile"],
            true,
        ),
        ("C:/Program Files/PowerShell/7/PWSH.EXE", &["-NoExit"], true),
        (cmd, &["/D"], true),
        ("/bin/zsh", &["-l"], true),
        (r"C:\tools\codex.exe", &[], false),
        ("/usr/bin/python", &["agent.py"], false),
        (pwsh, &["-Command", "Start-Sleep 60"], false),
        (pwsh, &["-NoExit", "-File", "agent.ps1"], false),
        (cmd, &["/C", "agent.exe"], false),
        ("/bin/sh", &["-c", "exec agent"], false),
        ("/bin/bash", &["script.sh"], false),
    ];
    for (executable, arguments, closes) in cases {
        let mut coordinator = session();
        let launch = LaunchSelection::new(
            executable.to_owned(),
            arguments
                .iter()
                .map(|argument| (*argument).to_owned())
                .collect(),
        )
        .unwrap();
        let exited = open_tab(&mut coordinator, 10, Some(launch));
        assert_eq!(
            exit_normally(&mut coordinator, exited, 20),
            closes,
            "{executable} {arguments:?}"
        );
    }
}

#[test]
fn a_default_launch_closes_by_the_default_program_its_capture_dispatched() {
    use orchbee_capability_node_protocol::{
        DefaultCustomProgram, DefaultLaunchProgram, LaunchPreferences, TerminalLaunchPreferences,
        TerminalLaunchThemes,
    };
    let preferences = |program| {
        let themes = TerminalLaunchThemes::new("l".into(), "d".into()).unwrap();
        LaunchPreferences::Available(
            TerminalLaunchPreferences::new(1_000, None, themes, Some(program)).unwrap(),
        )
    };
    let custom = |executable: &str, arguments: &[&str]| {
        preferences(DefaultLaunchProgram::Custom(
            DefaultCustomProgram::new(
                executable.to_owned(),
                arguments
                    .iter()
                    .map(|argument| (*argument).to_owned())
                    .collect(),
            )
            .unwrap(),
        ))
    };
    let pwsh = r"C:\Program Files\PowerShell\7\pwsh.exe";
    let cases = [
        (
            preferences(DefaultLaunchProgram::SystemShell { login: true }),
            None,
            true,
        ),
        (custom(pwsh, &["-NoLogo"]), None, true),
        (custom("/bin/zsh", &["-l"]), None, true),
        (custom(r"C:\tools\codex.exe", &[]), None, false),
        (custom(pwsh, &["-Command", "agent"]), None, false),
        // An explicit selection launches its own argv, never the default program.
        (
            custom(r"C:\tools\codex.exe", &[]),
            Some(LaunchSelection::new(pwsh.to_owned(), Vec::new()).unwrap()),
            true,
        ),
        (
            custom(pwsh, &[]),
            Some(LaunchSelection::new(r"C:\tools\codex.exe".to_owned(), Vec::new()).unwrap()),
            false,
        ),
    ];
    for (default, launch, closes) in cases {
        let mut coordinator = session();
        let capture = LaunchCapture::new(LaunchPreferences::Absent, default.clone());
        let exited = open_tab_with(&mut coordinator, 10, launch.clone(), &capture);
        assert_eq!(
            exit_normally(&mut coordinator, exited, 20),
            closes,
            "{default:?} {launch:?}"
        );
    }
}

#[test]
fn a_panel_with_another_presentation_keeps_an_exited_shell() {
    let mut coordinator = session();
    let shell = open_tab(&mut coordinator, 10, None);
    let (context, tab, panel) = workspace_leaf(&mut coordinator);
    let committed = snapshot(&mut coordinator);
    let command = WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{:032x}", 20)).unwrap(),
        session_id: committed.session_id,
        expected_revision: committed.revision,
        action: WorkbenchAction::PanelCreateTerminal {
            terminal_context_id: context,
            tab_id: tab,
            panel_id: panel,
            target: WorkbenchPanelTerminalTarget::Child {
                label: WorkbenchLabel::parse("child").unwrap(),
            },
            geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
            launch: None,
            profile: None,
            options: Default::default(),
        },
    };
    let PreparedTerminalPanelCreate::Dispatch(pending) = coordinator.prepare_terminal_panel_create(
        command,
        CreateInputs::new(
            Some(id(21)),
            Some(expiry()),
            Some(worker(1)),
            LaunchCapture::absent(),
        ),
    ) else {
        panic!("child Presentation dispatch expected")
    };
    let completion = immediate_completion(pending.command());
    coordinator
        .complete_terminal_panel_create(pending, completion, Some(worker(1)))
        .unwrap();
    let PanelNode::Panel {
        child_presentations,
        ..
    } = &snapshot(&mut coordinator).terminal_contexts[0].tabs[0].root
    else {
        panic!("Panel expected")
    };
    let child = target(child_presentations[0].binding.as_ref().unwrap());

    for exited in [shell, child] {
        assert!(!exit_normally(&mut coordinator, exited, 30));
        assert_eq!(
            coordinator.target_lifecycle(exited),
            TargetLifecycle::Unfenced
        );
    }
}
