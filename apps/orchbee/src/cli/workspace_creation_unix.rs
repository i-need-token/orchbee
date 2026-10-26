//! Preserve the Unix Workbench create/launch routes behind the shared parsed request.
use super::Output;
use crate::invocation::{
    ExpectedValue, MutationCommand, OutputFormat, ParsedWorkspaceCreationCommand,
    ParsedWorkspaceCreationDirectory as Directory, UsageContext, UsageReason, selection_envelope,
    usage_with,
};
use crate::localization::Locale;

pub(super) fn run(
    command: ParsedWorkspaceCreationCommand,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    let context = UsageContext::Path("workspace create");
    let envelope = selection_envelope(Some(format));
    if command.source.is_some() || command.client.is_some() || command.focus {
        return super::unsupported_platform(Some(format), locale);
    }
    let (Some(project), Some(label)) = (command.project, command.label) else {
        // The common grammar admits the Windows default/source workflow. Unix's existing
        // Workbench route needs an explicit project and label and does not emulate it.
        return super::unsupported_platform(Some(format), locale);
    };
    let directory = match command.directory {
        Directory::Default => None,
        Directory::Explicit(path) => {
            match orchbee_workbench_protocol::WorkbenchDirectoryPath::parse(path.as_str()) {
                Ok(path) => Some(path),
                Err(_) => {
                    return super::parse_failure(
                        usage_with(
                            UsageReason::InvalidValue {
                                context,
                                option: "--cwd",
                                value: path.as_str().to_owned(),
                                expected: ExpectedValue::DirectoryPath,
                            },
                            envelope,
                        ),
                        locale,
                    );
                }
            }
        }
        Directory::Follow | Directory::Home | Directory::Current | Directory::Workspace => {
            return super::unsupported_platform(Some(format), locale);
        }
    };
    let mutation = if let Some(directory) = directory {
        let launch = match crate::invocation::program::resolve(command.launch, context, envelope) {
            Ok(launch) => launch,
            Err(failure) => return super::parse_failure(failure, locale),
        };
        let mut options = command.options;
        options.cwd = Some(directory);
        MutationCommand::WorkspaceLaunch {
            project: Some(project),
            label,
            geometry: command.geometry,
            launch,
            profile: command.profile,
            options,
        }
    } else {
        if command.launch.is_some()
            || command.profile.is_some()
            || command.options != Default::default()
        {
            return super::unsupported_platform(Some(format), locale);
        }
        MutationCommand::WorkspaceCreate { project, label }
    };
    super::mutation(mutation, format, locale)
}
