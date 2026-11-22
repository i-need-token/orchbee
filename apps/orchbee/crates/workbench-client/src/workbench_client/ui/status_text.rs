//! Selects Workbench status precedence from the current immutable projection.
//! Text renders selected facts; it never reads a view, retains commands or changes retirement.

use super::WorkbenchClientView;
pub(crate) use crate::localization::PersistentStatus;
use crate::localization::{Locale, WorkbenchClientNotice};
use crate::workbench_state::workspace_setup::{LocationLabelStatus, SetupStatus};
use orchbee_workbench_protocol::TerminalRuntimeState;

/// One selected status fact, with its short form when it names a lasting state.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct NativeStatus {
    pub(crate) text: String,
    pub(crate) persistent: Option<PersistentStatus>,
    /// The ordinary Ready and controlling fact, which the floating status leaves unsaid.
    pub(crate) ordinary: bool,
    /// The documented shorter forms that replace `text` when it does not fit, longest first.
    pub(crate) shorter: Vec<String>,
    /// `text` states a notice, which the status line never drops for width.
    pub(crate) notice: bool,
}

impl NativeStatus {
    const fn fact(text: String) -> Self {
        Self {
            text,
            persistent: None,
            ordinary: false,
            shorter: Vec::new(),
            notice: false,
        }
    }

    /// A notice stated by its forms, longest first.
    fn notice(forms: impl IntoIterator<Item = String>) -> Self {
        let mut forms = forms.into_iter();
        Self {
            text: forms.next().unwrap_or_default(),
            persistent: None,
            ordinary: false,
            shorter: forms.collect(),
            notice: true,
        }
    }
}

impl WorkbenchClientView<'_> {
    /// Select one current status without changing the retained operation or its outcome.
    pub(crate) fn status(&self, locale: Locale, columns: u16) -> NativeStatus {
        let view = self;
        // A lost local connection the shell is bringing back says so until it returns or gives up.
        if view.rejoining() {
            let reconnecting = locale.status().reconnecting.to_owned();
            return NativeStatus {
                text: reconnecting.clone(),
                persistent: Some(PersistentStatus::Reconnecting),
                ordinary: false,
                shorter: vec![reconnecting],
                notice: false,
            };
        }
        // The Server's state speaks for every terminal ahead of any one operation's.
        if view.retirement().is_none() {
            match view.terminal_runtime() {
                TerminalRuntimeState::Serving => {}
                TerminalRuntimeState::Reconnecting => {
                    return NativeStatus {
                        text: view.phase_status_text(locale),
                        persistent: Some(PersistentStatus::Reconnecting),
                        ordinary: false,
                        shorter: vec![locale.status().reconnecting.to_owned()],
                        notice: false,
                    };
                }
                TerminalRuntimeState::GivenUp => {
                    return NativeStatus::notice([locale.terminal_given_up_notice()]);
                }
            }
        }
        if let Some(notice) = view.worktree_stated_notice() {
            if notice == crate::localization::WorktreeNotice::Failed
                && matches!(
                    view.worktree_failure(),
                    Some(orchbee_workbench_protocol::WorktreeOperationFailure::LaunchRequestTooLarge {})
                )
            {
                return NativeStatus::notice(
                    locale.native_notice_forms(WorkbenchClientNotice::LaunchRequestTooLarge, None),
                );
            }
            return NativeStatus::fact(
                locale.worktree_status_line(notice, view.worktree().map(|v| v.stage())),
            );
        }
        if let Some((status, _)) = view.takeover_status() {
            return NativeStatus::fact(locale.takeover_status(status).to_owned());
        }
        if let Some(text) = view
            .workspace_close_status()
            .and_then(|status| locale.workspace_close_status(status))
        {
            return NativeStatus::fact(text);
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
                shorter: Vec::new(),
                notice: false,
            };
        }
        let directory_unconfigured = view.phase() == crate::localization::DisplayPhase::Empty
            && view.workspace_directory_unconfigured();
        // A setup outcome that says nothing leaves the line as if there were none.
        let setup = view.setup_status().filter(|status| {
            locale.setup_status(*status).is_some()
                || *status == SetupStatus::WorkspaceCreated && directory_unconfigured
        });
        if view.notice().is_none()
            && view.retirement().is_none()
            && setup.is_none()
            && let Some(progress) = view.duplicate_progress()
            && let Some(text) = locale.duplication_progress(progress)
        {
            return NativeStatus::fact(text);
        }
        let phase = view.phase_status_text(locale);
        let text = status_with_setup(
            locale,
            phase.clone(),
            view.retirement().is_some(),
            view.notice(),
            setup,
            directory_unconfigured,
        );
        // Only the bare fact has a short form; an appended recovery fact keeps the full text.
        let persistent = if view.retirement().is_none()
            && view.notice().is_none()
            && setup == Some(SetupStatus::WorkspaceCreated)
            && directory_unconfigured
        {
            Some(PersistentStatus::DirectoryMissing)
        } else if text == phase
            && view.retirement().is_none()
            && view.notice().is_none()
            && setup.is_none()
            && view.phase() == crate::localization::DisplayPhase::SelectedWithoutAttachment
            && view.selected_is_held()
        {
            Some(PersistentStatus::DirectoryUnavailable)
        } else if text == phase
            // A retired connection is Disconnected whatever its last operation's outcome; the
            // pane keeps the full fact, including an unknown outcome that will not replay.
            && (view.retirement().is_some()
                || (view.notice().is_none()
                    && setup.is_none()
                    && view.phase() == crate::localization::DisplayPhase::Retired))
        {
            Some(PersistentStatus::Disconnected)
        } else {
            None
        };
        let ordinary = text == phase
            && view.retirement().is_none()
            && view.notice().is_none()
            && setup.is_none()
            && view.quick_workspace_recovery().is_none()
            && view.phase() == crate::localization::DisplayPhase::Ready
            && view.controller() == Some(crate::localization::DisplayController::Controlling)
            && view.active_position().is_some();
        // A stated notice keeps its outcome when the line is short: an appended recovery fact
        // yields first, then the notice's advice, then its context.
        if let Some(notice) = view.notice().filter(|_| view.retirement().is_none()) {
            let forms = locale.native_notice_forms(notice, view.notice_context());
            return match appended_setup_fact(locale, false, setup) {
                Some(fact) => {
                    let stated = forms
                        .first()
                        .map_or_else(String::new, |notice| format!("{notice} · {fact}"));
                    NativeStatus::notice(std::iter::once(stated).chain(forms))
                }
                None => NativeStatus::notice(forms),
            };
        }
        NativeStatus {
            text,
            persistent,
            ordinary,
            shorter: Vec::new(),
            notice: false,
        }
    }

    /// Describe the selected presentation after the feature-owned recovery precedence.
    pub(crate) fn phase_status_text(&self, locale: Locale) -> String {
        let view = self;
        if view.retirement().is_none() {
            match view.terminal_runtime() {
                TerminalRuntimeState::Serving => {}
                TerminalRuntimeState::Reconnecting => {
                    return locale.terminal_reconnecting(view.safe_tab_label(locale).as_deref());
                }
                TerminalRuntimeState::GivenUp => {
                    return locale.terminal_cant_reconnect(view.safe_tab_label(locale).as_deref());
                }
            }
        }
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
            // A held restore names what its placeholder says; any other unbound member is an
            // empty slot.
            return if view.selected_is_held() {
                locale.held_restore_title()
            } else {
                locale.native_unbound_terminal()
            }
            .to_owned();
        }
        locale.native_phase_status_values(
            view.phase(),
            view.safe_tab_label(locale).as_deref(),
            view.controller(),
            view.selected_provenance(),
            view.retirement(),
        )
    }

    /// The shown Tab's label, safe to draw, which a phase status names first.
    fn safe_tab_label(&self, locale: Locale) -> Option<String> {
        self.active_position()
            .and_then(|position| self.tab(position - 1))
            .map(|tab| {
                crate::terminal_text::sanitize_terminal_text(&tab.display_label(locale))
                    .into_owned()
            })
    }
}

