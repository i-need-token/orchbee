//! Resolves catalog selectors before the existing terminal reservation boundary.
use orchbee_client_server_protocol::{
    LaunchSelection, PluginTerminalEnvironment, WorkingDirectory,
};
use orchbee_workbench_protocol::{
    PluginPaneSelector, Snapshot, WorkbenchCommand, WorkbenchCommandUnavailableReason,
};

/// Frozen execution facts; display environment never supplies terminal authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginPaneLaunch {
    pub selector: PluginPaneSelector,
    pub launch: LaunchSelection,
    pub directory: WorkingDirectory,
    pub environment: PluginTerminalEnvironment,
}

pub(super) fn resolve(
    snapshot: &Snapshot,
    command: &WorkbenchCommand,
    selector: &PluginPaneSelector,
) -> Result<PluginPaneLaunch, WorkbenchCommandUnavailableReason> {
    #[cfg(windows)]
    {
        let registry = orchbee_local_site_runtime::PluginRegistry::from_environment(|name| {
            std::env::var_os(name)
        })
        .map_err(|_| WorkbenchCommandUnavailableReason::ServiceUnavailable)?;
        resolve_from(&registry, snapshot, command, selector)
    }
    #[cfg(not(windows))]
    {
        let _ = (snapshot, command, selector);
        Err(WorkbenchCommandUnavailableReason::NoCompatibleWorker)
    }
}

#[cfg(windows)]
fn resolve_from(
    registry: &orchbee_local_site_runtime::PluginRegistry,
    snapshot: &Snapshot,
    command: &WorkbenchCommand,
    selector: &PluginPaneSelector,
) -> Result<PluginPaneLaunch, WorkbenchCommandUnavailableReason> {
    use orchbee_workbench_protocol::{PluginInvocationContext, WorkbenchAction};
    let unavailable = WorkbenchCommandUnavailableReason::ServiceUnavailable;
    let (context_id, tab_id, panel_id) = match &command.action {
        WorkbenchAction::TabCreate {
            terminal_context_id,
            ..
        } => (terminal_context_id, None, None),
        WorkbenchAction::PanelSplit {
            terminal_context_id,
            tab_id,
            panel_id,
            ..
        } => (terminal_context_id, Some(tab_id), Some(panel_id)),
        _ => return Err(unavailable),
    };
    let mut context = PluginInvocationContext {
        invocation_source: Some("cli".into()),
        correlation_id: Some(command.request_id.to_string()),
        tab_id: tab_id.map(ToString::to_string),
        focused_pane_id: panel_id.map(ToString::to_string),
        ..Default::default()
    };
    for project in &snapshot.navigation.projects {
        if let Some(workspace) = project
            .workspaces
            .iter()
            .find(|w| &w.terminal_context_id == context_id)
        {
            context.workspace_id = Some(workspace.workspace_id.to_string());
            context.workspace_label = Some(workspace.label.clone());
            context.workspace_cwd = workspace
                .primary_directory_id
                .as_ref()
                .and_then(|id| {
                    project
                        .directories
                        .iter()
                        .find(|directory| &directory.directory_id == id)
                })
                .map(|directory| directory.path.as_str().to_owned());
        }
    }
    context.tab_label = snapshot
        .terminal_contexts
        .iter()
        .find(|candidate| &candidate.terminal_context_id == context_id)
        .and_then(|context| context.tabs.iter().find(|tab| Some(&tab.tab_id) == tab_id))
        .map(|tab| tab.label.clone());
    context = source::resolve(snapshot, command, context)?;
    freeze(registry, selector, context).map(|(launch, _)| launch)
}

