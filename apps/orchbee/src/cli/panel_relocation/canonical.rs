//! Existing Unix Panel moves retain their Workbench mutation route.
use crate::cli::Output;
use crate::invocation::{
    MutationCommand, OutputFormat, PanelMoveDestination, PanelRelocationCommand,
    PanelRelocationDestination,
};
use crate::localization::Locale;

pub(super) fn run(
    command: &PanelRelocationCommand,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    if command.client.is_some() || command.focus.is_some() || command.timeout_ms.is_some() {
        return crate::cli::unsupported_platform(Some(format), locale);
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
            label: Some(label),
        } => PanelMoveDestination::NewTab {
            label: label.clone(),
        },
        PanelRelocationDestination::Tab { ratio: Some(_), .. }
        | PanelRelocationDestination::NewTab {
            context: Some(_), ..
        }
        | PanelRelocationDestination::NewTab { label: None, .. }
        | PanelRelocationDestination::NewWorkspace { .. } => {
            return crate::cli::unsupported_platform(Some(format), locale);
        }
    };
    crate::cli::mutation(
        MutationCommand::PanelMove {
            target: command.source.clone(),
            destination,
        },
        format,
        locale,
    )
}
