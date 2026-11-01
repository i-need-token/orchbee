use super::native_workspace_fixture::*;
use super::pty_client::FrameExpectation;
use super::tui_observer::{ModalRow, WorkbenchHeader};
use super::*;
use orchbee_workbench_protocol::Snapshot;
use std::path::Path;

const RENAME_ACTIONS: &str = "[ Cancel  Esc ] [ Rename  Enter ]";

#[derive(Clone, Copy)]
enum Target {
    Workspace,
    Project,
}

impl Target {
    fn shortcut(self) -> &'static [u8] {
        match self {
            Self::Workspace => b"\x02r",
            Self::Project => b"\x02R",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::Workspace => "Rename Workspace",
            Self::Project => "Rename Project",
        }
    }

    fn renamed_notice(self) -> &'static str {
        match self {
            Self::Workspace => "Workspace renamed",
            Self::Project => "Project renamed",
        }
    }
}

fn unchanged_notice(target: Target) -> &'static str {
    match target {
        Target::Workspace => "Workspace label unchanged",
        Target::Project => "Project label unchanged",
    }
}

fn editor(
    client: &mut Client,
    checkpoint: usize,
    target: Target,
    project: &str,
    workspace: &str,
    text: &str,
) {
    // The title names the value, so the field stands alone; its reserved help row stays blank
    // while the label is valid.
    let rows = [
        ModalRow::exact(target.title()),
        ModalRow::exact(""),
        ModalRow::field_at_end(text),
        ModalRow::exact(""),
        ModalRow::exact(""),
        ModalRow::exact(RENAME_ACTIONS),
    ];
    let location = header(project, workspace);
    modal(
        client,
        checkpoint,
        WorkbenchHeader::active(&location, 1, "1"),
        &rows,
    );
}

fn open_editor(client: &mut Client, target: Target, project: &str, workspace: &str) {
    let checkpoint = client.checkpoint();
    client.send(target.shortcut());
    let label = match target {
        Target::Workspace => workspace,
        Target::Project => project,
    };
    editor(client, checkpoint, target, project, workspace, label);
}

fn edit_label(client: &mut Client, target: Target, project: &str, workspace: &str, label: &str) {
    let checkpoint = client.checkpoint();
    client.send(format!("\x15{label}").as_bytes());
    editor(client, checkpoint, target, project, workspace, label);
}

fn header(project: &str, workspace: &str) -> String {
    format!("{project} / {workspace}")
}

fn wait_workspace(client: &mut Client, checkpoint: usize, header: &str, notice: Option<&str>) {
    let mut required = Vec::new();
    required.extend(notice);
    client.wait_native_frame_text_after(
        checkpoint,
        Instant::now() + SWITCH_TIMEOUT,
        FrameExpectation::native(
            WorkbenchHeader::active(header, 1, "1"),
            &required,
            &[
                "Rename Workspace",
                "Rename Project",
                "Workbench navigator",
                EMPTY,
            ],
            &[],
        ),
        &mut [],
    );
}

fn observe_peer_refresh(
    peer: &mut Client,
    active: &mut Client,
    snapshot: &Snapshot,
    expected_header: &str,
) {
    let checkpoint = peer.checkpoint();
    peer.send(b"\x02w");
    let refreshed = format!("Tabs refreshed · revision {}", snapshot.revision.value());
    peer.wait_frame_text_after(
        checkpoint,
        Instant::now() + SWITCH_TIMEOUT,
        &["Workbench navigator", &refreshed],
        &[],
        &[],
        &mut [active],
    );
    let checkpoint = peer.checkpoint();
    peer.send(b"\x02w");
    // Active-header identity cannot be satisfied by a matching label in the navigator.
    peer.wait_native_frame_text_after(
        checkpoint,
        Instant::now() + SWITCH_TIMEOUT,
        FrameExpectation::native(
            WorkbenchHeader::active(expected_header, 1, "1"),
            &["Tab 1 · Ready ·", "LOCATION_SHELL_RETAINED"],
            &["Workbench navigator", EMPTY],
            &[],
        ),
        &mut [active],
    );
}

