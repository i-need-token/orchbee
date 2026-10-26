//! Resolve admitted literal operands only in the CLI that executes the Worktree request.
use super::{
    ExpectedValue, OutputFormat, ParseFailure, ParsedWorktreeCommand, ParsedWorktreeQueryCommand,
    UsageContext, WorktreeCommand, WorktreeOpenSelector, WorktreeQueryCommand,
    WorktreeStartCommand, WorktreeStartKind, invalid,
};
use crate::invocation::{
    selection_envelope,
    user_path_windows::{PathFailure, normalize},
};
use std::{ffi::OsString, path::PathBuf};

impl ParsedWorktreeCommand {
    /// Resolves explicit paths before request identity, configuration or runtime access.
    ///
    /// # Errors
    /// Returns the command's usage failure when the executing path context is invalid.
    pub(crate) fn into_resolved(
        self,
        format: OutputFormat,
        mut environment: impl FnMut(&str) -> Option<OsString>,
        mut cwd: impl FnMut() -> std::io::Result<PathBuf>,
    ) -> Result<WorktreeCommand, ParseFailure> {
        Ok(match self {
            WorktreeCommand::Start(command) => {
                let command = *command;
                let context = UsageContext::Path(match &command.kind {
                    WorktreeStartKind::Create { .. } => "worktree create",
                    WorktreeStartKind::Open { .. } => "worktree open",
                    WorktreeStartKind::Remove { .. } => "worktree remove",
                });
                let mut resolve_path = |operand: super::ParsedWorktreePath, option| {
                    normalize(operand.as_str(), &mut environment, &mut cwd).map_err(|failure| {
                        path_failure(operand.as_str(), failure, context, option, format)
                    })
                };
                // Preserve target-before-source precedence when both contexts are invalid.
                let kind = match command.kind {
                    WorktreeStartKind::Create { branch, base, path } => WorktreeStartKind::Create {
                        branch,
                        base,
                        path: path.map(|path| resolve_path(path, "--path")).transpose()?,
                    },
                    WorktreeStartKind::Open { selector } => WorktreeStartKind::Open {
                        selector: match selector {
                            WorktreeOpenSelector::Path { path } => WorktreeOpenSelector::Path {
                                path: resolve_path(path, "--path")?,
                            },
                            WorktreeOpenSelector::Branch { branch } => {
                                WorktreeOpenSelector::Branch { branch }
                            }
                        },
                    },
                    WorktreeStartKind::Remove { force } => WorktreeStartKind::Remove { force },
                };
                WorktreeCommand::Start(Box::new(WorktreeStartCommand {
                    project: command.project,
                    workspace: command.workspace,
                    cwd: command
                        .cwd
                        .map(|path| resolve_path(path, "--cwd"))
                        .transpose()?,
                    client: command.client,
                    kind,
                    label: command.label,
                    geometry: command.geometry,
                    trust_repository: command.trust_repository,
                    operation_id: command.operation_id,
                    focus: command.focus,
                    detach: command.detach,
                    timeout_ms: command.timeout_ms,
                }))
            }
            WorktreeCommand::Status { operation_id } => WorktreeCommand::Status { operation_id },
            WorktreeCommand::Resume {
                operation_id,
                expected_attempt,
                detach,
                timeout_ms,
            } => WorktreeCommand::Resume {
                operation_id,
                expected_attempt,
                detach,
                timeout_ms,
            },
        })
    }
}

impl ParsedWorktreeQueryCommand {
    /// Resolves the explicit source path before deadline, signal or runtime acquisition.
    ///
    /// # Errors
    /// Returns the worktree-list usage failure when the executing path context is invalid.
    pub(crate) fn into_resolved(
        self,
        format: OutputFormat,
        environment: impl FnMut(&str) -> Option<OsString>,
        cwd: impl FnMut() -> std::io::Result<PathBuf>,
    ) -> Result<WorktreeQueryCommand, ParseFailure> {
        let cwd = self
            .cwd
            .map(|operand| {
                normalize(operand.as_str(), environment, cwd).map_err(|failure| {
                    path_failure(
                        operand.as_str(),
                        failure,
                        UsageContext::Path("worktree list"),
                        "--cwd",
                        format,
                    )
                })
            })
            .transpose()?;
        Ok(WorktreeQueryCommand {
            workspace: self.workspace,
            cwd,
            client: self.client,
            trust_repository: self.trust_repository,
        })
    }
}

fn path_failure(
    operand: &str,
    failure: PathFailure,
    context: UsageContext,
    option: &'static str,
    format: OutputFormat,
) -> ParseFailure {
    let (option, value, expected) = match failure {
        PathFailure::Input => (option, operand, ExpectedValue::WorktreePath),
        PathFailure::Context(name) => (name, "", ExpectedValue::DirectoryPath),
    };
    invalid(
        context,
        option,
        value,
        expected,
        selection_envelope(Some(format)),
    )
}

#[cfg(test)]
mod tests;
