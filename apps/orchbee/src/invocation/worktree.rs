use super::{
    ExpectedValue, FailureEnvelope, NO_OPTIONS, Options, OutputFormat, ParseFailure, Request,
    Resource, Scope, Target, UsageContext, UsageReason, WorkbenchDirectoryPath, WorkbenchLabel,
    WorkbenchRequestId, WorkbenchTerminalGeometry, check_scope, geometry_option, label_option,
    missing_required, target, usage_with, wait_timeout_option,
};
use orchbee_workbench_protocol::{WorkbenchClientId, WorktreeReference, WorktreeSelector};
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum WorktreeStartKind {
    Create {
        branch: Option<WorktreeReference>,
        base: Option<WorktreeReference>,
        path: Option<WorkbenchDirectoryPath>,
    },
    Open {
        selector: WorktreeSelector,
    },
    Remove {
        force: bool,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorktreeStartCommand {
    pub(crate) project: Option<Target>,
    pub(crate) workspace: Option<Target>,
    pub(crate) cwd: Option<WorkbenchDirectoryPath>,
    pub(crate) client: Option<WorkbenchClientId>,
    pub(crate) kind: WorktreeStartKind,
    pub(crate) label: Option<WorkbenchLabel>,
    pub(crate) geometry: WorkbenchTerminalGeometry,
    pub(crate) trust_repository: bool,
    pub(crate) operation_id: Option<WorkbenchRequestId>,
    pub(crate) focus: bool,
    pub(crate) detach: bool,
    pub(crate) timeout_ms: u32,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum WorktreeCommand {
    Start(Box<WorktreeStartCommand>),
    Status {
        operation_id: WorkbenchRequestId,
    },
    Resume {
        operation_id: WorkbenchRequestId,
        expected_attempt: u32,
        detach: bool,
        timeout_ms: u32,
    },
}
pub(super) fn start(
    verb: &str,
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    let context = UsageContext::Path(match verb {
        "create" => "worktree create",
        "open" => "worktree open",
        _ => "worktree remove",
    });
    let remove = verb == "remove";
    check_scope(
        options,
        Scope {
            project: !remove,
            workspace: true,
            cwd: !remove,
            client: !remove,
            focus: !remove,
            no_focus: !remove,
            label: !remove,
            geometry: !remove,
            branch: !remove,
            base: verb == "create",
            path: !remove,
            force: remove,
            confirm: remove,
            trust_repository: true,
            request_id: true,
            detach: true,
            timeout: true,
            ..NO_OPTIONS
        },
        context,
        envelope,
    )?;
    if remove && !options.confirm {
        return Err(missing_required(context, "'--confirm'", envelope));
    }
    if options.project.is_some() && options.workspace.is_some() {
        return Err(usage_with(
            UsageReason::ConflictingOptions {
                context,
                first: "--project",
                second: "--workspace",
            },
            envelope,
        ));
    }
    if options.workspace.is_some() && options.cwd.is_some() {
        return Err(usage_with(
            UsageReason::ConflictingOptions {
                context,
                first: "--workspace",
                second: "--cwd",
            },
            envelope,
        ));
    }
    for (conflict, second) in [
        (options.focus && options.no_focus, "--no-focus"),
        (options.focus && options.detach, "--detach"),
    ] {
        if conflict {
            return Err(usage_with(
                UsageReason::ConflictingOptions {
                    context,
                    first: "--focus",
                    second,
                },
                envelope,
            ));
        }
    }
    let workspace = options
        .workspace
        .as_ref()
        .map(|value| target(value.clone(), Resource::Workspace, envelope))
        .transpose()?;
    if remove && workspace.is_none() {
        return Err(missing_required(context, "'--workspace'", envelope));
    }
    let reference = |value: &str, option| {
        WorktreeReference::parse(value).map_err(|_| {
            invalid(
                context,
                option,
                value,
                ExpectedValue::WorktreeReference,
                envelope,
            )
        })
    };
    let branch = options
        .branch
        .as_deref()
        .map(|value| reference(value, "--branch"))
        .transpose()?;
    let base = options
        .base
        .as_deref()
        .map(|value| reference(value, "--base"))
        .transpose()?;
    let path = options
        .path
        .as_ref()
        .map(|value| worktree_path(value, context, "--path", envelope))
        .transpose()?;
    let kind = match verb {
        "create" => WorktreeStartKind::Create { branch, base, path },
        "open" => {
            let selector = match (path, branch) {
                (Some(path), None) => WorktreeSelector::Path { path },
                (None, Some(branch)) => WorktreeSelector::Branch { branch },
                (None, None) => {
                    return Err(missing_required(
                        context,
                        "'--path' or '--branch'",
                        envelope,
                    ));
                }
                _ => {
                    return Err(usage_with(
                        UsageReason::ConflictingOptions {
                            context,
                            first: "--path",
                            second: "--branch",
                        },
                        envelope,
                    ));
                }
            };
            WorktreeStartKind::Open { selector }
        }
        _ => WorktreeStartKind::Remove {
            force: options.force,
        },
    };
    let command = WorktreeStartCommand {
        project: options
            .project
            .as_ref()
            .map(|value| target(value.clone(), Resource::Project, envelope))
            .transpose()?,
        workspace,
        cwd: options
            .cwd
            .as_deref()
            .map(|value| worktree_path(value, context, "--cwd", envelope))
            .transpose()?,
        kind,
        client: super::client_control::client_id(options, context, envelope)?,
        focus: options.focus,
        label: options
            .label
            .as_ref()
            .map(|_| label_option(options, context, envelope))
            .transpose()?,
        geometry: geometry_option(options, context, envelope)?,
        trust_repository: options.trust_repository,
        operation_id: options
            .request_id
            .as_deref()
            .map(|value| operation_id(value, context, "--request-id", envelope))
            .transpose()?,
        detach: options.detach,
        timeout_ms: timeout(options, context, envelope)?,
    };
    Ok(Request::Worktree {
        command: WorktreeCommand::Start(Box::new(command)),
        format: options.format.unwrap_or(OutputFormat::Text),
    })
}
pub(super) fn observe(
    verb: &str,
    value: &str,
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    let resume = verb == "resume";
    let context = UsageContext::Path(if resume {
        "worktree resume"
    } else {
        "worktree status"
    });
    check_scope(
        options,
        Scope {
            confirm: resume,
            generation: resume,
            detach: resume,
            timeout: resume,
            ..NO_OPTIONS
        },
        context,
        envelope,
    )?;
    let operation_id = operation_id(value, context, "<operation-id>", envelope)?;
    let command = if resume {
        if !options.confirm {
            return Err(missing_required(context, "'--confirm'", envelope));
        }
        let generation = options
            .generation
            .as_deref()
            .ok_or_else(|| missing_required(context, "'--generation'", envelope))?;
        let expected_attempt = generation
            .parse::<u32>()
            .ok()
            .filter(|attempt| (1..=64).contains(attempt) && attempt.to_string() == generation)
            .ok_or_else(|| {
                invalid(
                    context,
                    "--generation",
                    generation,
                    ExpectedValue::WorktreeAttempt,
                    envelope,
                )
            })?;
        WorktreeCommand::Resume {
            operation_id,
            expected_attempt,
            detach: options.detach,
            timeout_ms: timeout(options, context, envelope)?,
        }
    } else {
        WorktreeCommand::Status { operation_id }
    };
    Ok(Request::Worktree {
        command,
        format: options.format.unwrap_or(OutputFormat::Text),
    })
}
fn operation_id(
    value: &str,
    context: UsageContext,
    option: &'static str,
    envelope: FailureEnvelope,
) -> Result<WorkbenchRequestId, ParseFailure> {
    WorkbenchRequestId::parse(value).map_err(|_| {
        invalid(
            context,
            option,
            value,
            ExpectedValue::WorktreeOperationId,
            envelope,
        )
    })
}
fn timeout(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<u32, ParseFailure> {
    if options.timeout.is_some() {
        wait_timeout_option(options, context, envelope)
    } else {
        Ok(120_000)
    }
}
fn invalid(
    context: UsageContext,
    option: &'static str,
    value: &str,
    expected: ExpectedValue,
    envelope: FailureEnvelope,
) -> ParseFailure {
    usage_with(
        UsageReason::InvalidValue {
            context,
            option,
            value: value.to_owned(),
            expected,
        },
        envelope,
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorktreeQueryCommand {
    pub(crate) workspace: Option<Target>,
    pub(crate) cwd: Option<WorkbenchDirectoryPath>,
    pub(crate) client: Option<WorkbenchClientId>,
    pub(crate) trust_repository: bool,
}

pub(super) fn list(options: &Options, envelope: FailureEnvelope) -> Result<Request, ParseFailure> {
    let context = UsageContext::Path("worktree list");
    if options.directory.is_some() {
        check_scope(options, super::WORKTREE_LIST_SCOPE, context, envelope)?;
        return Ok(super::query(
            super::QueryCommand::WorktreeList {
                directory: super::directory_option(options, context, envelope)?,
            },
            options,
        ));
    }
    check_scope(
        options,
        Scope {
            workspace: true,
            cwd: true,
            client: true,
            trust_repository: true,
            ..NO_OPTIONS
        },
        context,
        envelope,
    )?;
    if options.workspace.is_some() && options.cwd.is_some() {
        return Err(usage_with(
            UsageReason::ConflictingOptions {
                context,
                first: "--workspace",
                second: "--cwd",
            },
            envelope,
        ));
    }
    Ok(Request::WorktreeQuery {
        command: WorktreeQueryCommand {
            client: super::client_control::client_id(options, context, envelope)?,
            workspace: options
                .workspace
                .as_ref()
                .map(|value| target(value.clone(), Resource::Workspace, envelope))
                .transpose()?,
            cwd: options
                .cwd
                .as_deref()
                .map(|value| worktree_path(value, context, "--cwd", envelope))
                .transpose()?,
            trust_repository: options.trust_repository,
        },
        format: options.format.unwrap_or(OutputFormat::Text),
    })
}

fn worktree_path(
    value: &str,
    context: UsageContext,
    option: &'static str,
    envelope: FailureEnvelope,
) -> Result<WorkbenchDirectoryPath, ParseFailure> {
    #[cfg(windows)]
    {
        use super::user_path_windows::{PathFailure, normalize};
        normalize(value, |name| std::env::var_os(name), std::env::current_dir).map_err(|failure| {
            match failure {
                PathFailure::Input => invalid(
                    context,
                    option,
                    value,
                    ExpectedValue::WorktreePath,
                    envelope,
                ),
                PathFailure::Context(name) => {
                    invalid(context, name, "", ExpectedValue::DirectoryPath, envelope)
                }
            }
        })
    }
    #[cfg(not(windows))]
    WorkbenchDirectoryPath::parse(value).map_err(|_| {
        invalid(
            context,
            option,
            value,
            ExpectedValue::WorktreePath,
            envelope,
        )
    })
}
