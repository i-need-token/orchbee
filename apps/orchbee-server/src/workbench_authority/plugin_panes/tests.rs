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
fn plugin_pane_resolution_refuses_disabled_missing_platform_and_shell_wrapper() {
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
        SOURCE.replace("pane.exe", "script.cmd"),
        "invalid manifest".into(),
    ] {
        fs::write(root.0.join("orchbee-plugin.toml"), source).unwrap();
        assert!(resolve_from(&registry, &snapshot, &command, &selector).is_err());
    }
    fs::remove_file(root.0.join("orchbee-plugin.toml")).unwrap();
    assert!(resolve_from(&registry, &snapshot, &command, &selector).is_err());
}
