//! Preserve the Unix Workbench create/launch routes behind the shared parsed request.
use crate::cli::Output;
use crate::invocation::workspace_creation::ParsedWorkspaceCreationDirectory as Directory;
use crate::invocation::{
    ExpectedValue, MutationCommand, OutputFormat, ParsedWorkspaceCreationCommand, UsageContext,
    UsageReason, selection_envelope, usage_with,
};
use crate::localization::Locale;

pub(super) fn run(
    command: ParsedWorkspaceCreationCommand,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    if let Err(failure) =
        crate::cli::execution::admit(crate::cli::execution::Capability::WorkbenchMutation)
    {
        return crate::cli::failure_output(failure, Some(format), locale);
    }
    let context = UsageContext::Path("workspace create");
    let envelope = selection_envelope(Some(format));
    if command.source.is_some() || command.client.is_some() || command.focus {
        return crate::cli::unsupported_platform(Some(format), locale);
    }
    let (Some(project), Some(label)) = (command.project, command.label) else {
        // The common grammar admits the Windows default/source workflow. Unix's existing
        // Workbench route needs an explicit project and label and does not emulate it.
        return crate::cli::unsupported_platform(Some(format), locale);
    };
    let directory = match command.directory {
        Directory::Default => None,
        Directory::Explicit(path) => {
            match orchbee_workbench_protocol::WorkbenchDirectoryPath::parse(path.as_str()) {
                Ok(path) => Some(path),
                Err(_) => {
                    return crate::cli::parse_failure(
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
            return crate::cli::unsupported_platform(Some(format), locale);
        }
    };
    let mutation = if let Some(directory) = directory {
        let launch = match crate::invocation::program::resolve(command.launch, context, envelope) {
            Ok(launch) => launch,
            Err(failure) => return crate::cli::parse_failure(failure, locale),
        };
        let mut options = command.options;
        options.cwd = Some(directory);
        MutationCommand::WorkspaceLaunch {
            project: Some(project),
            label,
            automatic_label: false,
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
            return crate::cli::unsupported_platform(Some(format), locale);
        }
        return crate::cli::mutation::create_workspace(&project, &label, format, locale);
    };
    crate::cli::mutation(mutation, format, locale)
}
