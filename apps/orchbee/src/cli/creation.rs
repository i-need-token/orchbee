//! Canonical terminal creation followed by explicitly requested Client effects.
use super::Output;
use crate::{
    invocation::{
        CreationCommand, CreationKind, MutationCommand, OutputFormat, TabContext, Target,
    },
    localization::Locale,
};
use orchbee_workbench_protocol::WorkbenchLabel;

pub(super) mod directory;
mod receipt;
mod targeting;
mod workflow;

pub(crate) fn run_parsed(
    command: &CreationCommand<crate::invocation::program::ParsedLaunch>,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    let route = match Route::select(command) {
        Ok(route) => route,
        Err(failure) => return super::failure_output(failure, Some(format), locale),
    };
    let named = matches!(
        command.launch,
        Some(crate::invocation::program::ParsedLaunch::Path { .. })
    );
    match command.clone().into_resolved(format) {
        Ok(command) => {
            let output = route.run(&command, format, locale);
            super::launch_receipt::annotate(
                output,
                command.launch.as_ref().filter(|_| named),
                format,
                locale,
            )
        }
        Err(failure) => super::parse_failure(failure, locale),
    }
}

pub(crate) fn run(command: &CreationCommand, format: OutputFormat, locale: Locale) -> Output {
    match Route::select(command) {
        Ok(route) => route.run(command, format, locale),
        Err(failure) => super::failure_output(failure, Some(format), locale),
    }
}

enum Route {
    Extended,
    Canonical {
        context: Option<TabContext>,
        target: Option<Target>,
        label: WorkbenchLabel,
    },
}

impl Route {
    fn select<L>(command: &CreationCommand<L>) -> Result<Self, super::Failure> {
        let failure = match super::execution::admit(super::execution::Capability::ExtendedCreation)
        {
            Ok(()) => return Ok(Self::Extended),
            Err(failure) => failure,
        };
        // Preserve the existing explicit Unix route without admitting Client-only effects.
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
        if matches!(
            command.directory,
            crate::invocation::CreationDirectory::Default
                | crate::invocation::CreationDirectory::Explicit
        ) && !command.focus
            && command.client.is_none()
            && let (Some((context, target)), Some(label)) = (explicit, &command.label)
        {
            super::execution::admit(super::execution::Capability::WorkbenchMutation)?;
            return Ok(Self::Canonical {
                context,
                target,
                label: label.clone(),
            });
        }
        Err(failure)
    }

    fn run(self, command: &CreationCommand, format: OutputFormat, locale: Locale) -> Output {
        match self {
            Self::Extended => workflow::run(command, format, locale),
            Self::Canonical {
                context,
                target,
                label,
            } => super::mutation::run(&canonical(command, context, target, label), format, locale),
        }
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
            automatic_label: command.label.is_none(),
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
            // A chosen label names the Panel the way `panel rename` does.
            custom_label: command.label.as_ref().and_then(|label| {
                orchbee_workbench_protocol::PanelLabelUpdate::parse(Some(label.as_str()))
                    .ok()
                    .and_then(|update| update.custom_label().cloned())
            }),
            geometry: command.geometry,
            launch: command.launch.clone(),
            profile: command.profile.clone(),
            options: command.options.clone(),
        },
    }
}
