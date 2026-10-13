//! Pane commands use canonical terminal creation and its postcommit receipt.
use super::failure::Reason;
use crate::{
    cli::{Output, creation, failure_output},
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
            .ok_or_else(|| Reason::SelectorUnavailable.failure())?;
        let registry = PluginRegistry::from_environment(|name| std::env::var_os(name))
            .map_err(|_| Reason::RegistryUnavailable.failure())?;
        let catalog = registry
            .list(Some(&selector.plugin_id))
            .map_err(|_| Reason::RegistryUnavailable.failure())?;
        let entry = catalog
            .plugins
            .first()
            .ok_or_else(|| Reason::PluginNotFound.failure())?;
        if !entry.enabled || entry.warning.is_some() {
            return Err(Reason::PluginDisabled.failure());
        }
        let pane = entry
            .manifest
            .panes
            .iter()
            .find(|pane| pane.id == selector.entrypoint_id)
            .ok_or_else(|| Reason::PaneNotFound.failure())?;
        if !entry.manifest.supports_windows(&pane.platforms) {
            return Err(Reason::UnsupportedPlatform.failure());
        }
        command.creation.label =
            Some(WorkbenchLabel::parse(&pane.title).map_err(|_| Reason::InvalidTitle.failure())?);
        let placement = command.placement.unwrap_or(pane.placement);
        if placement != PluginPanePlacement::Popup
            && (command.width.is_some() || command.height.is_some())
        {
            return Err(Reason::InvalidGeometry.failure());
        }
        match placement {
            PluginPanePlacement::Popup => {
                if !command.creation.focus {
                    return Err(Reason::PopupFocusRequired.failure());
                }
                if command.active_target_overrides || command.popup_geometry_override {
                    return Err(Reason::PopupTargetUnsupported.failure());
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
                    return Err(Reason::PopupTargetUnsupported.failure());
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
