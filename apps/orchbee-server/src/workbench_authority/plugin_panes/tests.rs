use super::*;
use crate::test_support::WorkbenchFixture;
use orchbee_local_runtime::{PluginRegistry, PluginRegistryMutation};
use orchbee_workbench_protocol::{
    LaunchOptions, PluginInvocationContext, ProtocolVersion, SessionId, WorkbenchAction,
    WorkbenchLabel, WorkbenchRequestId, WorkbenchRevision, WorkbenchTerminalGeometry,
};
use std::{fs, path::PathBuf};

struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let mut nonce = [0; 16];
        getrandom::fill(&mut nonce).unwrap();
        let root = std::env::temp_dir().join(format!(
            "orchbee-plugin-pane-{:032x}",
            u128::from_be_bytes(nonce)
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        assert!(self.0.starts_with(std::env::temp_dir()));
        assert!(
            self.0
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("orchbee-plugin-pane-")
        );
        fs::remove_dir_all(&self.0).unwrap();
    }
}
const SOURCE: &str = "id='tools'\nname='Tools'\nversion='1'\n[[panes]]\nid='dashboard'\ntitle='Dashboard'\nplacement='tab'\ncommand=['pane.exe','hello world','&literal']\n";
fn fixture() -> (
    Root,
    PluginRegistry,
    Snapshot,
    WorkbenchCommand,
    PluginPaneSelector,
) {
    let root = Root::new();
    let path = root.0.join("orchbee-plugin.toml");
    fs::write(&path, SOURCE).unwrap();
    fs::write(
        root.0.join("pane.exe"),
        b"not executed by this resolver test",
    )
    .unwrap();
    let registry = PluginRegistry::at(root.0.join("catalog")).unwrap();
    registry
        .mutate(PluginRegistryMutation::Link {
            path,
            enabled: true,
        })
        .unwrap();
    let authority = super::super::WorkbenchAuthority::workspace_fixture(
        SessionId::parse(&format!("{:032x}", 1)).unwrap(),
    )
    .unwrap();
    let snapshot = authority.projection.snapshot().clone();
    let selector = PluginPaneSelector::new("tools".into(), "dashboard".into()).unwrap();
    let command = WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{:032x}", 2)).unwrap(),
        session_id: snapshot.session_id.clone(),
        expected_revision: WorkbenchRevision::new(0),
        action: WorkbenchAction::TabCreate {
            placement: None,
            automatic_label: false,
            terminal_context_id: snapshot.navigation.workspaces[0]
                .terminal_context_id
                .clone(),
            label: WorkbenchLabel::parse("Dashboard").unwrap(),
            geometry: WorkbenchTerminalGeometry::new(80, 24, 0, 0).unwrap(),
            launch: None,
            profile: None,
            options: Box::new(LaunchOptions {
                plugin_pane: Some(selector.clone()),
                ..Default::default()
            }),
        },
    };
    (root, registry, snapshot, command, selector)
}

#[test]
fn plugin_pane_resolution_freezes_root_relative_argv_and_protected_target_context() {
    let (root, registry, snapshot, mut command, selector) = fixture();
    let WorkbenchAction::TabCreate { options, .. } = &mut command.action else {
        panic!()
    };
    options.cwd = Some(
        orchbee_workbench_protocol::WorkbenchDirectoryPath::parse("C:/different-cwd").unwrap(),
    );
    let frozen = resolve_from(&registry, &snapshot, &command, &selector).unwrap();
    assert_eq!(
        PathBuf::from(frozen.launch.executable()),
        fs::canonicalize(&root.0).unwrap().join("pane.exe")
    );
    assert_eq!(frozen.launch.arguments(), &["hello world", "&literal"]);
    assert_eq!(
        PathBuf::from(frozen.directory.as_str()),
        fs::canonicalize(&root.0).unwrap()
    );
    let value = |key| {
        &frozen
            .environment
            .entries()
            .iter()
            .find(|(name, _)| name == key)
            .unwrap()
            .1
    };
    assert_eq!(value("ORCHBEE_PLUGIN_ENTRYPOINT_ID"), "dashboard");
    let context: PluginInvocationContext =
        serde_json::from_str(value("ORCHBEE_PLUGIN_CONTEXT_JSON")).unwrap();
    assert_eq!(
        context.correlation_id.as_deref(),
        Some(command.request_id.as_str())
    );
    // A new Tab has no Tab or Pane yet; its target context belongs to the fixture Workspace.
    assert_eq!(
        context.workspace_id,
        Some(snapshot.navigation.workspaces[0].workspace_id.to_string())
    );
    assert!(context.tab_id.is_none() && context.focused_pane_id.is_none());
    assert!(
        !frozen
            .environment
            .entries()
            .iter()
            .any(|(key, _)| key == "ORCHBEE_TERMINAL_ID")
    );
    fs::write(
        root.0.join("orchbee-plugin.toml"),
        SOURCE.replace("hello world", "changed"),
    )
    .unwrap();
    assert_eq!(frozen.launch.arguments()[0], "hello world");
    assert_eq!(
        resolve_from(&registry, &snapshot, &command, &selector)
            .unwrap()
            .launch
            .arguments()[0],
        "changed"
    );
}