fn assert_cwd(fixture: &Fixture, client: &mut Client, directory: &Path, filename: &str) {
    client.send(format!("pwd -P > {}/{}\r", fixture.base.display(), filename).as_bytes());
    assert_eq!(
        wait_file(
            &fixture.base.join(filename),
            &mut [client],
            Instant::now() + OBSERVATION_TIMEOUT,
        )
        .trim_end(),
        directory.to_str().unwrap(),
        "location labels must not change the live shell's working directory",
    );
}

#[test]
fn native_location_rename_refresh_cancel_and_unchanged_preserve_the_live_workspace() -> TestResult {
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
    let mut observer = SnapshotObserver::connect(&fixture)?;
    let initial_workspace = observer.snapshot()?;
    assert!(initial_workspace.navigation.projects.is_empty());
    {
        let (target, notice) = (
            Target::Project,
            "No current Project to rename · no command was sent",
        );
        let checkpoint = client.checkpoint();
        client.send(target.shortcut());
        client.wait_native_frame_text_after(
            checkpoint,
            Instant::now() + SWITCH_TIMEOUT,
            FrameExpectation::native(
                WorkbenchHeader::active(INITIAL_WORKSPACE_LOCATION, 1, "1"),
                &[notice],
                &[target.title()],
                &[],
            ),
            &mut [],
        );
        assert_eq!(
            observer.snapshot()?,
            initial_workspace,
            "ungrouped Workspace has no Project rename target"
        );
    }
    seed_empty_workspace(&fixture, &mut client)?;
    let directory = fixture.base.join("location label directory");
    fs::create_dir(&directory)?;
    let directory = fs::canonicalize(directory)?;
    let checkpoint = submit_directory(&mut client, directory.to_str().ok_or("path UTF-8")?);
    wait_empty(
        &mut client,
        checkpoint,
        Some(directory.to_str().ok_or("path UTF-8")?),
        Some(DIRECTORY_ACCEPTED),
    );
    finish_modal_handoff(
        &mut client,
        WorkbenchHeader::empty(WORKSPACE_LOCATION),
        &[EMPTY],
    );
    let checkpoint = client.checkpoint();
    client.send(b"\x02c");
    client.wait_native_frame_text_after(
        checkpoint,
        Instant::now() + STARTUP_OR_CREATE_TIMEOUT,
        FrameExpectation::native(
            WorkbenchHeader::active(WORKSPACE_LOCATION, 1, "1"),
            &["Tab 1 · Ready · controlling"],
            &[EMPTY],
            &[],
        ),
        &mut [],
    );
    client.send(shell_identity_command(&fixture, "location", "location-before", true).as_bytes());
    let identity = wait_file(
        &fixture.base.join("location-before"),
        &mut [&mut client],
        Instant::now() + OBSERVATION_TIMEOUT,
    );
    require_shell_identity(&identity, "location");
    assert_cwd(&fixture, &mut client, &directory, "location-cwd-before");
    let checkpoint = client.checkpoint();
    client.send(b"printf '%s%s\\n' LOCATION_SHELL_ RETAINED\r");
    client.wait_terminal_output_after(
        checkpoint,
        Instant::now() + OBSERVATION_TIMEOUT,
        "LOCATION_SHELL_RETAINED",
    );
    let mut expected = observer.snapshot()?;

    // A second native Client selects this Workspace by the existing context controls.
    // The observer connection above remains SnapshotRead-only throughout the scenario.
    let mut peer = Client::spawn(&fixture, "en_US.UTF-8");
    peer.wait_ready_after(
        0,
        Instant::now() + STARTUP_OR_CREATE_TIMEOUT,
        1,
        "1",
        ReadyMode::Controlling,
    );
    let checkpoint = peer.checkpoint();
    peer.send(b"\x02]");
    peer.wait_native_frame_text_after(
        checkpoint,
        Instant::now() + SWITCH_TIMEOUT,
        FrameExpectation::native(
            WorkbenchHeader::active(WORKSPACE_LOCATION, 1, "1"),
            &["Tab 1 · Ready · view only", "LOCATION_SHELL_RETAINED"],
            &[EMPTY],
            &[],
        ),
        &mut [&mut client],
    );

    let mut project = "Setup process";
    let mut workspace = "Chosen workspace";
    for (target, label, checkpoint_name) in [
        (
            Target::Workspace,
            "Renamed workspace",
            "workspace-label-changed",
        ),
        (Target::Project, "Renamed project", "project-label-changed"),
    ] {
        open_editor(&mut client, target, project, workspace);
        edit_label(&mut client, target, project, workspace, label);
        // The peer checkpoint precedes the commit so its auto-installed frame always lands
        // after it, however the publication/refresh interleaves with the renamer's draw.
        let peer_checkpoint = peer.checkpoint();
        let checkpoint = client.checkpoint();
        client.send(b"\r");
        match target {
            Target::Workspace => workspace = label,
            Target::Project => project = label,
        }
        let renamed_header = header(project, workspace);
        wait_workspace(
            &mut client,
            checkpoint,
            &renamed_header,
            Some(target.renamed_notice()),
        );
        let renamed = observer.snapshot()?;
        assert_eq!(renamed.revision.value(), expected.revision.value() + 1);
        expected.revision = renamed.revision;
        match target {
            Target::Workspace => expected.navigation.workspaces[1].label = label.to_owned(),
            Target::Project => expected.navigation.projects[0].label = label.to_owned(),
        }
        assert_eq!(
            renamed, expected,
            "only the requested canonical label may change"
        );
        // The peer installs the committed rename through the publication probe itself —
        // no navigator, no `w` — replacing the previous header on its own.
        peer.wait_native_frame_text_after(
            peer_checkpoint,
            Instant::now() + OBSERVATION_TIMEOUT,
            FrameExpectation::native(
                WorkbenchHeader::active(&renamed_header, 1, "1"),
                &[],
                &[],
                &[],
            ),
            &mut [&mut client],
        );
        finish_modal_handoff(
            &mut client,
            WorkbenchHeader::active(&renamed_header, 1, "1"),
            &[],
        );
        assert_shell_survives(&fixture, &mut client, &identity, checkpoint_name);
        observe_peer_refresh(&mut peer, &mut client, &expected, &renamed_header);
    }

    for (target, label, checkpoint_name) in [
        (Target::Workspace, workspace, "workspace-label-unchanged"),
        (Target::Project, project, "project-label-unchanged"),
    ] {
        open_editor(&mut client, target, project, workspace);
        let checkpoint = client.checkpoint();
        client.send(b"\r");
        let current_header = header(project, workspace);
        wait_workspace(
            &mut client,
            checkpoint,
            &current_header,
            Some(unchanged_notice(target)),
        );
        assert_eq!(
            observer.snapshot()?,
            expected,
            "unchanged submission preserves revision and every canonical fact"
        );
        finish_modal_handoff(
            &mut client,
            WorkbenchHeader::active(&current_header, 1, "1"),
            &[],
        );
        assert_shell_survives(&fixture, &mut client, &identity, checkpoint_name);

        open_editor(&mut client, target, project, workspace);
        edit_label(
            &mut client,
            target,
            project,
            workspace,
            &format!("{label} discarded"),
        );
        let checkpoint = client.checkpoint();
        client.send(b"\x1b");
        wait_workspace(&mut client, checkpoint, &current_header, None);
        assert_eq!(
            observer.snapshot()?,
            expected,
            "cancel allocates no canonical mutation"
        );
        finish_modal_handoff(
            &mut client,
            WorkbenchHeader::active(&current_header, 1, "1"),
            &[],
        );
        assert_shell_survives(
            &fixture,
            &mut client,
            &identity,
            &format!("{checkpoint_name}-cancel"),
        );
    }
    assert_cwd(&fixture, &mut client, &directory, "location-cwd-after");
    assert_eq!(observer.snapshot()?, expected);
    peer.close_before(Instant::now() + EXIT_TIMEOUT);
    client.close_before(Instant::now() + EXIT_TIMEOUT);
    Ok(())
}
