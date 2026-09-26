//! Windows launch defaults are opt-in extensions around the valid legacy forms.
use super::{
    CreationDirectory, ExpectedValue, FailureEnvelope, LaunchOptions, LaunchProfileSelector,
    LaunchSelection, NO_OPTIONS, Options, OutputFormat, ParseFailure, Request, Resource, Scope,
    Target, UsageContext, UsageReason, WorkbenchDirectoryPath, WorkbenchLabel,
    WorkbenchTerminalGeometry, check_scope, client_control, geometry_option, label_option,
    launch_option, launch_options_with_directory, profile_option, reject_launch_profile_conflict,
    target, usage_with, wait_timeout_option,
};
use orchbee_workbench_protocol::WorkbenchClientId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkspaceCreationCommand {
    pub(crate) project: Option<Target>,
    pub(crate) source: Option<Target>,
    pub(crate) directory: CreationDirectory,
    pub(crate) label: Option<WorkbenchLabel>,
    pub(crate) geometry: WorkbenchTerminalGeometry,
    pub(crate) launch: Option<LaunchSelection>,
    pub(crate) profile: Option<LaunchProfileSelector>,
    pub(crate) options: LaunchOptions,
    pub(crate) client: Option<WorkbenchClientId>,
    pub(crate) focus: bool,
    pub(crate) timeout_ms: u32,
}

pub(super) fn enhanced(options: &Options) -> bool {
    options.project.is_none()
        || options.label.is_none()
        || options.workspace.is_some()
        || options.client.is_some()
        || options.focus
        || options.no_focus
        || options.timeout.is_some()
        || options.cwd.as_deref().is_some_and(|value| {
            matches!(value, "follow" | "home" | "current" | "workspace")
                || WorkbenchDirectoryPath::parse(value).is_err()
        })
        || (options.cwd.is_none()
            && (options.exec.is_some()
                || options.profile.is_some()
                || !options.args.is_empty()
                || !options.environment.is_empty()
                || options.cols.is_some()
                || options.rows.is_some()
                || options.cell_width.is_some()
                || options.cell_height.is_some()))
}

pub(super) fn parse(options: &Options, envelope: FailureEnvelope) -> Result<Request, ParseFailure> {
    let context = UsageContext::Path("workspace create");
    check_scope(
        options,
        Scope {
            project: true,
            workspace: true,
            label: true,
            cwd: true,
            client: true,
            focus: true,
            no_focus: true,
            timeout: true,
            geometry: true,
            exec: true,
            args: true,
            profile: true,
            environment: true,
            ..NO_OPTIONS
        },
        context,
        envelope,
    )?;
    if options.focus && options.no_focus {
        return Err(usage_with(
            UsageReason::ConflictingOptions {
                context,
                first: "--focus",
                second: "--no-focus",
            },
            envelope,
        ));
    }
    reject_launch_profile_conflict(options, context, envelope)?;
    let directory = match options.cwd.as_deref() {
        None => CreationDirectory::Default,
        Some("follow") => CreationDirectory::Follow,
        Some("home") => CreationDirectory::Home,
        Some("current") => CreationDirectory::Current,
        Some("workspace") => CreationDirectory::Workspace,
        Some(_) => CreationDirectory::Explicit,
    };
    let mut launch_options = launch_options_with_directory(options, None, context, envelope)?;
    if directory == CreationDirectory::Explicit {
        let value = options.cwd.as_deref().expect("explicit directory operand");
        launch_options.cwd = Some(
            super::user_path_windows::normalize(
                value,
                |name| std::env::var_os(name),
                std::env::current_dir,
            )
            .map_err(|failure| {
                let (option, value, expected) = match failure {
                    super::user_path_windows::PathFailure::Input => {
                        ("--cwd", value.to_owned(), ExpectedValue::WorktreePath)
                    }
                    super::user_path_windows::PathFailure::Context(name) => {
                        (name, String::new(), ExpectedValue::DirectoryPath)
                    }
                };
                usage_with(
                    UsageReason::InvalidValue {
                        context,
                        option,
                        value,
                        expected,
                    },
                    envelope,
                )
            })?,
        );
    }
    Ok(Request::WorkspaceCreation {
        command: WorkspaceCreationCommand {
            project: options
                .project
                .as_ref()
                .map(|value| target(value.clone(), Resource::Project, envelope))
                .transpose()?,
            source: options
                .workspace
                .as_ref()
                .map(|value| target(value.clone(), Resource::Workspace, envelope))
                .transpose()?,
            directory,
            label: options
                .label
                .as_ref()
                .map(|_| label_option(options, context, envelope))
                .transpose()?,
            geometry: geometry_option(options, context, envelope)?,
            launch: launch_option(options, context, envelope)?,
            profile: profile_option(options, context, envelope)?,
            options: launch_options,
            client: client_control::client_id(options, context, envelope)?,
            focus: options.focus,
            timeout_ms: if options.timeout.is_some() {
                wait_timeout_option(options, context, envelope)?
            } else {
                30_000
            },
        },
        format: options.format.unwrap_or(OutputFormat::Text),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    fn parse(words: &[&str]) -> Result<Request, ParseFailure> {
        super::super::parse_request(words.iter().map(OsString::from))
    }
    #[test]
    fn explicit_legacy_forms_keep_their_empty_or_directory_mutation() {
        for cwd in [None, Some("C:/repo")] {
            let mut args = vec!["workspace", "create", "--project=p", "--label=w"];
            if let Some(cwd) = cwd {
                args.extend(["--cwd", cwd]);
            }
            assert!(matches!(parse(&args).unwrap(), Request::Mutation { .. }));
        }
        for tail in [
            vec![],
            vec!["--label=w"],
            vec!["--project=p"],
            vec!["--project=p", "--label=w", "--no-focus"],
            vec!["--project=p", "--label=w", "--env=TOKEN=value"],
            vec!["--cwd=follow", "--workspace=source"],
            vec!["--cwd=./repo"],
        ] {
            let mut args = vec!["workspace", "create"];
            args.extend(tail);
            let Request::WorkspaceCreation { command, .. } = parse(&args).unwrap() else {
                panic!("enhanced workflow")
            };
            assert!(!command.focus);
            assert_eq!(command.timeout_ms, 30_000);
        }
    }
    #[test]
    fn invalid_enhanced_inputs_refuse_before_runtime() {
        for tail in [
            vec!["--focus", "--no-focus"],
            vec!["--focus", "--detach"],
            vec!["--client=bad"],
            vec!["--cols=80"],
            vec!["--cwd=C:relative"],
            vec!["--timeout=0"],
            vec!["--env=ORCHBEE_SECRET=private"],
            vec!["--profile=p", "--exec=C:/shell.exe"],
        ] {
            let mut args = vec!["workspace", "create"];
            args.extend(tail);
            assert!(parse(&args).is_err(), "{args:?}");
        }
    }
}
