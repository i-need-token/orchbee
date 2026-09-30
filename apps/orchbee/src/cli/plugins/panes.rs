//! Pane commands use canonical terminal creation and its postcommit receipt.
use crate::{
    cli::{Failure, FailureKind, Output, creation, failure_output},
    invocation::{CreationKind, OutputFormat, plugins::panes::Open},
    localization::Locale,
};
use orchbee_local_site_runtime::{PluginPanePlacement, PluginRegistry};
use orchbee_workbench_protocol::WorkbenchLabel;

pub(super) fn open(mut command: Open, format: OutputFormat, locale: Locale) -> Output {
    let mut popup = false;
    let prepared = (|| {
        let selector = command
            .creation
            .options
            .plugin_pane
            .as_ref()
            .ok_or_else(|| Failure::new(FailureKind::InternalFailure))?;
        let registry = PluginRegistry::from_environment(|name| std::env::var_os(name))
            .map_err(|_| Failure::new(FailureKind::ServiceUnavailable))?;
        let catalog = registry
            .list(Some(&selector.plugin_id))
            .map_err(|_| Failure::new(FailureKind::ServiceUnavailable))?;
        let entry = catalog
            .plugins
            .first()
            .ok_or_else(|| Failure::new(FailureKind::NotFound))?;
        if !entry.enabled || entry.warning.is_some() {
            return Err(Failure::new(FailureKind::Rejected));
        }
        let pane = entry
            .manifest
            .panes
            .iter()
            .find(|pane| pane.id == selector.entrypoint_id)
            .ok_or_else(|| Failure::new(FailureKind::NotFound))?;
        if !entry.manifest.supports_windows(&pane.platforms) {
            return Err(Failure::new(FailureKind::Rejected));
        }
        command.creation.label = Some(
            WorkbenchLabel::parse(&pane.title).map_err(|_| Failure::new(FailureKind::Rejected))?,
        );
        let placement = command.placement.unwrap_or(pane.placement);
        if placement != PluginPanePlacement::Popup
            && (command.width.is_some() || command.height.is_some())
        {
            return Err(Failure::new(FailureKind::Rejected));
        }
        match placement {
            PluginPanePlacement::Popup => {
                if command.active_target_overrides || command.popup_geometry_override {
                    return Err(Failure::new(FailureKind::Rejected));
                }
                let size = |size: &orchbee_local_site_runtime::PluginPopupSize| match size {
                    orchbee_local_site_runtime::PluginPopupSize::Cells(value) => {
                        Some(orchbee_workbench_protocol::PluginPopupSize::Cells(*value))
                    }
                    orchbee_local_site_runtime::PluginPopupSize::Percentage(value) => {
                        orchbee_workbench_protocol::PluginPopupSize::parse(value)
                    }
                };
                command.width = command.width.or_else(|| pane.width.as_ref().and_then(size));
                command.height = command
                    .height
                    .or_else(|| pane.height.as_ref().and_then(size));
                popup = true;
            }
            PluginPanePlacement::Tab => {
                command.creation.kind = CreationKind::Tab {
                    context: command.workspace.clone(),
                };
            }
            PluginPanePlacement::Split => {}
            PluginPanePlacement::Overlay => {
                if command.active_target_overrides {
                    return Err(Failure::new(FailureKind::Rejected));
                }
                command.creation.kind = CreationKind::Split {
                    right_click: Default::default(),
                    target: None,
                    direction: orchbee_workbench_protocol::WorkbenchPanelSplitDirection::Right,
                    ratio: None,
                };
                command.creation.focus = true;
                command.creation.zoom = true;
                command.creation.overlay = true;
            }
            PluginPanePlacement::Zoomed => {
                command.creation.focus = true;
                command.creation.zoom = true;
            }
        }
        Ok(())
    })();
    match prepared {
        Ok(()) if popup => super::popup::open(&command, format, locale),
        Ok(()) => creation::run(&command.creation, format, locale),
        Err(failure) => failure_output(failure, Some(format), locale),
    }
}

mod manage;
pub(super) use manage::run as manage;
