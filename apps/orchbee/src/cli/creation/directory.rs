//! Resolve a creation directory before the single canonical submission.
use crate::{
    cli::{
        Failure, FailureKind, query,
        source::{ArmedSignals, SignalWatch},
    },
    invocation::{CreationCommand, CreationDirectory, MutationCommand, TabContext, Target},
    local_client::LocalClient,
};
use orchbee_terminal_protocol::TerminalProcessInfo;
use orchbee_workbench_protocol::{ClientViewRecord, Snapshot, WorkbenchDirectoryPath};
use serde_json::{Value, json};
use std::{path::Path, time::Instant};

pub(in crate::cli) struct Resolution {
    policy: &'static str,
    source: &'static str,
    path: Option<WorkbenchDirectoryPath>,
    source_panel_id: Option<String>,
}

impl Resolution {
    pub(in crate::cli) fn document(&self) -> Value {
        json!({"policy":self.policy,"source":self.source,"path":self.path,"sourcePanelId":self.source_panel_id})
    }
}

pub(super) fn resolve(
    command: &CreationCommand,
    canonical: &mut MutationCommand,
    snapshot: &Snapshot,
    origin: (Option<&Target>, Option<&ClientViewRecord>),
    client: &mut LocalClient,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<Resolution, Failure> {
    if command.options.plugin_pane.is_some() && command.options.cwd.is_none() {
        return Ok(Resolution {
            policy: "plugin-root",
            source: "launch-default",
            path: None,
            source_panel_id: None,
        });
    }
    let panel = if matches!(
        command.directory,
        CreationDirectory::Default | CreationDirectory::Follow
    ) {
        source(canonical, snapshot, origin.0, origin.1)?
    } else {
        None
    };
    let options = match canonical {
        MutationCommand::TabCreate { options, .. }
        | MutationCommand::PanelSplit { options, .. } => options,
        _ => return Err(Failure::new(FailureKind::InternalFailure)),
    };
    resolve_options(
        command.directory,
        options,
        snapshot,
        panel.as_ref(),
        client,
        deadline,
        signals,
    )
}

pub(in crate::cli) fn resolve_options(
    directory: CreationDirectory,
    options: &mut orchbee_workbench_protocol::LaunchOptions,
    snapshot: &Snapshot,
    panel: Option<&Target>,
    client: &mut LocalClient,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<Resolution, Failure> {
    let policy = match directory {
        CreationDirectory::Explicit => "explicit",
        CreationDirectory::Default | CreationDirectory::Follow => "follow",
        CreationDirectory::Home => "home",
        CreationDirectory::Current => "current",
        CreationDirectory::Workspace => "workspace",
    };
    let mut result = Resolution {
        policy,
        source: "launch-default",
        path: None,
        source_panel_id: None,
    };
    match directory {
        CreationDirectory::Workspace => return Ok(result),
        CreationDirectory::Explicit => {
            result.source = "argument";
            result.path = options.cwd.clone();
        }
        CreationDirectory::Home | CreationDirectory::Current => {
            let path = if directory == CreationDirectory::Home {
                std::env::var_os("USERPROFILE")
                    .filter(|value| !value.is_empty())
                    .map(std::path::PathBuf::from)
                    .ok_or_else(unavailable)?
            } else {
                std::env::current_dir().map_err(|_| unavailable())?
            };
            result.source = policy;
            result.path =
                Some(absolute(path.to_str().ok_or_else(unavailable)?).ok_or_else(unavailable)?);
        }
        CreationDirectory::Default | CreationDirectory::Follow => {
            let Some(panel) = panel else {
                return Ok(result);
            };
            result.source_panel_id = Some(panel.text().to_owned());
            let terminal = match query::primary_terminal(snapshot, panel) {
                Ok(terminal) => terminal,
                Err(error) if error.kind == FailureKind::NotFound => return Ok(result),
                Err(error) => return Err(error),
            };
            let sample = query::read_terminal_processes_cancellable(
                client,
                deadline,
                snapshot,
                &terminal,
                signals.poll_fd(),
            );
            // A wake is provisional inside transport. Only the signal owner consumes it.
            super::windows::check(signals, deadline)?;
            match sample {
                Ok((_, sample)) => {
                    result.path = root_directory(&sample.info);
                    if result.path.is_some() {
                        result.source = "terminal-process";
                    }
                }
                Err(error) if error.kind == FailureKind::ServiceUnavailable => {}
                Err(error) => return Err(error),
            }
        }
    }
    options.cwd = result.path.clone();
    Ok(result)
}

fn unavailable() -> Failure {
    Failure::new(FailureKind::ServiceUnavailable)
}

fn absolute(value: &str) -> Option<WorkbenchDirectoryPath> {
    Path::new(value)
        .is_absolute()
        .then(|| WorkbenchDirectoryPath::parse(value).ok())
        .flatten()
}

fn root_directory(info: &TerminalProcessInfo) -> Option<WorkbenchDirectoryPath> {
    info.processes
        .iter()
        .find(|row| row.pid == info.root_pid)
        .and_then(|row| row.current_directory.as_deref())
        .and_then(absolute)
}

fn source(
    canonical: &MutationCommand,
    snapshot: &Snapshot,
    caller: Option<&Target>,
    selected: Option<&ClientViewRecord>,
) -> Result<Option<Target>, Failure> {
    match canonical {
        MutationCommand::PanelSplit { target, .. } => Ok(Some(target.clone())),
        MutationCommand::TabCreate { context, .. } => {
            let context_id = match context {
                TabContext::Standalone => &snapshot.navigation.standalone_terminal_context_id,
                TabContext::Workspace(workspace) => {
                    &snapshot
                        .navigation
                        .workspaces
                        .iter()
                        .find(|entry| entry.workspace_id.as_str() == workspace.text())
                        .ok_or_else(unavailable)?
                        .terminal_context_id
                }
            };
            let selected_panel = selected
                .and_then(|client| client.view.selection.as_ref())
                .filter(|view| &view.terminal_context_id == context_id)
                .and_then(|view| view.panel_id.as_ref())
                .map(|id| Target::Id(id.to_string()));
            let Some(panel) = caller.or(selected_panel.as_ref()) else {
                return Ok(None);
            };
            let (id, source_context, _) = super::targeting::panel(snapshot, panel)?;
            Ok((source_context == context_id).then(|| Target::Id(id.to_string())))
        }
        _ => Err(Failure::new(FailureKind::InternalFailure)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orchbee_terminal_protocol::{AttachmentId, TerminalId, TerminalProcess};

    #[test]
    fn root_directory_never_guesses_a_child_or_relative_path() {
        let row = |pid, value: &str| TerminalProcess {
            pid,
            parent_pid: None,
            creation_time: 1,
            image: None,
            command_line: None,
            current_directory: Some(value.to_owned()),
        };
        let mut info = TerminalProcessInfo {
            attachment_id: AttachmentId::from_bytes([1; 16]).unwrap(),
            terminal_id: TerminalId::from_bytes([2; 16]).unwrap(),
            root_pid: 20,
            processes: vec![row(30, r"C:\child"), row(20, r"C:\项目 with spaces")],
        };
        assert_eq!(
            root_directory(&info).unwrap().as_str(),
            r"C:\项目 with spaces"
        );
        for value in ["relative", r"C:relative", "", "bad\0path"] {
            info.processes[1].current_directory = Some(value.to_owned());
            assert!(root_directory(&info).is_none(), "{value:?}");
        }
        info.processes.pop();
        assert!(root_directory(&info).is_none());
    }
}