#[cfg(windows)]
fn freeze(
    registry: &orchbee_local_site_runtime::PluginRegistry,
    selector: &PluginPaneSelector,
    context: orchbee_workbench_protocol::PluginInvocationContext,
) -> Result<(PluginPaneLaunch, String), WorkbenchCommandUnavailableReason> {
    use std::path::Path;
    let unavailable = WorkbenchCommandUnavailableReason::ServiceUnavailable;
    let catalog = registry
        .list(Some(&selector.plugin_id))
        .map_err(|_| unavailable)?;
    let entry = catalog.plugins.first().ok_or(unavailable)?;
    if !entry.enabled || entry.warning.is_some() {
        return Err(unavailable);
    }
    let pane = entry
        .manifest
        .panes
        .iter()
        .find(|pane| pane.id == selector.entrypoint_id)
        .ok_or(unavailable)?;
    if !entry.manifest.supports_windows(&pane.platforms) {
        return Err(WorkbenchCommandUnavailableReason::NoCompatibleWorker);
    }
    let root = Path::new(&entry.plugin_root);
    let program = orchbee_local_site_runtime::resolve_windows_program(&pane.command[0], root);
    let path = |path: &Path| path.to_str().map(str::to_owned).ok_or(unavailable);
    let launch = LaunchSelection::new(path(&program)?, pane.command[1..].to_vec())
        .map_err(|_| unavailable)?;
    context.validate().map_err(|_| unavailable)?;
    let (config, state) = registry
        .user_directories(&entry.manifest.id)
        .map_err(|_| unavailable)?;
    let bin = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|parent| parent.join("orchbee.exe")))
        .ok_or(unavailable)?;
    let mut environment = vec![
        ("ORCHBEE_PLUGIN_ID".into(), entry.manifest.id.clone()),
        ("ORCHBEE_PLUGIN_ROOT".into(), entry.plugin_root.clone()),
        ("ORCHBEE_PLUGIN_ENTRYPOINT_ID".into(), pane.id.clone()),
        ("ORCHBEE_PLUGIN_CONFIG_DIR".into(), path(&config)?),
        ("ORCHBEE_PLUGIN_STATE_DIR".into(), path(&state)?),
        (
            "ORCHBEE_PLUGIN_CONTEXT_JSON".into(),
            serde_json::to_string(&context).map_err(|_| unavailable)?,
        ),
        ("ORCHBEE_BIN_PATH".into(), path(&bin)?),
    ];
    for (key, value) in [
        ("ORCHBEE_WORKSPACE_ID", context.workspace_id),
        ("ORCHBEE_TAB_ID", context.tab_id),
        ("ORCHBEE_PANE_ID", context.focused_pane_id),
    ] {
        if let Some(value) = value {
            environment.push((key.into(), value));
        }
    }
    Ok((
        PluginPaneLaunch {
            selector: selector.clone(),
            launch,
            directory: WorkingDirectory::parse(&entry.plugin_root).map_err(|_| unavailable)?,
            environment: PluginTerminalEnvironment::new(environment).map_err(|_| unavailable)?,
        },
        pane.title.clone(),
    ))
}

#[cfg(windows)]
impl super::WorkbenchAuthority {
    /// An independent terminal never acquires canonical plugin Pane membership.
    pub fn resolve_plugin_popup(
        &self,
        popup_id: &orchbee_workbench_protocol::WorkbenchRequestId,
        spec: &orchbee_workbench_protocol::PluginPopupSpec,
    ) -> Result<(PluginPaneLaunch, String), WorkbenchCommandUnavailableReason> {
        use orchbee_workbench_protocol::PanelNode;
        let unavailable = WorkbenchCommandUnavailableReason::ServiceUnavailable;
        spec.validate().map_err(|_| unavailable)?;
        if self.retired {
            return Err(unavailable);
        }
        let snapshot = self.snapshot();
        let tab = snapshot
            .terminal_contexts
            .iter()
            .find(|context| context.terminal_context_id == spec.source.terminal_context_id)
            .and_then(|context| {
                context
                    .tabs
                    .iter()
                    .find(|tab| tab.tab_id == spec.source.tab_id)
            })
            .ok_or(unavailable)?;
        fn contains(
            node: &PanelNode,
            source: &orchbee_workbench_protocol::ClientPopupSource,
        ) -> bool {
            match node {
                PanelNode::Panel {
                    panel_id,
                    primary_presentation,
                    child_presentations,
                    ..
                } => {
                    *panel_id == source.panel_id
                        && std::iter::once(primary_presentation)
                            .chain(child_presentations)
                            .any(|candidate| {
                                &candidate.presentation_id == source.member.presentation_id()
                                    && candidate.binding.as_ref() == source.member.binding()
                            })
                }
                PanelNode::Split { children, .. } => {
                    children.iter().any(|child| contains(&child.node, source))
                }
            }
        }
        if !contains(&tab.root, &spec.source) {
            return Err(unavailable);
        }
        let context = source::observed(snapshot, popup_id, &spec.context)?;
        let registry = orchbee_local_site_runtime::PluginRegistry::from_environment(|name| {
            std::env::var_os(name)
        })
        .map_err(|_| unavailable)?;
        freeze(&registry, &spec.selector, context)
    }
}

#[cfg(all(test, windows))]
mod tests;

mod membership;
#[cfg(windows)]
mod source;
pub(super) use membership::Membership;