fn status_with_setup(
    locale: Locale,
    phase: String,
    retired: bool,
    notice: Option<WorkbenchClientNotice>,
    setup: Option<SetupStatus>,
    directory_unconfigured: bool,
) -> String {
    let primary = if retired {
        Some(phase.clone())
    } else {
        notice.map(|notice| locale.native_notice(notice))
    };
    match (primary, setup) {
        (Some(primary), setup) => match appended_setup_fact(locale, retired, setup) {
            Some(fact) => format!("{primary} · {fact}"),
            None => primary,
        },
        (None, Some(SetupStatus::WorkspaceCreated)) if directory_unconfigured => {
            locale.setup_workspace_directory_action().to_owned()
        }
        (None, Some(status)) => locale.setup_status(status).map_or(phase, str::to_owned),
        (None, None) => phase,
    }
}

/// The recovery fact a stated notice, or a retired connection's phase, carries after it.
fn appended_setup_fact(
    locale: Locale,
    retired: bool,
    setup: Option<SetupStatus>,
) -> Option<&'static str> {
    match setup? {
        SetupStatus::LocationLabel {
            status: LocationLabelStatus::CommittedNeedRefresh,
            ..
        } if retired => Some(locale.location_rename_committed_fact()),
        SetupStatus::CommittedNeedRefresh | SetupStatus::TabsCommittedNeedRefresh if retired => {
            Some(locale.setup_committed_fact())
        }
        status @ (SetupStatus::CommittedNeedRefresh
        | SetupStatus::OutcomeUnknown
        | SetupStatus::TabsCommittedNeedRefresh
        | SetupStatus::TabsOutcomeUnknown
        | SetupStatus::LocationLabel {
            status: LocationLabelStatus::CommittedNeedRefresh | LocationLabelStatus::OutcomeUnknown,
            ..
        }) => locale.setup_status(status),
        _ => None,
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
