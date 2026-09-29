use super::*;
use orchbee_local_site_runtime::{PluginRegistry, PluginRegistryMutation};
use orchbee_workbench_protocol::{
    LaunchOptions, PluginInvocationContext, ProtocolVersion, SiteId, WorkbenchAction,
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
    let authority =
        super::super::WorkbenchAuthority::new(SiteId::parse(&format!("{:032x}", 1)).unwrap())
            .unwrap();
    let snapshot = authority.projection.snapshot().clone();
    let selector = PluginPaneSelector::new("tools".into(), "dashboard".into()).unwrap();
    let command = WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{:032x}", 2)).unwrap(),
        site_id: snapshot.site_id.clone(),
        expected_revision: WorkbenchRevision::new(0),
        action: WorkbenchAction::TabCreate {
            automatic_label: false,
            terminal_context_id: snapshot.navigation.standalone_terminal_context_id.clone(),
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
    assert!(
        context.workspace_id.is_none()
            && context.tab_id.is_none()
            && context.focused_pane_id.is_none()
    );
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
    let result = orchbee_local_site_runtime::run_windows_command(
        process,
        Some(std::time::Instant::now() + std::time::Duration::from_secs(30)),
        || Ok(false),
    );
    if result.failure == Some(orchbee_local_site_runtime::WindowsCommandFailure::PhysicalUnknown) {
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
    let site = SiteId::parse(&id(100)).unwrap();
    let mut authority = WorkbenchAuthority::new(site.clone()).unwrap();
    let grant = WorkbenchGrant::manage_navigation_and_terminal_create(site);
    let command: WorkbenchCommand = serde_json::from_value(json!({
        "protocolVersion":1,"requestId":id(200),"siteId":id(100),"expectedRevision":"0",
        "action":{"kind":"tab.create","terminalContextId":id(1),"label":"Dashboard",
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
        let WorkbenchTabCreatePreparation::Prepared(prepared) = admission.finish(binding.clone())
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
