//! Workspace creation resolves source identity independently from its destination Project.
use crate::{
    cli::{
        Failure, FailureKind,
        caller_context::PanelTarget,
        client_control::{selection, windows as control},
        query, results,
        source::{ArmedSignals, SignalWatch},
        source_context,
    },
    invocation::{CreationDirectory, WorkspaceCreationCommand, user_path_windows},
    local_client::LocalClient,
    localization::Locale,
};
use orchbee_workbench_protocol::{
    ClientViewAction, ClientViewOutcome, ClientViewPhase, ClientViewRecord, Snapshot,
    WorkbenchDirectoryPath, WorkbenchLabel,
};
use serde_json::{Value, json};
use std::time::Instant;

pub(super) struct Prepared {
    pub(super) project: Option<orchbee_workbench_protocol::WorkbenchProjectId>,
    pub(super) directory: Directory,
    pub(super) label: WorkbenchLabel,
    pub(super) selected: Option<ClientViewRecord>,
}
pub(super) struct Directory {
    pub(super) path: WorkbenchDirectoryPath,
    policy: &'static str,
    source: &'static str,
    panel: Option<String>,
}
impl Directory {
    pub(super) fn document(&self) -> Value {
        json!({"policy":self.policy,"source":self.source,"path":self.path,"sourcePanelId":self.panel})
    }
}

pub(super) fn prepare(
    command: &WorkspaceCreationCommand,
    client: &mut LocalClient,
    snapshot: &Snapshot,
    deadline: Instant,
    signals: &mut ArmedSignals,
    locale: Locale,
) -> Result<Prepared, Failure> {
    signals.check_before(deadline)?;
    let follows = matches!(
        command.directory,
        CreationDirectory::Default | CreationDirectory::Follow | CreationDirectory::Workspace
    );
    let implicit = follows && command.source.is_none();
    let caller = if command.client.is_none() && implicit {
        PanelTarget::prepare_optional(|name| std::env::var_os(name))?
            .map(|caller| caller.resolve(snapshot))
            .transpose()?
    } else {
        None
    };
    let selected = if command.client.is_some() || command.focus || (implicit && caller.is_none()) {
        let reply = control::exchange(
            client,
            &snapshot.session_id,
            ClientViewAction::List {},
            deadline,
            signals,
        )?;
        signals.check_before(deadline)?;
        let clients = match reply.outcome {
            ClientViewOutcome::Listed { clients } => clients,
            ClientViewOutcome::Denied {} => return Err(Failure::new(FailureKind::Denied)),
            ClientViewOutcome::Unavailable { .. } => return Err(unavailable()),
            ClientViewOutcome::Synchronized { .. }
            | ClientViewOutcome::Found { .. }
            | ClientViewOutcome::Operation { .. }
            | ClientViewOutcome::NotFound { .. }
            | ClientViewOutcome::Conflict { .. } => {
                return Err(Failure::new(FailureKind::ProtocolInvalid));
            }
        };
        if clients.is_empty() && command.client.is_none() && !command.focus && caller.is_none() {
            None
        } else {
            let selected =
                selection::select(&clients, command.client.as_ref(), caller.as_ref())?.clone();
            if selected.view.phase != ClientViewPhase::Ready {
                return Err(unavailable());
            }
            Some(selected)
        }
    } else {
        None
    };
    let mut origin = if follows {
        source_context::resolve(snapshot, caller.as_ref(), selected.as_ref())?
    } else {
        None
    };
    let source_target = if follows {
        command
            .source
            .as_ref()
            .or_else(|| origin.as_ref().map(|origin| &origin.workspace))
    } else {
        None
    };
    let workspace = source_target
        .map(|target| results::resolve_workspace(snapshot, target))
        .transpose()?;
    // Explicit source can use ambient cwd only after proving its exact membership.
    if follows && command.source.is_some() && origin.is_none() && command.client.is_none() {
        origin = PanelTarget::prepare_optional(|name| std::env::var_os(name))
            .ok()
            .flatten()
            .and_then(|caller| caller.resolve(snapshot).ok())
            .and_then(|caller| {
                source_context::resolve(snapshot, Some(&caller), None)
                    .ok()
                    .flatten()
            });
    }
    let panel = origin
        .as_ref()
        .filter(|origin| {
            workspace
                .is_none_or(|workspace| origin.workspace.text() == workspace.workspace_id.as_str())
        })
        .and_then(|origin| origin.panel.as_ref());
    let primary = workspace
        .and_then(|workspace| {
            snapshot
                .navigation
                .workspace_directory(&workspace.workspace_id)
        })
        .map(|directory| directory.path.clone());
    let policy = match command.directory {
        CreationDirectory::Default | CreationDirectory::Follow => "follow",
        CreationDirectory::Explicit => "explicit",
        CreationDirectory::Home => "home",
        CreationDirectory::Current => "current",
        CreationDirectory::Workspace => "workspace",
    };
    let mut resolved_source = policy;
    let mut source_panel = None;
    let path = match command.directory {
        CreationDirectory::Explicit => {
            resolved_source = "argument";
            command.options.cwd.clone().ok_or_else(internal)?
        }
        CreationDirectory::Home => local_directory("~")?,
        CreationDirectory::Current => local_directory(".")?,
        CreationDirectory::Workspace => {
            primary.ok_or_else(|| Failure::new(FailureKind::NotFound))?
        }
        CreationDirectory::Default | CreationDirectory::Follow => {
            let live = if let Some(panel) = panel {
                source_panel = Some(panel.text().to_owned());
                let terminal = query::primary_terminal(snapshot, panel)?;
                let sample = query::read_terminal_processes_cancellable(
                    client,
                    deadline,
                    snapshot,
                    &terminal,
                    signals.poll_fd(),
                );
                signals.check_before(deadline)?;
                root_directory(&sample?.1.info)?
            } else {
                None
            };
            if let Some(path) = live {
                resolved_source = "terminal-process";
                path
            } else if let Some(path) = primary {
                resolved_source = "workspace";
                path
            } else {
                resolved_source = "home";
                local_directory("~")?
            }
        }
    };
    let directory = Directory {
        policy,
        source: resolved_source,
        path,
        panel: source_panel,
    };
    let project = command
        .project
        .as_ref()
        .map(|target| {
            results::resolve_project(snapshot, target).map(|project| project.project_id.clone())
        })
        .transpose()?;
    let label = command
        .label
        .clone()
        .unwrap_or_else(|| automatic_label(&directory.path, snapshot, locale));
    Ok(Prepared {
        project,
        directory,
        label,
        selected,
    })
}

