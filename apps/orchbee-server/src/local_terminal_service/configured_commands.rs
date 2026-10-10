//! Server-resolved configuration dispatch; execution stays with existing physical owners.
use super::{LocalTerminalService, placement};
use crate::worker_obcn_carrier::{
    WorkerOperationRequest, WorkerOperationResult, WorkerRequestError,
};
use orchbee_capability_node_protocol as node;
use orchbee_local_site_runtime::{
    WindowsCommandSize, WindowsCustomCommand, WindowsCustomCommandAction as Kind,
    WindowsCustomCommandEntry,
};
use orchbee_workbench_protocol::{
    ConfiguredCommandDescriptor, ConfiguredCommandFailure as Failure, ConfiguredCommandInvocation,
    ConfiguredCommandKind, ConfiguredCommandOutcome as Outcome, ConfiguredCommandRequest,
    LaunchOptions, LaunchSelection, PluginAction, PluginPopupSize, PluginRequest, PluginWorker,
    ProtocolVersion, Snapshot, WorkbenchAction, WorkbenchCommand, WorkbenchDirectoryPath,
    WorkbenchLabel, WorkbenchPanelSplitDirection, WorkbenchTerminalGeometry, WorkerId,
};

pub(crate) fn descriptor(entry: &WindowsCustomCommandEntry) -> ConfiguredCommandDescriptor {
    let command = &entry.definition;
    ConfiguredCommandDescriptor {
        id: entry.id.clone(),
        action: match command.action {
            Kind::Shell => ConfiguredCommandKind::Shell,
            Kind::Pane => ConfiguredCommandKind::Pane,
            Kind::Popup => ConfiguredCommandKind::Popup,
            Kind::PluginAction => ConfiguredCommandKind::PluginAction,
        },
        bindings: command
            .triggers
            .iter()
            .map(orchbee_local_site_runtime::trigger_label)
            .collect(),
        description: command.description.clone(),
        width: size(command.width),
        height: size(command.height),
    }
}
fn size(value: Option<WindowsCommandSize>) -> Option<PluginPopupSize> {
    value.map(|value| match value {
        WindowsCommandSize::Cells(n) => PluginPopupSize::Cells(n),
        WindowsCommandSize::Percent(n) => PluginPopupSize::Percent(n),
    })
}
pub(crate) struct Prepared {
    pub(crate) command: WindowsCustomCommand,
    pub(crate) snapshot: Snapshot,
    pub(crate) context: orchbee_workbench_protocol::PluginInvocationContext,
    pub(crate) directory: WorkbenchDirectoryPath,
    pub(crate) environment: Vec<(String, String)>,
    placement: Option<crate::terminal_coordinator::WorkerPlacement>,
}
pub(in crate::local_terminal_service) struct Captured {
    pub directory: Option<WorkbenchDirectoryPath>,
    pub selected_text: Option<String>,
}
impl LocalTerminalService {
    pub(crate) fn resolve_configured_command(
        &self,
        id: &str,
    ) -> Result<WindowsCustomCommand, Failure> {
        self.configured_commands()?
            .1
            .into_iter()
            .find(|entry| entry.id == id)
            .map(|entry| entry.definition)
            .ok_or(Failure::NotFound)
    }
    pub(crate) fn prepare_configured_command(
        &self,
        invocation: &ConfiguredCommandInvocation,
    ) -> Result<Prepared, Failure> {
        invocation.validate().map_err(|_| Failure::InvalidIntent)?;
        self.prepare_configured_launch(
            &invocation.command_id,
            invocation
                .source
                .as_ref()
                .map(|source| &source.presentation),
            &invocation.context,
            invocation.expected_revision,
            invocation.selection,
        )
    }
    pub(in crate::local_terminal_service) fn prepare_configured_launch(
        &self,
        command_id: &str,
        source: Option<&orchbee_workbench_protocol::ClientPopupSource>,
        provided: &orchbee_workbench_protocol::PluginInvocationContext,
        expected_revision: orchbee_workbench_protocol::WorkbenchRevision,
        selection: Option<orchbee_workbench_protocol::ConfiguredCommandSelection>,
    ) -> Result<Prepared, Failure> {
        let command = self.resolve_configured_command(command_id)?;
        if matches!(command.action, Kind::Pane | Kind::Popup) && source.is_none() {
            return Err(Failure::StaleSource);
        }
        let (snapshot, target, worker) = {
            let workers = self.workers.lock().map_err(|_| Failure::Unavailable)?;
            let handle = workers.request_handle();
            let mut coordinator = self.coordinator.lock().map_err(|_| Failure::Unavailable)?;
            let snapshot = coordinator.configured_command_snapshot(expected_revision)?;
            let target = source
                .map(|source| {
                    coordinator.configured_command_source(
                        source,
                        expected_revision,
                        placement(handle.as_ref()),
                    )
                })
                .transpose()?;
            (snapshot, target, placement(handle.as_ref()))
        };
        let selection = selection.filter(|_| command.action == Kind::PluginAction);
        let observed = target
            .as_ref()
            .map(|target| self.configured_command_capture(target, selection.as_ref()))
            .transpose()?;
        let mut context = provided.clone();
        if let Some(capture) = &observed
            && selection.is_some()
        {
            context.selected_text = capture.selected_text.clone();
        }
        let mut canonical_directory = None;
        if let Some(selected) = source {
            let workspace =
                snapshot.navigation.workspaces.iter().find(|workspace| {
                    workspace.terminal_context_id == selected.terminal_context_id
                });
            context.workspace_id =
                workspace.map(|workspace| workspace.workspace_id.as_str().into());
            context.workspace_label = workspace.map(|workspace| workspace.label.clone());
            canonical_directory = workspace
                .and_then(|workspace| {
                    snapshot
                        .navigation
                        .workspace_directory(&workspace.workspace_id)
                })
                .map(|directory| directory.path.clone());
            context.workspace_cwd = canonical_directory
                .as_ref()
                .map(|path| path.as_str().into());
            context.tab_id = Some(selected.tab_id.as_str().into());
            context.tab_label = snapshot
                .terminal_contexts
                .iter()
                .find(|context| context.terminal_context_id == selected.terminal_context_id)
                .and_then(|context| {
                    context
                        .tabs
                        .iter()
                        .find(|tab| tab.tab_id == selected.tab_id)
                })
                .map(|tab| tab.label.clone());
            context.focused_pane_id = Some(selected.panel_id.as_str().into());
            let workers = self.workers.lock().map_err(|_| Failure::Unavailable)?;
            let mut coordinator = self.coordinator.lock().map_err(|_| Failure::Unavailable)?;
            let fresh = coordinator.configured_command_source(
                selected,
                expected_revision,
                placement(workers.request_handle().as_ref()),
            )?;
            if target
                .as_ref()
                .is_none_or(|target| target.target != fresh.target)
            {
                return Err(Failure::StaleSource);
            }
        }
        let directory = observed
            .and_then(|capture| capture.directory)
            .or(canonical_directory)
            .or_else(|| {
                std::env::current_dir().ok().and_then(|path| {
                    path.to_str()
                        .and_then(|path| WorkbenchDirectoryPath::parse(path).ok())
                })
            })
            .ok_or(Failure::Unavailable)?;
        if source.is_some() {
            context.focused_pane_cwd = Some(directory.as_str().into());
        }
        context.validate().map_err(|_| Failure::InvalidIntent)?;
        let bin = std::env::current_exe()
            .ok()
            .and_then(|path| path.parent().map(|parent| parent.join("orchbee.exe")))
            .and_then(|path| path.to_str().map(str::to_owned))
            .ok_or(Failure::Unavailable)?;
        let mut environment = vec![
            ("ORCHBEE_COMMAND_ID".into(), command_id.into()),
            ("ORCHBEE_BIN_PATH".into(), bin),
        ];
        for (name, value) in [
            ("ORCHBEE_WORKSPACE_ID", &context.workspace_id),
            ("ORCHBEE_TAB_ID", &context.tab_id),
            ("ORCHBEE_PANE_ID", &context.focused_pane_id),
            ("ORCHBEE_PANE_CWD", &context.focused_pane_cwd),
        ] {
            if let Some(value) = value {
                environment.push((name.into(), value.clone()));
            }
        }
        if self.resolve_configured_command(command_id)? != command {
            return Err(Failure::Conflict);
        }
        Ok(Prepared {
            command,
            snapshot,
            context,
            directory,
            environment,
            placement: worker,
        })
    }
    pub(crate) fn dispatch_configured_command(
        &self,
        request: &ConfiguredCommandRequest,
        invocation: &ConfiguredCommandInvocation,
        prepared: Prepared,
    ) -> Result<Outcome, Failure> {
        {
            let workers = self.workers.lock().map_err(|_| Failure::Unavailable)?;
            if prepared.placement != placement(workers.request_handle().as_ref()) {
                return Err(Failure::StaleSource);
            }
        }
        match prepared.command.action {
            Kind::Shell => {
                let handle = self
                    .workers
                    .lock()
                    .map_err(|_| Failure::Unavailable)?
                    .request_handle()
                    .ok_or(Failure::Unavailable)?;
                if placement(Some(&handle)) != prepared.placement {
                    return Err(Failure::StaleSource);
                }
                let dispatch = node::PluginJobRequest {
                    operation_id: identity(request.request_id.as_str())?,
                    worker_id: node::WorkerId::from_bytes(handle.worker_id().as_bytes())
                        .map_err(|_| Failure::Unavailable)?,
                    worker_generation: node::WorkerGeneration::new(handle.generation()),
                    action: node::PluginJobAction::StartConfigured(node::ConfiguredCommandIntent {
                        command_id: invocation.command_id.clone(),
                        directory: prepared.directory.as_str().into(),
                        command: prepared.command.command,
                        environment: prepared.environment,
                    }),
                };
                let worker = PluginWorker {
                    worker_id: WorkerId::parse(&hex(handle.worker_id().as_bytes()))
                        .map_err(|_| Failure::Unavailable)?,
                    generation: orchbee_workbench_protocol::WorkbenchRevision::new(
                        handle.generation().get(),
                    ),
                };
                match handle.request(WorkerOperationRequest::PluginJob(dispatch.clone())) {
                    Ok(WorkerOperationResult::PluginJob(result)) if result.answers(&dispatch) => {
                        match result.outcome {
                            node::PluginJobOutcome::ConfiguredStarted { existing, .. } => {
                                Ok(Outcome::ShellStarted {
                                    command_id: invocation.command_id.clone(),
                                    worker,
                                    job_id: request.request_id.clone(),
                                    existing,
                                })
                            }
                            node::PluginJobOutcome::Refused(node::PluginJobRefusal::Busy) => {
                                Err(Failure::Busy)
                            }
                            node::PluginJobOutcome::Refused(node::PluginJobRefusal::Conflict) => {
                                Err(Failure::Conflict)
                            }
                            node::PluginJobOutcome::Refused(
                                node::PluginJobRefusal::PhysicalUnknown,
                            ) => Err(Failure::PhysicalUnknown),
                            _ => Err(Failure::Unavailable),
                        }
                    }
                    Err(WorkerRequestError::Backpressure) => Err(Failure::Busy),
                    Err(WorkerRequestError::PostDeliveryOutcomeUnknown(_)) | Ok(_) => {
                        Err(Failure::PhysicalUnknown)
                    }
                    Err(_) => Err(Failure::Unavailable),
                }
            }
            Kind::PluginAction => {
                let reply = self.execute_plugin(PluginRequest {
                    protocol_version: ProtocolVersion,
                    request_id: request.request_id.clone(),
                    site_id: request.site_id.clone(),
                    action: PluginAction::Invoke {
                        operation_id: request.request_id.as_str().into(),
                        plugin_id: None,
                        action_id: prepared.command.command,
                        context: Box::new(prepared.context),
                        link_catalog: None,
                    },
                });
                Ok(Outcome::Plugin {
                    command_id: invocation.command_id.clone(),
                    result: Box::new(reply),
                })
            }
            Kind::Pane => {
                let source = &invocation
                    .source
                    .as_ref()
                    .ok_or(Failure::StaleSource)?
                    .presentation;
                let geometry = match source.member.viewport() {
                    Some(orchbee_workbench_protocol::ClientViewportObservation::Observed {
                        source,
                        scroll,
                        ..
                    }) => {
                        WorkbenchTerminalGeometry::new(source.columns, scroll.viewport_rows, 0, 0)
                    }
                    _ => WorkbenchTerminalGeometry::new(80, 24, 0, 0),
                }
                .map_err(|_| Failure::InvalidIntent)?;
                let command = WorkbenchCommand {
                    protocol_version: ProtocolVersion,
                    request_id: request.request_id.clone(),
                    site_id: request.site_id.clone(),
                    expected_revision: prepared.snapshot.revision,
                    action: WorkbenchAction::PanelSplit {
                        terminal_context_id: source.terminal_context_id.clone(),
                        tab_id: source.tab_id.clone(),
                        panel_id: source.panel_id.clone(),
                        direction: WorkbenchPanelSplitDirection::Below,
                        right_click: Default::default(),
                        label: WorkbenchLabel::parse(
                            prepared.command.description.as_deref().unwrap_or("Command"),
                        )
                        .or_else(|_| WorkbenchLabel::parse("Command"))
                        .map_err(|_| Failure::InvalidIntent)?,
                        geometry,
                        launch: Some(launch(&prepared.command.command, &prepared.environment)?),
                        profile: None,
                        options: Box::new(LaunchOptions {
                            cwd: Some(prepared.directory),
                            ..Default::default()
                        }),
                    },
                };
                let result = self
                    .execute_terminal_tab_create(command)
                    .map_err(|_| Failure::PhysicalUnknown)?;
                Ok(Outcome::Pane {
                    command_id: invocation.command_id.clone(),
                    result: Box::new(result),
                })
            }
            Kind::Popup => Err(Failure::InvalidIntent),
        }
    }
}
pub(in crate::local_terminal_service) fn launch(
    command: &str,
    environment: &[(String, String)],
) -> Result<LaunchSelection, Failure> {
    let root = std::env::var_os("SystemRoot")
        .map(std::path::PathBuf::from)
        .filter(|path| path.is_absolute())
        .ok_or(Failure::Unavailable)?;
    let shell = std::env::var_os("ComSpec")
        .map(std::path::PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| root.join("System32/cmd.exe"));
    let program = root.join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let encoded = orchbee_local_site_runtime::windows_raw_command_encoded(
        shell.to_str().ok_or(Failure::Unavailable)?,
        command,
        environment,
    )
    .ok_or(Failure::InvalidIntent)?;
    LaunchSelection::new(
        program.to_str().ok_or(Failure::Unavailable)?.into(),
        vec![
            "-NoLogo".into(),
            "-NoProfile".into(),
            "-EncodedCommand".into(),
            encoded,
        ],
    )
    .map_err(|_| Failure::InvalidIntent)
}
fn identity(value: &str) -> Result<node::OperationId, Failure> {
    u128::from_str_radix(value, 16)
        .ok()
        .and_then(|value| node::OperationId::from_bytes(value.to_be_bytes()).ok())
        .ok_or(Failure::InvalidIntent)
}
fn hex(bytes: [u8; 16]) -> String {
    format!("{:032x}", u128::from_be_bytes(bytes))
}

#[cfg(test)]
mod tests;
