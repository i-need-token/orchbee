//! Shared Workspace creation syntax preserves inert directory/program and focus intent.
use super::user_path::ParsedUserPath;
use super::{
    CreationDirectory, ExpectedValue, FailureEnvelope, LaunchOptions, LaunchProfileSelector,
    LaunchSelection, Options, OutputFormat, ParseFailure, Request, Resource, Target, UsageContext,
    UsageReason, WorkbenchLabel, WorkbenchTerminalGeometry, check_scope, client_control,
    geometry_option, label_option, launch_options_with_directory, profile_option,
    reject_launch_profile_conflict, target, usage_with, wait_timeout_option,
};
use orchbee_workbench_protocol::WorkbenchClientId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkspaceCreationCommand<D = CreationDirectory, L = LaunchSelection> {
    pub(crate) project: Option<Target>,
    pub(crate) source: Option<Target>,
    pub(crate) directory: D,
    pub(crate) label: Option<WorkbenchLabel>,
    pub(crate) geometry: WorkbenchTerminalGeometry,
    pub(crate) launch: Option<L>,
    pub(crate) profile: Option<LaunchProfileSelector>,
    pub(crate) options: LaunchOptions,
    pub(crate) client: Option<WorkbenchClientId>,
    pub(crate) focus: bool,
    pub(crate) timeout_ms: u32,
}

/// Parsed directory intent has no environment or current-directory authority yet.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ParsedWorkspaceCreationDirectory {
    Default,
    Follow,
    Home,
    Current,
    Workspace,
    Explicit(ParsedUserPath),
}

pub(crate) type ParsedWorkspaceCreationCommand =
    WorkspaceCreationCommand<ParsedWorkspaceCreationDirectory, super::program::ParsedLaunch>;

mod windows;

fn path_failure(
    value: &str,
    failure: super::user_path_windows::PathFailure,
    envelope: FailureEnvelope,
) -> ParseFailure {
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
            context: UsageContext::Path("workspace create"),
            option,
            value,
            expected,
        },
        envelope,
    )
}

pub(super) fn parse(options: &Options, envelope: FailureEnvelope) -> Result<Request, ParseFailure> {
    let context = UsageContext::Path("workspace create");
    check_scope(options, context, envelope)?;
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
    let launch_options = launch_options_with_directory(options, None, context, envelope)?;
    let directory = match options.cwd.as_deref() {
        None => ParsedWorkspaceCreationDirectory::Default,
        Some("follow") => ParsedWorkspaceCreationDirectory::Follow,
        Some("home") => ParsedWorkspaceCreationDirectory::Home,
        Some("current") => ParsedWorkspaceCreationDirectory::Current,
        Some("workspace") => ParsedWorkspaceCreationDirectory::Workspace,
        Some(value) => ParsedWorkspaceCreationDirectory::Explicit(
            ParsedUserPath::parse(value)
                .map_err(|failure| path_failure(value, failure, envelope))?,
        ),
    };
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
            launch: super::program::parse(options, context, envelope)?,
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
