//! Every Windows Workspace creation uses one launch workflow and explicit focus policy.
use super::{
    CreationDirectory, ExpectedValue, FailureEnvelope, LaunchOptions, LaunchProfileSelector,
    LaunchSelection, NO_OPTIONS, Options, OutputFormat, ParseFailure, Request, Resource, Scope,
    Target, UsageContext, UsageReason, WorkbenchLabel, WorkbenchTerminalGeometry, check_scope,
    client_control, geometry_option, label_option, launch_options_with_directory, profile_option,
    reject_launch_profile_conflict, selection_envelope, target, usage_with, wait_timeout_option,
};
use orchbee_local_site_runtime::WindowsUserPathOperand;
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
    Explicit(WindowsUserPathOperand),
}

pub(crate) type ParsedWorkspaceCreationCommand =
    WorkspaceCreationCommand<ParsedWorkspaceCreationDirectory, super::program::ParsedLaunch>;

impl ParsedWorkspaceCreationCommand {
    /// Resolves explicit paths in the final process, before configuration or runtime access.
    ///
    /// # Errors
    /// Returns the existing workspace-create usage failure for invalid execution context.
    pub(crate) fn into_resolved(
        self,
        format: OutputFormat,
        environment: impl FnMut(&str) -> Option<std::ffi::OsString>,
        cwd: impl FnOnce() -> std::io::Result<std::path::PathBuf>,
    ) -> Result<WorkspaceCreationCommand, ParseFailure> {
        let mut options = self.options;
        // Parsed directory intent is the sole path authority, including policy defaults.
        options.cwd = None;
        let directory = match self.directory {
            ParsedWorkspaceCreationDirectory::Default => CreationDirectory::Default,
            ParsedWorkspaceCreationDirectory::Follow => CreationDirectory::Follow,
            ParsedWorkspaceCreationDirectory::Home => CreationDirectory::Home,
            ParsedWorkspaceCreationDirectory::Current => CreationDirectory::Current,
            ParsedWorkspaceCreationDirectory::Workspace => CreationDirectory::Workspace,
            ParsedWorkspaceCreationDirectory::Explicit(path) => {
                options.cwd = Some(
                    super::user_path_windows::resolve(&path, environment, cwd).map_err(
                        |failure| {
                            path_failure(path.as_str(), failure, selection_envelope(Some(format)))
                        },
                    )?,
                );
                CreationDirectory::Explicit
            }
        };
        Ok(WorkspaceCreationCommand {
            project: self.project,
            source: self.source,
            directory,
            label: self.label,
            geometry: self.geometry,
            launch: super::program::resolve(
                self.launch,
                UsageContext::Path("workspace create"),
                selection_envelope(Some(format)),
            )?,
            profile: self.profile,
            options,
            client: self.client,
            focus: self.focus,
            timeout_ms: self.timeout_ms,
        })
    }
}

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
    let launch_options = launch_options_with_directory(options, None, context, envelope)?;
    let directory = match options.cwd.as_deref() {
        None => ParsedWorkspaceCreationDirectory::Default,
        Some("follow") => ParsedWorkspaceCreationDirectory::Follow,
        Some("home") => ParsedWorkspaceCreationDirectory::Home,
        Some("current") => ParsedWorkspaceCreationDirectory::Current,
        Some("workspace") => ParsedWorkspaceCreationDirectory::Workspace,
        Some(value) => ParsedWorkspaceCreationDirectory::Explicit(
            WindowsUserPathOperand::parse(value)
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

#[cfg(test)]
mod tests;