fn internal() -> Failure {
    Failure::new(FailureKind::InternalFailure)
}
fn unavailable() -> Failure {
    Failure::new(FailureKind::ServiceUnavailable)
}
fn local_directory(value: &str) -> Result<WorkbenchDirectoryPath, Failure> {
    user_path_windows::normalize(value, |name| std::env::var_os(name), std::env::current_dir)
        .map_err(|_| unavailable())
}
fn root_directory(
    info: &orchbee_terminal_protocol::TerminalProcessInfo,
) -> Result<Option<WorkbenchDirectoryPath>, Failure> {
    let row = info
        .processes
        .iter()
        .find(|row| row.pid == info.root_pid)
        .ok_or_else(unavailable)?;
    row.current_directory
        .as_deref()
        .map(|path| {
            if !std::path::Path::new(path).is_absolute() {
                return Err(Failure::new(FailureKind::ProtocolInvalid));
            }
            WorkbenchDirectoryPath::parse(path)
                .map_err(|_| Failure::new(FailureKind::ProtocolInvalid))
        })
        .transpose()
}
fn automatic_label(
    path: &WorkbenchDirectoryPath,
    snapshot: &Snapshot,
    locale: Locale,
) -> WorkbenchLabel {
    let name = std::path::Path::new(path.as_str())
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(locale.cli_default_workspace_label());
    let occupied: std::collections::HashSet<_> = snapshot
        .navigation
        .workspaces
        .iter()
        .map(|workspace| workspace.label.as_str())
        .collect();
    // There are at most N occupied names, so one of these N+1 candidates is free.
    for ordinal in 1..=occupied.len() + 1 {
        let suffix = if ordinal == 1 {
            String::new()
        } else {
            format!(" ({ordinal})")
        };
        let mut label = name
            .chars()
            .take(512 - suffix.chars().count())
            .collect::<String>();
        label.push_str(&suffix);
        if !occupied.contains(label.as_str()) {
            return WorkbenchLabel::parse(&label).expect("nonempty bounded directory label");
        }
    }
    unreachable!("more distinct candidates than occupied Workspace labels")
}

#[cfg(test)]
mod tests;
