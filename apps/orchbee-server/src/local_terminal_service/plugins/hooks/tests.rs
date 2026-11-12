use super::*;
use orchbee_local_runtime::PluginRegistryMutation;
use orchbee_workbench_protocol::{
    SemanticEventPosition, WorkbenchRequestId, WorkbenchRevision, WorkbenchTabId,
    WorkbenchTerminalContextId, WorkbenchWorkspaceId,
};
use std::{collections::BTreeSet, fs, path::PathBuf};

struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let mut nonce = [0; 16];
        getrandom::fill(&mut nonce).unwrap();
        let path = std::env::temp_dir().join(format!(
            "orchbee-plugin-hooks-{:032x}",
            u128::from_be_bytes(nonce)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn link(&self, registry: &PluginRegistry, name: &str, manifest: &str, enabled: bool) {
        let directory = self.0.join(name);
        fs::create_dir(&directory).unwrap();
        fs::write(directory.join("orchbee-plugin.toml"), manifest).unwrap();
        registry
            .mutate(PluginRegistryMutation::Link {
                path: directory,
                enabled,
            })
            .unwrap();
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
                .starts_with("orchbee-plugin-hooks-")
        );
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[derive(Default)]
struct FakeJobs {
    next: u128,
    started: Vec<(node::OperationId, node::PluginCommandIntent)>,
    finished: BTreeSet<node::OperationId>,
}
impl Jobs for FakeJobs {
    fn allocate(&mut self) -> Option<node::OperationId> {
        self.next += 1;
        node::OperationId::from_bytes(self.next.to_be_bytes()).ok()
    }
    fn start(&mut self, id: node::OperationId, intent: node::PluginCommandIntent) -> Admission {
        self.started.push((id, intent));
        Admission::Started
    }
    fn running(&mut self, id: node::OperationId) -> bool {
        !self.finished.contains(&id)
    }
    fn stopping(&self) -> bool {
        false
    }
}
impl FakeJobs {
    fn finish_all(&mut self) {
        self.finished.extend(self.started.iter().map(|(id, _)| *id));
    }
}

fn variable<'a>(intent: &'a node::PluginCommandIntent, name: &str) -> Option<&'a str> {
    intent
        .environment
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
}

fn tab_created(sequence: u64) -> SemanticEvent {
    SemanticEvent {
        workspace_id: Some(WorkbenchWorkspaceId::parse(&format!("{:032x}", 1)).unwrap()),
        position: SemanticEventPosition::Journal {
            cursor: SemanticEventCursor {
                epoch: WorkbenchRequestId::parse(&format!("{:032x}", 3)).unwrap(),
                sequence: WorkbenchRevision::new(sequence),
            },
        },
        workbench_revision: WorkbenchRevision::new(sequence),
        event: Kind::TabCreated,
        data: Data::Tab {
            terminal_context_id: WorkbenchTerminalContextId::parse(&format!("{:032x}", 1)).unwrap(),
            tab_id: WorkbenchTabId::parse(&format!("{:032x}", 4)).unwrap(),
            label: "Build".into(),
            index: 0,
        },
    }
}

const TOOLS: &str = "id='tools'\nname='Tools'\nversion='1'\n\
[[startup]]\ncommand=['cmd.exe','/d','/c','echo ready']\n\
[[startup]]\nplatforms=['linux']\ncommand=['true']\n\
[[events]]\non='tab.created'\ncommand=['cmd.exe','/d','/c','echo tab']\n\
[[events]]\non='tab.created'\ncommand=['cmd.exe','/d','/c','echo again']\n\
[[events]]\non='panel.created'\ncommand=['cmd.exe','/d','/c','echo pane']\n\
[[events]]\non='panel.output_changed'\ncommand=['cmd.exe','/d','/c','echo output']\n";

#[test]
fn plugin_startup_runs_each_windows_entry_of_enabled_plugins_once() {
    let root = Root::new();
    let registry = PluginRegistry::at(root.0.join("catalog")).unwrap();
    root.link(&registry, "tools", TOOLS, true);
    root.link(
        &registry,
        "off",
        &TOOLS.replace("id='tools'", "id='off'"),
        false,
    );
    let mut jobs = FakeJobs::default();
    Dispatcher::default().startup(&registry, &mut jobs);
    assert_eq!(jobs.started.len(), 1);
    let (id, intent) = &jobs.started[0];
    assert_eq!(intent.plugin_id, "tools");
    assert_eq!(intent.command_id, STARTUP);
    assert_eq!(intent.argv, ["cmd.exe", "/d", "/c", "echo ready"]);
    assert_eq!(variable(intent, "ORCHBEE_PLUGIN_EVENT"), Some(STARTUP));
    assert_eq!(variable(intent, "ORCHBEE_PLUGIN_EVENT_JSON"), None);
    let context: PluginInvocationContext =
        serde_json::from_str(variable(intent, "ORCHBEE_PLUGIN_CONTEXT_JSON").unwrap()).unwrap();
    assert_eq!(context.invocation_source.as_deref(), Some(STARTUP));
    assert_eq!(context.correlation_id, Some(hex(id.as_bytes())));
}

#[test]
fn plugin_event_hooks_run_once_per_occurrence_and_drop_overlaps_without_queueing() {
    let root = Root::new();
    let registry = PluginRegistry::at(root.0.join("catalog")).unwrap();
    root.link(&registry, "tools", TOOLS, true);
    let mut hooks = Dispatcher::default();
    let mut jobs = FakeJobs::default();
    hooks.events(&registry, &[tab_created(1)], &mut jobs);
    assert_eq!(jobs.started.len(), 2);
    for (_, intent) in &jobs.started {
        assert_eq!(intent.command_id, "tab.created");
        assert_eq!(
            variable(intent, "ORCHBEE_PLUGIN_EVENT"),
            Some("tab.created")
        );
        assert_eq!(
            variable(intent, "ORCHBEE_TAB_ID"),
            Some(format!("{:032x}", 4).as_str())
        );
        let payload: serde_json::Value =
            serde_json::from_str(variable(intent, "ORCHBEE_PLUGIN_EVENT_JSON").unwrap()).unwrap();
        assert_eq!(
            serde_json::from_value::<SemanticEvent>(payload).unwrap(),
            tab_created(1)
        );
    }
    // A later occurrence while either hook runs is dropped, and nothing is kept for later.
    hooks.events(&registry, &[tab_created(2), tab_created(3)], &mut jobs);
    assert_eq!(jobs.started.len(), 2);
    jobs.finish_all();
    hooks.events(&registry, &[tab_created(4)], &mut jobs);
    assert_eq!(jobs.started.len(), 4);
    // Disabling the plugin stops its hooks.
    registry
        .mutate(PluginRegistryMutation::SetEnabled {
            id: "tools".into(),
            enabled: false,
        })
        .unwrap();
    jobs.finish_all();
    hooks.events(&registry, &[tab_created(5)], &mut jobs);
    assert_eq!(jobs.started.len(), 4);
}

#[test]
fn plugin_hooks_match_the_event_name_and_never_hook_undelivered_events() {
    let root = Root::new();
    let registry = PluginRegistry::at(root.0.join("catalog")).unwrap();
    root.link(&registry, "tools", TOOLS, true);
    let entry = &registry.list(None).unwrap().plugins[0];
    assert_eq!(hook_commands(entry, Kind::PaneCreated).len(), 1);
    assert!(hook_commands(entry, Kind::PaneOutputChanged).is_empty());
}
