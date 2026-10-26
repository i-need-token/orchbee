//! Selects Workbench status precedence from the current immutable projection.
//! Text renders selected facts; it never reads a view, retains commands or changes retirement.

use super::WorkbenchClientView;
use crate::{
    localization::Locale,
    workbench_client::{LocationLabelStatus, SetupStatus, WorkbenchClientNotice},
};

impl WorkbenchClientView<'_> {
    /// Select one current status without changing the retained operation or its outcome.
    pub(crate) fn status_text(&self, locale: Locale, columns: u16) -> String {
        let view = self;
        if let Some(notice) = view.worktree_notice() {
            return locale.worktree_status_line(notice, view.worktree().map(|v| v.stage()));
        }
        if let Some((status, id)) = view.takeover_status() {
            return locale.takeover_status(status, id);
        }
        if let Some(status) = view.workspace_close_status() {
            return locale.workspace_close_status(status);
        }
        if view.retirement().is_none()
            && (view.phase() == crate::localization::DisplayPhase::Ready
                && view
                    .focus_panel_id()
                    .and_then(|panel| view.terminal_end(panel))
                    .is_some()
                || view.phase() == crate::localization::DisplayPhase::SelectedWithoutAttachment
                    && view.selected_provenance()
                        == Some(crate::localization::DisplayProvenance::Failed))
        {
            let status = view.phase_status_text(locale);
            if crate::terminal_text::display_width(&status) > usize::from(columns)
                && let Some(end) = view
                    .focus_panel_id()
                    .and_then(|panel| view.terminal_end(panel))
            {
                return locale.terminal_end_compact(end);
            }
            return status;
        }
        if view.notice().is_none()
            && view.retirement().is_none()
            && view.setup_status().is_none()
            && let Some(progress) = view.duplicate_progress()
        {
            return locale.duplication_progress(progress);
        }
        status_with_setup(
            locale,
            view.phase_status_text(locale),
            view.retirement().is_some(),
            view.notice(),
            view.setup_status(),
            view.phase() == crate::localization::DisplayPhase::Empty
                && view.workspace_directory_unconfigured(),
        )
    }

    /// Describe the selected presentation after the feature-owned recovery precedence.
    pub(crate) fn phase_status_text(&self, locale: Locale) -> String {
        let view = self;
        if view.retirement().is_none()
            && view.phase() == crate::localization::DisplayPhase::Ready
            && let Some(end) = view
                .focus_panel_id()
                .and_then(|panel| view.terminal_end(panel))
        {
            return locale.terminal_end(end);
        }
        if let Some(recovery) = view.quick_workspace_recovery() {
            return locale.quick_workspace_failure(
                recovery.project,
                recovery.workspace,
                recovery.unknown,
            );
        }
        if view.phase() == crate::localization::DisplayPhase::SelectedWithoutAttachment
            && view.selected_is_unbound()
            && view.retirement().is_none()
        {
            return locale.native_unbound_terminal().to_owned();
        }
        let tab = view
            .active_position()
            .and_then(|position| view.tab(position - 1));
        let label = tab.map(|tab| tab.display_label(locale));
        let safe_label = label
            .as_deref()
            .map(crate::terminal_text::sanitize_terminal_text);
        locale.native_phase_status_values(
            view.phase(),
            safe_label.as_deref(),
            view.controller(),
            view.selected_provenance(),
            view.projection_status(),
            view.retirement(),
        )
    }
}

fn status_with_setup(
    locale: Locale,
    phase: String,
    retired: bool,
    notice: Option<WorkbenchClientNotice>,
    setup: Option<SetupStatus>,
    can_configure_directory: bool,
) -> String {
    let primary = if retired {
        Some(phase.clone())
    } else {
        notice.map(|notice| locale.native_notice(notice))
    };
    match (primary, setup) {
        (
            Some(primary),
            Some(SetupStatus::LocationLabel {
                target,
                status: LocationLabelStatus::CommittedNeedRefresh,
            }),
        ) if retired => {
            format!(
                "{primary} · {}",
                locale.location_rename_committed_fact(target)
            )
        }
        (
            Some(primary),
            Some(SetupStatus::LocationLabel {
                target,
                status:
                    status @ (LocationLabelStatus::CommittedNeedRefresh
                    | LocationLabelStatus::OutcomeUnknown),
            }),
        ) => {
            format!(
                "{primary} · {}",
                locale.location_label_status(target, status)
            )
        }
        (Some(primary), Some(SetupStatus::CommittedNeedRefresh)) if retired => {
            format!("{primary} · {}", locale.setup_committed_fact())
        }
        (Some(primary), Some(SetupStatus::TabsCommittedNeedRefresh)) if retired => {
            format!("{primary} · {}", locale.setup_tabs_committed_fact())
        }
        (
            Some(primary),
            Some(
                status @ (SetupStatus::CommittedNeedRefresh
                | SetupStatus::OutcomeUnknown
                | SetupStatus::TabsCommittedNeedRefresh
                | SetupStatus::TabsOutcomeUnknown),
            ),
        ) => {
            format!("{primary} · {}", locale.setup_status(status))
        }
        (Some(primary), _) => primary,
        (None, Some(SetupStatus::WorkspaceCreated)) if can_configure_directory => {
            locale.setup_workspace_directory_action().to_owned()
        }
        (None, Some(status)) => locale.setup_status(status).to_owned(),
        (None, None) => phase,
    }
}

pub(super) const fn setup_status_transient(status: SetupStatus) -> bool {
    !matches!(
        status,
        SetupStatus::TabsCommittedNeedRefresh
            | SetupStatus::TabsOutcomeUnknown
            | SetupStatus::CommittedNeedRefresh
            | SetupStatus::OutcomeUnknown
            | SetupStatus::LocationLabel {
                status: LocationLabelStatus::CommittedNeedRefresh
                    | LocationLabelStatus::OutcomeUnknown,
                ..
            }
    )
}

#[cfg(test)]
mod tests;
