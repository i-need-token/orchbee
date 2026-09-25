//! Canonical terminal creation followed by explicitly requested Client effects.
use super::Output;
use crate::{
    invocation::{
        CreationCommand, CreationKind, MutationCommand, OutputFormat, TabContext, Target,
    },
    localization::Locale,
};
use orchbee_workbench_protocol::WorkbenchLabel;

#[cfg(windows)]
mod receipt;
#[cfg(windows)]
mod targeting;
#[cfg(windows)]
mod windows;

pub(crate) fn run(command: &CreationCommand, format: OutputFormat, locale: Locale) -> Output {
    #[cfg(windows)]
    {
        windows::run(command, format, locale)
    }
    #[cfg(not(windows))]
    {
        // Preserve the preexisting explicit creation route; the new workflow is Windows-only.
        let explicit = match &command.kind {
            CreationKind::Tab {
                context: Some(context),
            } => Some((Some(context.clone()), None)),
            CreationKind::Split {
                target: Some(target),
                ratio: None,
                ..
            } => Some((None, Some(target.clone()))),
            _ => None,
        };
        if !command.focus
            && command.client.is_none()
            && let (Some((context, target)), Some(label)) = (explicit, &command.label)
        {
            return super::mutation::run(
                &canonical(command, context, target, label.clone()),
                format,
                locale,
            );
        }
        super::failure_output(
            super::Failure::new(super::FailureKind::ServiceUnavailable),
            Some(format),
            locale,
        )
    }
}

fn canonical(
    command: &CreationCommand,
    context: Option<TabContext>,
    target: Option<Target>,
    label: WorkbenchLabel,
) -> MutationCommand {
    match &command.kind {
        CreationKind::Tab { .. } => MutationCommand::TabCreate {
            context: context.expect("creation preflight resolves a Tab context"),
            label,
            geometry: command.geometry,
            launch: command.launch.clone(),
            profile: command.profile.clone(),
            options: command.options.clone(),
        },
        CreationKind::Split {
            direction,
            right_click,
            ..
        } => MutationCommand::PanelSplit {
            right_click: *right_click,
            target: target.expect("creation preflight resolves a Panel"),
            direction: *direction,
            label,
            geometry: command.geometry,
            launch: command.launch.clone(),
            profile: command.profile.clone(),
            options: command.options.clone(),
        },
    }
}
