use super::native_workspace_fixture::*;
use super::*;

#[test]
fn native_workspace_setup_uses_literal_directory_and_only_explicitly_launches() -> TestResult {
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
    client.send(shell_identity_command(&fixture, "standalone", "setup-before", true).as_bytes());
    let standalone = wait_file(
        &fixture.base.join("setup-before"),
        &mut [&mut client],
        Instant::now() + OBSERVATION_TIMEOUT,
    );
    let standalone_pid = require_shell_identity(&standalone, "standalone");
    let mut observer = SnapshotObserver::connect(&fixture)?;
    let initial = observer.snapshot()?;
    assert!(
        initial.navigation.projects.is_empty(),
        "fresh Site has no Projects"
    );
    assert_eq!(initial.terminal_contexts.len(), 1);
    assert_eq!(initial.terminal_contexts[0].tabs.len(), 1);

    open_new_project(&mut client);
    submit_project(&mut client, "Setup process");
    let partial = observer.snapshot()?;
    assert_eq!(partial.site_id, initial.site_id);
    assert!(partial.revision > initial.revision);
    assert_eq!(partial.terminal_contexts, initial.terminal_contexts);
    assert_eq!(partial.navigation.projects.len(), 1);
    let project = &partial.navigation.projects[0];
    assert_eq!(project.label, "Setup process");
    assert!(project.workspaces.is_empty());
    assert!(project.directories.is_empty());

    let checkpoint = submit_workspace(&mut client, "Chosen workspace");
    wait_empty(&mut client, checkpoint, None);
    let created = observer.snapshot()?;
    assert_eq!(created.site_id, initial.site_id);
    assert!(created.revision > partial.revision);
    assert_eq!(created.navigation.projects.len(), 1);
    let project = &created.navigation.projects[0];
    assert_eq!(
        project.project_id,
        partial.navigation.projects[0].project_id
    );
    assert_eq!(project.workspaces.len(), 1);
    assert!(project.directories.is_empty());
    let workspace = &project.workspaces[0];
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
    let retained_standalone = created
        .terminal_contexts
        .iter()
        .find(|context| {
            context.terminal_context_id == initial.navigation.standalone_terminal_context_id
        })
        .ok_or("standalone context missing")?;
    assert_eq!(retained_standalone, &initial.terminal_contexts[0]);

    // A shell-looking suffix stays literal. The missing parent makes it invalid even if
    // a buggy implementation tries to expand "$HOME" or otherwise choose a fallback.
    let invalid = fixture.base.join("missing directory/$HOME");
    let checkpoint = submit_directory(&mut client, invalid.to_str().ok_or("path UTF-8")?);
    wait_empty(&mut client, checkpoint, Some(DIRECTORY_REFUSED));
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
    wait_empty(&mut client, checkpoint, Some(DIRECTORY_ACCEPTED));
    let configured = observer.snapshot()?;
    assert!(configured.revision > created.revision);
    assert_eq!(configured.site_id, created.site_id);
    assert_eq!(
        configured.terminal_contexts, created.terminal_contexts,
        "directory acceptance does not create any Workspace Tab or terminal binding"
    );
    let configured_project = &configured.navigation.projects[0];
    assert_eq!(configured.navigation.projects.len(), 1);
    assert_eq!(configured_project.project_id, project.project_id);
    assert_eq!(configured_project.workspaces.len(), 1);
    assert_eq!(configured_project.directories.len(), 1);
    let configured_workspace = &configured_project.workspaces[0];
    assert_eq!(configured_workspace.workspace_id, workspace.workspace_id);
    assert_eq!(
        configured_workspace.terminal_context_id,
        workspace.terminal_context_id
    );
    let accepted_directory = &configured_project.directories[0];
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
    expected_configured.navigation.projects[0].directories = configured_project.directories.clone();
    expected_configured.navigation.projects[0].workspaces[0].primary_directory_id =
        Some(accepted_directory.directory_id.clone());
    assert_eq!(
        configured, expected_configured,
        "acceptance changes only this Workspace's directory association"
    );

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
    client.send(shell_identity_command(&fixture, "workspace", "setup-workspace", true).as_bytes());
    let workspace_shell = wait_file(
        &fixture.base.join("setup-workspace"),
        &mut [&mut client],
        Instant::now() + OBSERVATION_TIMEOUT,
    );
    assert_ne!(
        require_shell_identity(&workspace_shell, "workspace"),
        standalone_pid
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
    client.wait_frame_text_after(
        checkpoint,
        Instant::now() + SWITCH_TIMEOUT,
        &[STANDALONE_HEADER, "[*1 1]", "Tab 1 · Ready · controlling"],
        &[WORKSPACE_HEADER, EMPTY],
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
        standalone,
        "native setup retains the exact standalone PID and shell-local variable",
    );
    assert_eq!(observer.snapshot()?, launched);
    client.close_before(Instant::now() + EXIT_TIMEOUT);
    Ok(())
}

#[test]
fn native_workspace_setup_cancel_keeps_the_already_committed_project() -> TestResult {
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
    client.send(shell_identity_command(&fixture, "cancel", "cancel-before", true).as_bytes());
    let standalone = wait_file(
        &fixture.base.join("cancel-before"),
        &mut [&mut client],
        Instant::now() + OBSERVATION_TIMEOUT,
    );
    require_shell_identity(&standalone, "cancel");
    let mut observer = SnapshotObserver::connect(&fixture)?;
    let initial = observer.snapshot()?;
    assert!(initial.navigation.projects.is_empty());
    open_new_project(&mut client);
    submit_project(&mut client, "Retained project");
    let committed = observer.snapshot()?;
    assert!(committed.revision > initial.revision);
    assert_eq!(committed.terminal_contexts, initial.terminal_contexts);
    assert_eq!(committed.navigation.projects.len(), 1);
    assert_eq!(committed.navigation.projects[0].label, "Retained project");
    assert!(committed.navigation.projects[0].workspaces.is_empty());
    assert!(committed.navigation.projects[0].directories.is_empty());

    let checkpoint = client.checkpoint();
    client.send(b"\x1b");
    client.wait_frame_text_after(
        checkpoint,
        Instant::now() + SWITCH_TIMEOUT,
        &[STANDALONE_HEADER, "[*1 1]", PROJECT_CREATED],
        &[WORKSPACE_NAME],
        &[],
        &mut [],
    );
    assert_eq!(
        observer.snapshot()?,
        committed,
        "cancel must retain the known Project commit"
    );
    finish_modal_handoff(&mut client, &[STANDALONE_HEADER, "[*1 1]", PROJECT_CREATED]);
    // The retained commit notice legitimately occupies the footer after cancellation.
    // Prove control with actual shell execution instead of its clipped sidebar label.
    assert_shell_survives(&fixture, &mut client, &standalone, "cancel-after");

    // Reopening sees the retained empty Project, but standalone still defaults to New Project.
    let checkpoint = client.checkpoint();
    client.send(b"\x02a");
    modal(
        &mut client,
        checkpoint,
        STANDALONE_HEADER,
        &[
            CHOOSE_PROJECT,
            "",
            "1. Retained project",
            "> 2. New Project",
            "",
            "[ / ] · choose Project    Enter · continue    Esc · cancel",
        ],
    );
    let checkpoint = client.checkpoint();
    client.send(b"[");
    modal(
        &mut client,
        checkpoint,
        STANDALONE_HEADER,
        &[
            CHOOSE_PROJECT,
            "",
            "> 1. Retained project",
            "2. New Project",
            "",
            "[ / ] · choose Project    Enter · continue    Esc · cancel",
        ],
    );
    let checkpoint = client.checkpoint();
    client.send(b"\r");
    modal(
        &mut client,
        checkpoint,
        STANDALONE_HEADER,
        &[
            WORKSPACE_NAME,
            "",
            "Project: Retained project",
            "",
            "",
            "",
            LABEL_HINT,
            "",
            TEXT_ACTIONS,
        ],
    );
    assert_eq!(
        observer.snapshot()?,
        committed,
        "selecting an existing Project does not create one"
    );
    finish_modal_handoff(&mut client, &[STANDALONE_HEADER, WORKSPACE_NAME]);
    let checkpoint = client.checkpoint();
    client.send(b"\x1b");
    client.wait_frame_text_after(
        checkpoint,
        Instant::now() + SWITCH_TIMEOUT,
        &[STANDALONE_HEADER, "[*1 1]", "Tab 1 · Ready · controlling"],
        &[WORKSPACE_NAME],
        &[],
        &mut [],
    );
    finish_modal_handoff(
        &mut client,
        &[STANDALONE_HEADER, "Tab 1 · Ready · controlling"],
    );
    assert_eq!(observer.snapshot()?, committed);
    client.close_before(Instant::now() + EXIT_TIMEOUT);
    Ok(())
}
