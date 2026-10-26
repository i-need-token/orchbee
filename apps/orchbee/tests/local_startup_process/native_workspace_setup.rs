use super::native_workspace_fixture::*;
use super::tui_observer::WorkbenchHeader;
use super::*;

#[test]
fn native_empty_workspace_uses_literal_directory_and_only_explicitly_launches() -> TestResult {
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
        shell_identity_command(&fixture, "initial_workspace", "setup-before", true).as_bytes(),
    );
    let initial_workspace = wait_file(
        &fixture.base.join("setup-before"),
        &mut [&mut client],
        Instant::now() + OBSERVATION_TIMEOUT,
    );
    let initial_workspace_pid = require_shell_identity(&initial_workspace, "initial_workspace");
    let mut observer = SnapshotObserver::connect(&fixture)?;
    let initial = observer.snapshot()?;
    assert!(
        initial.navigation.projects.is_empty(),
        "fresh Session has no Projects"
    );
    assert_eq!(initial.terminal_contexts.len(), 1);
    assert_eq!(initial.terminal_contexts[0].tabs.len(), 1);

    seed_empty_workspace(&fixture, &mut client)?;
    let created = observer.snapshot()?;
    assert_eq!(created.session_id, initial.session_id);
    assert!(created.revision > initial.revision);
    assert_eq!(created.navigation.projects.len(), 1);
    let project = &created.navigation.projects[0];
    assert_eq!(project.label, "Setup process");
    assert_eq!(created.navigation.workspaces.len(), 2);
    assert_eq!(created.navigation.directories.len(), 1);
    let workspace = &created.navigation.workspaces[1];
    assert_eq!(workspace.project_id.as_ref(), Some(&project.project_id));
    assert_eq!(workspace.label, "Chosen workspace");
    assert!(workspace.primary_directory_id.is_none());
    assert_eq!(created.terminal_contexts.len(), 2);
    let empty_context = created
        .terminal_contexts
        .iter()
        .find(|context| context.terminal_context_id == workspace.terminal_context_id)
        .ok_or("created Workspace context missing")?;
    assert!(empty_context.tabs.is_empty());
    assert!(empty_context.groups.is_empty());
    let retained_initial_workspace = created
        .terminal_contexts
        .iter()
        .find(|context| {
            context.terminal_context_id == initial.navigation.workspaces[0].terminal_context_id
        })
        .ok_or("initial_workspace context missing")?;
    assert_eq!(retained_initial_workspace, &initial.terminal_contexts[0]);

    // A shell-looking suffix stays literal. The missing parent makes it invalid even if
    // a buggy implementation tries to expand "$HOME" or otherwise choose a fallback.
    let invalid = fixture.base.join("missing directory/$HOME");
    let checkpoint = submit_directory(&mut client, invalid.to_str().ok_or("path UTF-8")?);
    wait_empty(&mut client, checkpoint, None, Some(DIRECTORY_REFUSED));
    assert!(!invalid.exists());
    assert_eq!(
        observer.snapshot()?,
        created,
        "refusal has no fallback or canonical mutation"
    );

    let directory = fixture.base.join("chosen directory with spaces");
    fs::create_dir(&directory)?;
    let directory = fs::canonicalize(directory)?;
    let checkpoint = submit_directory(&mut client, directory.to_str().ok_or("path UTF-8")?);
    wait_empty(
        &mut client,
        checkpoint,
        Some(directory.to_str().ok_or("path UTF-8")?),
        Some(DIRECTORY_ACCEPTED),
    );
    let configured = observer.snapshot()?;
    assert!(configured.revision > created.revision);
    assert_eq!(configured.session_id, created.session_id);
    assert_eq!(
        configured.terminal_contexts, created.terminal_contexts,
        "directory acceptance does not create any Workspace Tab or terminal binding"
    );
    let configured_project = &configured.navigation.projects[0];
    assert_eq!(configured.navigation.projects.len(), 1);
    assert_eq!(configured_project.project_id, project.project_id);
    assert_eq!(configured.navigation.workspaces.len(), 2);
    assert_eq!(configured.navigation.directories.len(), 2);
    let configured_workspace = &configured.navigation.workspaces[1];
    assert_eq!(configured_workspace.workspace_id, workspace.workspace_id);
    assert_eq!(
        configured_workspace.terminal_context_id,
        workspace.terminal_context_id
    );
    let accepted_directory = &configured.navigation.directories[1];
    assert_eq!(
        configured_workspace.primary_directory_id.as_ref(),
        Some(&accepted_directory.directory_id)
    );
    assert_eq!(
        accepted_directory.path.as_str(),
        directory.to_str().ok_or("path UTF-8")?
    );
    let mut expected_configured = created.clone();
    expected_configured.revision = configured.revision;
    expected_configured.navigation.directories = configured.navigation.directories.clone();
    expected_configured.navigation.workspaces[1].primary_directory_id =
        Some(accepted_directory.directory_id.clone());
    assert_eq!(
        configured, expected_configured,
        "acceptance changes only this Workspace's directory association"
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
        WorkbenchHeader::active(WORKSPACE_LOCATION, 1, "1"),
        &["Tab 1 · Ready · controlling"],
        &[EMPTY],
        &[],
        &mut [],
    );
    client.send(shell_identity_command(&fixture, "workspace", "setup-workspace", true).as_bytes());
    let workspace_shell = wait_file(
        &fixture.base.join("setup-workspace"),
        &mut [&mut client],
        Instant::now() + OBSERVATION_TIMEOUT,
    );
    assert_ne!(
        require_shell_identity(&workspace_shell, "workspace"),
        initial_workspace_pid
    );
    client.send(format!("pwd -P > {}/setup-cwd\r", fixture.base.display()).as_bytes());
    assert_eq!(
        wait_file(
            &fixture.base.join("setup-cwd"),
            &mut [&mut client],
            Instant::now() + OBSERVATION_TIMEOUT,
        )
        .trim_end(),
        directory.to_str().ok_or("path UTF-8")?,
    );
    let launched = observer.snapshot()?;
    let launched_context = launched
        .terminal_contexts
        .iter()
        .find(|context| context.terminal_context_id == workspace.terminal_context_id)
        .ok_or("launched Workspace context missing")?;
    assert_eq!(launched_context.tabs.len(), 1);
    assert!(launched.revision > configured.revision);
    let mut expected = configured.clone();
    expected.revision = launched.revision;
    expected
        .terminal_contexts
        .iter_mut()
        .find(|context| context.terminal_context_id == workspace.terminal_context_id)
        .ok_or("configured Workspace context missing")?
        .tabs = launched_context.tabs.clone();
    assert_eq!(
        launched, expected,
        "explicit c changes only the selected Workspace's Tabs"
    );

    let checkpoint = client.checkpoint();
    client.send(b"\x02[");
    client.wait_native_frame_text_after(
        checkpoint,
        Instant::now() + SWITCH_TIMEOUT,
        WorkbenchHeader::active(INITIAL_WORKSPACE_LOCATION, 1, "1"),
        &["Tab 1 · Ready · controlling"],
        &[WORKSPACE_LOCATION, EMPTY],
        &[],
        &mut [],
    );
    client.send(shell_identity_command(&fixture, "", "setup-after", false).as_bytes());
    assert_eq!(
        wait_file(
            &fixture.base.join("setup-after"),
            &mut [&mut client],
            Instant::now() + OBSERVATION_TIMEOUT,
        ),
        initial_workspace,
        "native setup retains the exact initial_workspace PID and shell-local variable",
    );
    assert_eq!(observer.snapshot()?, launched);
    client.close_before(Instant::now() + EXIT_TIMEOUT);
    Ok(())
}

