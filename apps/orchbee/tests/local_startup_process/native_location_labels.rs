use super::native_workspace_fixture::*;
use super::*;
use orchbee_workbench_protocol::Snapshot;
use std::path::Path;

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

    fn actions(self) -> &'static str {
        match self {
            Self::Workspace => {
                "Enter · rename Workspace    Esc · cancel    Backspace · delete    Ctrl+U · clear"
            }
            Self::Project => {
                "Enter · rename Project    Esc · cancel    Backspace · delete    Ctrl+U · clear"
            }
        }
    }

    fn hint(self) -> &'static str {
        match self {
            Self::Workspace => {
                "Use 1–512 characters. The shared Workspace label changes for all Clients."
            }
            Self::Project => {
                "Use 1–512 characters. The shared Project label changes for all Clients."
            }
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
    let project_row = format!("Project: {project}");
    let workspace_row = format!("Workspaces: {workspace}");
    let mut rows = vec![target.title(), "", &project_row, ""];
    if matches!(target, Target::Workspace) {
        rows.extend([workspace_row.as_str(), ""]);
    }
    rows.extend([text, "", target.hint(), "", target.actions()]);
    modal(client, checkpoint, &header(project, workspace), &rows);
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
    format!("WORKBENCH · {project} / {workspace}")
}

fn wait_workspace(client: &mut Client, checkpoint: usize, header: &str, notice: Option<&str>) {
    let mut required = vec![header, "[*1 1]"];
    required.extend(notice);
    client.wait_frame_text_after(
        checkpoint,
        Instant::now() + SWITCH_TIMEOUT,
        &required,
        &[
            "Rename Workspace",
            "Rename Project",
            "Workbench navigator",
            EMPTY,
        ],
        &[],
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
    peer.wait_frame_text_after(
        checkpoint,
        Instant::now() + SWITCH_TIMEOUT,
        &[
            expected_header,
            "[*1 1]",
            "Tab 1 · Ready ·",
            "LOCATION_SHELL_RETAINED",
        ],
        &["Workbench navigator", EMPTY],
        &[],
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
    let fixture = Fixture::new();
    let mut client = Client::spawn(&fixture, "en_US.UTF-8");
    client.wait_ready_after(
        0,
        Instant::now() + STARTUP_OR_CREATE_TIMEOUT,
        1,
        "1",
        ReadyMode::Controlling,
    );
    let mut observer = SnapshotObserver::connect(&fixture)?;
    let standalone = observer.snapshot()?;
    assert!(standalone.navigation.projects.is_empty());
    for (target, notice) in [
        (
            Target::Workspace,
            "No current Workspace to rename · no command was sent",
        ),
        (
            Target::Project,
            "No current Project to rename · no command was sent",
        ),
    ] {
        let checkpoint = client.checkpoint();
        client.send(target.shortcut());
        client.wait_frame_text_after(
            checkpoint,
            Instant::now() + SWITCH_TIMEOUT,
            &[STANDALONE_HEADER, "[*1 1]", notice],
            &[target.title()],
            &[],
            &mut [],
        );
        assert_eq!(
            observer.snapshot()?,
            standalone,
            "standalone has no location label target"
        );
    }
    seed_empty_workspace(&fixture, &mut client)?;
    let directory = fixture.base.join("location label directory");
    fs::create_dir(&directory)?;
    let directory = fs::canonicalize(directory)?;
    let checkpoint = submit_directory(&mut client, directory.to_str().ok_or("path UTF-8")?);
    wait_empty(&mut client, checkpoint, Some(DIRECTORY_ACCEPTED));
    finish_modal_handoff(&mut client, &[WORKSPACE_HEADER, EMPTY]);
    let checkpoint = client.checkpoint();
    client.send(b"\x02c");
    client.wait_frame_text_after(
        checkpoint,
        Instant::now() + STARTUP_OR_CREATE_TIMEOUT,
        &[WORKSPACE_HEADER, "[*1 1]", "Tab 1 · Ready · controlling"],
        &[EMPTY],
        &[],
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
    peer.wait_frame_text_after(
        checkpoint,
        Instant::now() + SWITCH_TIMEOUT,
        &[
            WORKSPACE_HEADER,
            "[*1 1]",
            "Tab 1 · Ready · view only",
            "LOCATION_SHELL_RETAINED",
        ],
        &[EMPTY],
        &[],
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
            Target::Workspace => expected.navigation.workspaces[0].label = label.to_owned(),
            Target::Project => expected.navigation.projects[0].label = label.to_owned(),
        }
        assert_eq!(
            renamed, expected,
            "only the requested canonical label may change"
        );
        // The peer installs the committed rename through the publication probe itself —
        // no navigator, no `w` — replacing the previous header on its own.
        peer.wait_frame_text_after(
            peer_checkpoint,
            Instant::now() + OBSERVATION_TIMEOUT,
            &[&renamed_header],
            &[],
            &[],
            &mut [&mut client],
        );
        finish_modal_handoff(&mut client, &[&renamed_header, "[*1 1]"]);
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
        finish_modal_handoff(&mut client, &[&current_header, "[*1 1]"]);
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
        finish_modal_handoff(&mut client, &[&current_header, "[*1 1]"]);
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