#[test]
fn plugin_pane_resolution_refuses_disabled_missing_and_incompatible_platform() {
    let (root, registry, snapshot, command, selector) = fixture();
    registry
        .mutate(PluginRegistryMutation::SetEnabled {
            id: "tools".into(),
            enabled: false,
        })
        .unwrap();
    assert!(resolve_from(&registry, &snapshot, &command, &selector).is_err());
    registry
        .mutate(PluginRegistryMutation::SetEnabled {
            id: "tools".into(),
            enabled: true,
        })
        .unwrap();
    for source in [
        SOURCE.replace("placement='tab'", "platforms=['linux']"),
        "invalid manifest".into(),
    ] {
        fs::write(root.0.join("orchbee-plugin.toml"), source).unwrap();
        assert!(resolve_from(&registry, &snapshot, &command, &selector).is_err());
    }
    for script in ["script.cmd", "script.BAT"] {
        fs::write(root.0.join(script), b"@exit /B 0").unwrap();
        fs::write(
            root.0.join("orchbee-plugin.toml"),
            SOURCE.replace("pane.exe", script),
        )
        .unwrap();
        let batch = resolve_from(&registry, &snapshot, &command, &selector).unwrap();
        assert_eq!(
            PathBuf::from(batch.launch.executable()),
            fs::canonicalize(&root.0).unwrap().join(script)
        );
        assert_eq!(batch.launch.arguments(), &["hello world", "&literal"]);
    }
    fs::remove_file(root.0.join("orchbee-plugin.toml")).unwrap();
    assert!(resolve_from(&registry, &snapshot, &command, &selector).is_err());
}

