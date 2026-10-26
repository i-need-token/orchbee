//! Existing Unix Panel moves retain their Workbench mutation route.
use super::Output;
use crate::invocation::{
    MutationCommand, OutputFormat, PanelMoveDestination, PanelRelocationCommand,
    PanelRelocationDestination,
};
use crate::localization::Locale;

pub(crate) fn run(
    command: &PanelRelocationCommand,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    if command.client.is_some() || command.focus.is_some() || command.timeout_ms.is_some() {
        return super::unsupported_platform(Some(format), locale);
    }
    let destination = match &command.destination {
        PanelRelocationDestination::Tab {
            tab,
            panel,
            direction,
            ratio: None,
        } => PanelMoveDestination::Tab {
            tab: tab.clone(),
            panel: panel.clone(),
            direction: *direction,
        },
        PanelRelocationDestination::NewTab {
            context: None,
            label,
        } => PanelMoveDestination::NewTab {
            label: label.clone(),
        },
        _ => return super::unsupported_platform(Some(format), locale),
    };
    super::mutation(
        MutationCommand::PanelMove {
            target: command.source.clone(),
            destination,
        },
        format,
        locale,
    )
}