#[test]
fn native_directory_setup_cancel_preserves_the_empty_workspace_and_original_shell() -> TestResult {
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
    client.send(shell_identity_command(&fixture, "cancel", "cancel-before", true).as_bytes());
    let initial_workspace = wait_file(
        &fixture.base.join("cancel-before"),
        &mut [&mut client],
        Instant::now() + OBSERVATION_TIMEOUT,
    );
    require_shell_identity(&initial_workspace, "cancel");
    seed_empty_workspace(&fixture, &mut client)?;
    let mut observer = SnapshotObserver::connect(&fixture)?;
    let committed = observer.snapshot()?;
    for path in ["", "/discarded directory/$HOME"] {
        edit_directory(&mut client, path);
        let checkpoint = client.checkpoint();
        client.send(b"\x1b");
        wait_empty(&mut client, checkpoint, None, None);
        assert_eq!(
            observer.snapshot()?,
            committed,
            "cancel submits no directory mutation"
        );
    }
    finish_modal_handoff(
        &mut client,
        WorkbenchHeader::empty(WORKSPACE_LOCATION),
        &[EMPTY],
    );
    let checkpoint = client.checkpoint();
    client.send(b"\x02[");
    client.wait_native_frame_text_after(
        checkpoint,
        Instant::now() + SWITCH_TIMEOUT,
        WorkbenchHeader::active(INITIAL_WORKSPACE_LOCATION, 1, "1"),
        &["Tab 1 · Ready · controlling"],
        &[WORKSPACE_LOCATION, EMPTY],
        &[],
        &mut [],
    );
    assert_shell_survives(&fixture, &mut client, &initial_workspace, "cancel-after");
    assert_eq!(observer.snapshot()?, committed);
    client.close_before(Instant::now() + EXIT_TIMEOUT);
    Ok(())
}