#[test]
fn plugin_pane_admission_publication_replay_and_membership_lifetime() {
    const CHILD: &str = "ORCHBEE_TEST_PLUGIN_PANE_ADMISSION";
    if let Some(root) = std::env::var_os(CHILD) {
        admission_witness(PathBuf::from(root));
        return;
    }
    let root = Root::new();
    fs::write(root.0.join("orchbee-plugin.toml"), SOURCE).unwrap();
    fs::write(root.0.join("pane.exe"), b"not launched in admission test").unwrap();
    let mut process = std::process::Command::new(std::env::current_exe().unwrap());
    process.args(["--exact", "workbench_authority::plugin_panes::tests::plugin_pane_admission_publication_replay_and_membership_lifetime", "--nocapture"])
        .env(CHILD, &root.0).env("APPDATA", root.0.join("appdata")).env_remove("ORCHBEE_CONFIG_FILE");
    let result = orchbee_local_runtime::run_windows_command(
        process,
        Some(std::time::Instant::now() + std::time::Duration::from_secs(30)),
        || Ok(false),
    );
    if result.failure == Some(orchbee_local_runtime::CommandFailure::PhysicalUnknown) {
        eprintln!(
            "Root agent owns retained admission fixture pending child settlement: {}",
            root.0.display()
        );
        std::mem::forget(root);
    }
    assert_eq!(
        result.failure,
        None,
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

fn admission_witness(root: PathBuf) {
    use crate::{
        WorkbenchAuthority, WorkbenchGrant, WorkbenchTabCreateBegin, WorkbenchTabCreatePreparation,
    };
    use orchbee_workbench_protocol::{
        PanelNode, PluginOutcome, PresentationBinding, TerminalId, WorkbenchCommandOutcome,
        WorkbenchProtocolDocument, WorkerId,
    };
    use serde_json::json;
    let registry = PluginRegistry::from_environment(|name| std::env::var_os(name)).unwrap();
    registry
        .mutate(PluginRegistryMutation::Link {
            path: root.join("orchbee-plugin.toml"),
            enabled: true,
        })
        .unwrap();
    let id = |value: u64| format!("{value:032x}");
    let session = SessionId::parse(&id(100)).unwrap();
    let mut authority = WorkbenchAuthority::workspace_fixture(session.clone()).unwrap();
    let grant = WorkbenchGrant::manage_navigation_and_terminal_create(session);
    let command: WorkbenchCommand = serde_json::from_value(json!({
        "protocolVersion":1,"requestId":id(200),"sessionId":id(100),"expectedRevision":"0",
        "action":{"kind":"tab.create","terminalContextId":authority.fixture_context_id(),"label":"Dashboard",
        "geometry":{"columns":80,"rows":24,"cellPixelWidth":0,"cellPixelHeight":0},
        "options":{"pluginPane":{"pluginId":"tools","entrypointId":"dashboard"}}}
    }))
    .unwrap();
    assert!(
        matches!(authority.begin_terminal_tab_create(None, command.clone()), WorkbenchTabCreateBegin::Ready(result) if matches!(result.outcome, WorkbenchCommandOutcome::Denied {}))
    );
    let binding = PresentationBinding {
        worker_id: WorkerId::parse(&id(300)).unwrap(),
        terminal_id: TerminalId::parse(&id(301)).unwrap(),
    };
    let mut created = None;
    for commit in [false, true] {
        let WorkbenchTabCreateBegin::Admitted(admission) =
            authority.begin_terminal_tab_create(Some(&grant), command.clone())
        else {
            panic!("admitted")
        };
        assert_eq!(
            admission.plugin_use().unwrap().launch.arguments()[0],
            "hello world"
        );
        let WorkbenchTabCreatePreparation::Prepared(prepared) =
            admission.finish(binding.clone(), None)
        else {
            panic!("prepared")
        };
        let metadata = prepared.metadata().clone();
        if commit {
            let committed = prepared.commit();
            created = Some((metadata, committed.into_pending()));
        } else {
            drop(prepared);
            assert_eq!(authority.projection.snapshot().revision.value(), 0);
            assert!(matches!(
                authority.plugin_pane(metadata.panel_id()),
                PluginOutcome::Failed { .. }
            ));
        }
    }
    let (metadata, _pending) = created.unwrap();
    assert!(
        matches!(authority.plugin_pane(metadata.panel_id()), PluginOutcome::PaneFound { revision, pane } if revision.value() == 1 && pane.binding == binding)
    );
    registry
        .mutate(PluginRegistryMutation::SetEnabled {
            id: "tools".into(),
            enabled: false,
        })
        .unwrap();
    assert!(
        matches!(authority.begin_terminal_tab_create(Some(&grant), command), WorkbenchTabCreateBegin::Ready(result) if matches!(result.outcome, WorkbenchCommandOutcome::TabCreatePending { .. }))
    );
    let mut snapshot = authority.projection.snapshot().clone();
    let PanelNode::Panel { panel_id, .. } = &mut snapshot.terminal_contexts[0].tabs[0].root else {
        panic!()
    };
    *panel_id = orchbee_workbench_protocol::WorkbenchPanelId::parse(&id(999)).unwrap();
    let moved = panel_id.clone();
    snapshot.revision = WorkbenchRevision::new(2);
    let install = |authority: &mut WorkbenchAuthority, snapshot| {
        authority.projection = crate::workbench_projection::WorkbenchProjectionStore::load(
            &orchbee_workbench_protocol::serialize_document(&WorkbenchProtocolDocument::Snapshot(
                snapshot,
            ))
            .unwrap(),
        )
        .unwrap();
    };
    install(&mut authority, snapshot.clone());
    assert!(matches!(
        authority.plugin_pane(metadata.panel_id()),
        PluginOutcome::Failed { .. }
    ));
    assert!(matches!(
        authority.plugin_pane(&moved),
        PluginOutcome::PaneFound { .. }
    ));
    let PanelNode::Panel {
        primary_presentation,
        ..
    } = &mut snapshot.terminal_contexts[0].tabs[0].root
    else {
        panic!()
    };
    primary_presentation.binding.as_mut().unwrap().terminal_id =
        TerminalId::parse(&id(302)).unwrap();
    snapshot.revision = WorkbenchRevision::new(3);
    install(&mut authority, snapshot);
    assert!(matches!(
        authority.plugin_pane(&moved),
        PluginOutcome::Failed { .. }
    ));
}

fn source_fixture(snapshot: &mut Snapshot) -> PluginInvocationContext {
    use serde_json::json;
    let id = |n: u64| format!("{n:032x}");
    snapshot
        .navigation
        .projects
        .push(serde_json::from_value(json!({"projectId":id(2),"label":"project"})).unwrap());
    snapshot.navigation.workspaces = serde_json::from_value(json!([
        {"workspaceId":id(3),"projectId":id(2),"terminalContextId":id(4),"label":"canonical workspace","primaryDirectoryId":null},
        {"workspaceId":id(12),"projectId":id(2),"terminalContextId":id(11),"label":"other","primaryDirectoryId":null}
    ])).unwrap();
    snapshot.navigation.workspace_order = snapshot
        .navigation
        .workspaces
        .iter()
        .map(|workspace| workspace.workspace_id.clone())
        .collect();
    snapshot.terminal_contexts.push(serde_json::from_value(json!({"terminalContextId":id(4),"groups":[],"tabs":[{"tabId":id(5),"label":"canonical tab","pinned":false,"groupId":null,"root":{"kind":"panel","panelId":id(6),"primaryPresentation":{"customLabel":null,"presentationId":id(7),"label":"Agent","binding":{"workerId":id(8),"terminalId":id(9)}},"childPresentations":[]}}]})).unwrap());
    snapshot.terminal_contexts.push(
        serde_json::from_value(json!({"terminalContextId":id(11),"groups":[],"tabs":[]})).unwrap(),
    );
    orchbee_workbench_protocol::serialize_document(
        &orchbee_workbench_protocol::WorkbenchProtocolDocument::Snapshot(snapshot.clone()),
    )
    .unwrap();
    serde_json::from_value(json!({"workspaceId":id(3),"workspaceLabel":"forged workspace","workspaceCwd":"C:/observed-child","tabId":id(5),"tabLabel":"forged tab","focusedPaneId":id(6),"focusedPaneCwd":"C:/observed-child","focusedPaneAgent":"codex","focusedPaneStatus":"blocked","selectedText":"source selection","invocationSource":"forged source","correlationId":"forged correlation","worktree":{"workerId":id(8),"repositoryRoot":"C:/repo","commonDirectory":"C:/repo/.git","repositoryName":"repo","checkoutPath":"C:/source","linked":true}})).unwrap()
}
fn source_options(command: &mut WorkbenchCommand) -> &mut LaunchOptions {
    let WorkbenchAction::TabCreate { options, .. } = &mut command.action else {
        panic!()
    };
    options
}
fn frozen_context(frozen: &PluginPaneLaunch) -> PluginInvocationContext {
    serde_json::from_str(
        &frozen
            .environment
            .entries()
            .iter()
            .find(|(key, _)| key == "ORCHBEE_PLUGIN_CONTEXT_JSON")
            .unwrap()
            .1,
    )
    .unwrap()
}

#[test]
fn plugin_pane_source_context_freezes_canonical_source_separately_from_destination_and_program() {
    let (root, registry, mut snapshot, mut command, selector) = fixture();
    let context = source_fixture(&mut snapshot);
    source_options(&mut command).plugin_context = Some(Box::new(context.clone()));
    let frozen = resolve_from(&registry, &snapshot, &command, &selector).unwrap();
    let observed = frozen_context(&frozen);
    assert_eq!(observed.workspace_id, context.workspace_id);
    assert_eq!(observed.tab_id, context.tab_id);
    assert_eq!(
        observed.workspace_label.as_deref(),
        Some("canonical workspace")
    );
    assert_eq!(observed.tab_label.as_deref(), Some("canonical tab"));
    assert_eq!(observed.focused_pane_cwd, context.focused_pane_cwd);
    assert_eq!(observed.focused_pane_agent, context.focused_pane_agent);
    assert_eq!(observed.focused_pane_status, context.focused_pane_status);
    assert_eq!(observed.worktree, context.worktree);
    assert_eq!(observed.invocation_source.as_deref(), Some("plugin-pane"));
    assert_eq!(
        observed.correlation_id.as_deref(),
        Some(command.request_id.as_str())
    );
    assert_eq!(
        PathBuf::from(frozen.directory.as_str()),
        fs::canonicalize(&root.0).unwrap()
    );
    assert_eq!(
        PathBuf::from(frozen.launch.executable()),
        fs::canonicalize(&root.0).unwrap().join("pane.exe")
    );
    for (key, expected) in [
        (
            "ORCHBEE_WORKSPACE_ID",
            context.workspace_id.as_ref().unwrap(),
        ),
        ("ORCHBEE_TAB_ID", context.tab_id.as_ref().unwrap()),
        ("ORCHBEE_PANE_ID", context.focused_pane_id.as_ref().unwrap()),
    ] {
        assert_eq!(
            &frozen
                .environment
                .entries()
                .iter()
                .find(|(name, _)| name == key)
                .unwrap()
                .1,
            expected
        );
    }
    fs::write(
        root.0.join("orchbee-plugin.toml"),
        SOURCE.replace("hello world", "changed"),
    )
    .unwrap();
    source_options(&mut command)
        .plugin_context
        .as_mut()
        .unwrap()
        .focused_pane_cwd = Some("C:/changed".into());
    assert_eq!(frozen_context(&frozen), observed);
    assert_eq!(frozen.launch.arguments()[0], "hello world");
}

#[test]
fn plugin_pane_source_context_refuses_missing_or_mismatched_source_before_user_storage() {
    for case in [
        "workspace",
        "tab",
        "parent",
        "panel",
        "no-tab",
        "no-panel",
        "orphan-worktree",
        "bounds",
        "canonical-bounds",
    ] {
        let (root, registry, mut snapshot, mut command, selector) = fixture();
        let mut context = source_fixture(&mut snapshot);
        // Linking already creates user storage. Remove only this fixture's owned
        // directories so refusal can witness that launch does not recreate them.
        for suffix in ["catalog/config", "catalog/state"] {
            let target = root.0.join(suffix);
            assert!(target.starts_with(&root.0));
            fs::remove_dir_all(target).unwrap();
        }
        match case {
            "workspace" => context.workspace_id = Some(format!("{:032x}", 99)),
            "tab" => context.tab_id = Some(format!("{:032x}", 99)),
            "parent" => context.workspace_id = Some(format!("{:032x}", 12)),
            "panel" => context.focused_pane_id = Some(format!("{:032x}", 99)),
            "no-tab" => context.tab_id = None,
            "no-panel" => context.focused_pane_id = None,
            "orphan-worktree" => context.workspace_id = None,
            "bounds" => context.focused_pane_agent = Some("界".repeat(1366)),
            "canonical-bounds" => {
                context.selected_text = Some("s".repeat(8192));
                context.focused_pane_cwd = Some("c".repeat(4096));
                snapshot.navigation.workspaces[0].label = "🐝".repeat(512);
                snapshot.terminal_contexts[1].tabs[0].label = "🐝".repeat(512);
                assert!(context.validate().is_ok());
            }
            _ => unreachable!(),
        }
        source_options(&mut command).plugin_context = Some(Box::new(context));
        assert!(
            resolve_from(&registry, &snapshot, &command, &selector).is_err(),
            "{case}"
        );
        assert!(!root.0.join("catalog/config").exists(), "{case}");
        assert!(!root.0.join("catalog/state").exists(), "{case}");
    }
}

#[test]
fn plugin_pane_empty_source_context_never_fabricates_destination_as_source() {
    let (_root, registry, mut snapshot, mut command, selector) = fixture();
    source_fixture(&mut snapshot);
    let WorkbenchAction::TabCreate {
        terminal_context_id,
        ..
    } = &mut command.action
    else {
        panic!()
    };
    *terminal_context_id = snapshot.terminal_contexts[1].terminal_context_id.clone();
    source_options(&mut command).plugin_context = Some(Box::default());
    let context = frozen_context(&resolve_from(&registry, &snapshot, &command, &selector).unwrap());
    assert!(
        context.workspace_id.is_none()
            && context.workspace_label.is_none()
            && context.workspace_cwd.is_none()
            && context.tab_id.is_none()
            && context.focused_pane_id.is_none()
            && context.selected_text.is_none()
    );
    assert_eq!(context.invocation_source.as_deref(), Some("plugin-pane"));
    source_options(&mut command).plugin_context = None;
    let legacy = frozen_context(&resolve_from(&registry, &snapshot, &command, &selector).unwrap());
    assert_eq!(
        legacy.workspace_id.as_deref(),
        Some(format!("{:032x}", 3).as_str())
    );
    assert_eq!(legacy.invocation_source.as_deref(), Some("cli"));
}
