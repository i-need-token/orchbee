//! Selects Workbench status precedence from the current immutable projection.
//! Text renders selected facts; it never reads a view, retains commands or changes retirement.

use super::WorkbenchClientView;
pub(crate) use crate::localization::PersistentStatus;
use crate::localization::{Locale, WorkbenchClientNotice};
use crate::workbench_state::workspace_setup::{LocationLabelStatus, SetupStatus};

/// One selected status fact, with its short form when it names a lasting state.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct NativeStatus {
    pub(crate) text: String,
    pub(crate) persistent: Option<PersistentStatus>,
    /// The ordinary Ready and controlling fact, which the floating status leaves unsaid.
    pub(crate) ordinary: bool,
}

impl NativeStatus {
    const fn fact(text: String) -> Self {
        Self {
            text,
            persistent: None,
            ordinary: false,
        }
    }
}

impl WorkbenchClientView<'_> {
    /// Select one current status without changing the retained operation or its outcome.
    pub(crate) fn status(&self, locale: Locale, columns: u16) -> NativeStatus {
        let view = self;
        if let Some(notice) = view.worktree_stated_notice() {
            if notice == crate::localization::WorktreeNotice::Failed
                && matches!(
                    view.worktree_failure(),
                    Some(orchbee_workbench_protocol::WorktreeOperationFailure::LaunchRequestTooLarge {})
                )
            {
                return NativeStatus::fact(locale.launch_request_too_large().to_owned());
            }
            return NativeStatus::fact(
                locale.worktree_status_line(notice, view.worktree().map(|v| v.stage())),
            );
        }
        if let Some((status, id)) = view.takeover_status() {
            return NativeStatus::fact(locale.takeover_status(status, id));
        }
        if let Some(status) = view.workspace_close_status() {
            return NativeStatus::fact(locale.workspace_close_status(status));
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
            let end = view
                .focus_panel_id()
                .and_then(|panel| view.terminal_end(panel));
            let text = match end {
                Some(end)
                    if crate::terminal_text::display_width(&status) > usize::from(columns) =>
                {
                    locale.terminal_end_compact(end)
                }
                _ => status,
            };
            return NativeStatus {
                text,
                persistent: end.map(PersistentStatus::TerminalEnded),
                ordinary: false,
            };
        }
        if view.notice().is_none()
            && view.retirement().is_none()
            && view.setup_status().is_none()
            && let Some(progress) = view.duplicate_progress()
        {
            return NativeStatus::fact(locale.duplication_progress(progress));
        }
        let phase = view.phase_status_text(locale);
        let can_configure_directory = view.phase() == crate::localization::DisplayPhase::Empty
            && view.workspace_directory_unconfigured();
        let text = status_with_setup(
            locale,
            phase.clone(),
            view.retirement().is_some(),
            view.notice(),
            view.setup_status(),
            can_configure_directory,
        );
        // Only the bare fact has a short form; an appended recovery fact keeps the full text.
        let persistent = if view.retirement().is_none()
            && view.notice().is_none()
            && view.setup_status() == Some(SetupStatus::WorkspaceCreated)
            && can_configure_directory
        {
            Some(PersistentStatus::DirectoryMissing)
        } else if text == phase
            // A retired connection is Disconnected whatever its last operation's outcome; the
            // pane keeps the full fact, including an unknown outcome that will not replay.
            && (view.retirement().is_some()
                || (view.notice().is_none()
                    && view.setup_status().is_none()
                    && view.phase() == crate::localization::DisplayPhase::Retired))
        {
            Some(PersistentStatus::Disconnected)
        } else {
            None
        };
        let ordinary = text == phase
            && view.retirement().is_none()
            && view.notice().is_none()
            && view.setup_status().is_none()
            && view.quick_workspace_recovery().is_none()
            && view.phase() == crate::localization::DisplayPhase::Ready
            && view.controller() == Some(crate::localization::DisplayController::Controlling)
            && view.active_position().is_some();
        NativeStatus {
            text,
            persistent,
            ordinary,
        }
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
