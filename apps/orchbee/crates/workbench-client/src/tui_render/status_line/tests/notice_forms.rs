//! A status notice is never dropped for width (ORC-414): every notice, in both locales, keeps a
//! whole form at the Compact minimum width and a cut one on the narrowest Mobile host.

use ratatui::{Terminal, backend::TestBackend, layout::Rect};

use super::{StatusFacts, facts, render};
use crate::client_view::{BackendAction, ClientView, Notice, SidebarSurface};
use crate::localization::{
    AgentStartStatus, EarlierLayoutNotice, Locale, SettingsNotice, WorkbenchClientNotice,
};
use crate::terminal_text::display_width;
use crate::tui_layout::{
    COMPACT_MINIMUM_COLUMNS, COMPACT_MINIMUM_ROWS, TOO_SMALL_MINIMUM_COLUMNS, project_shell_areas,
    status_chip_area,
};
use crate::tui_theme::Theme;

/// Every Workbench notice, with the widest value of each count. The exhaustive match at the end
/// fails to compile for a new variant until this list names it.
fn every_workbench_notice() -> Vec<WorkbenchClientNotice> {
    use crate::workbench_state::AgentStartRefusal as Refusal;
    use AgentStartStatus as Start;
    use EarlierLayoutNotice as Layout;
    use SettingsNotice as Settings;
    use WorkbenchClientNotice as N;
    use orchbee_local_runtime::{RecordProblem as Problem, VersionedRecord};
    use orchbee_workbench_protocol::{AgentNotResumedReason as Why, ConfiguredCommandFailure as F};

    let mut all = vec![
        N::MenuUnavailable,
        N::LocalBusy,
        N::NoTabAtPosition(u8::MAX),
        N::CreateNotCreated,
        N::CreateOrderConflict,
        N::LaunchRequestTooLarge,
        N::DuplicatePending,
        N::DuplicateNotStarted,
        N::DuplicateOutcomeUnknown,
        N::MoveNotMoved,
        N::MoveOrderConflict,
        N::PanelNotApplied,
        N::RenameNotRenamed,
        N::TerminalClearing,
        N::TerminalCleared,
        N::TerminalClearUnconfirmed,
        N::TerminalClearRefused,
        N::TerminalClearUnavailable,
        N::TerminalClearUnknown,
        N::ScrollbackEditorUnavailable,
        N::ScrollbackEditorTooSmall,
        N::CloseRequestNotApplied,
        N::WorkspaceCloseUnavailable,
        N::WorkspaceChanged,
        N::AgentUnavailable,
        N::NoWorkspace,
        N::WorkspaceNotReady,
        N::ConfirmationCancelled,
        N::DraftCancelled,
        N::DialogRefused,
        N::ModalOwnsInput,
        N::HistoryRecovering,
        N::HistoryUnavailable,
        N::HistoryPositionLost,
        N::CopyModeUnavailable,
        N::CopyModeBusy,
        N::CopyModeInvalidated,
        N::CopyModeQueryFailed,
        N::CopyModeNoMatch,
        N::CopyModeSearchLimit,
        N::CopyModeNoSelection,
        N::FindUnavailable,
        N::FindStopped,
        N::FindFailed,
        N::SelectionEvicted,
        N::CopyFailed,
        N::CopyUnknown,
        N::BellFailed,
        N::BellUnknown,
        N::PositionChanged,
        N::PositionChangedEmpty,
        N::PositionLocationChanged,
        N::PositionLocationChangedEmpty,
        N::PositionUnavailable,
        N::PopupUnavailable,
        N::CommandAccepted,
        N::CommandFailed,
        N::CommandUnknown,
        N::LaunchProfileSelected,
        N::LaunchProfileUnavailable,
        N::RestoreDirectoryUnavailable,
        N::RestoreAgentNotResumed,
        N::RestoreNotStarted,
        N::LinkOpened,
        N::LinkHandled,
        N::LinkNoLink,
        N::LinkBusy,
        N::LinkFailed,
        N::LinkUnknown,
    ];
    all.extend(crate::host_capabilities::Capability::ALL.map(N::Unsupported));
    for record in VersionedRecord::ALL {
        all.extend(
            [Problem::NotUpdated, Problem::Newer].map(|problem| N::RecordUnusable(record, problem)),
        );
    }
    all.extend(
        [
            Settings::SourceChanged,
            Settings::Failed,
            Settings::Unknown,
            Settings::Busy,
            Settings::PresentationUnavailable,
            Settings::TooSmall,
            Settings::FileBusy,
            Settings::InvalidValue,
            Settings::Unsupported,
            Settings::SyntaxError,
            Settings::ReadOnly,
            Settings::TooLarge,
            Settings::UnusableFile,
            Settings::Unreadable,
            Settings::WriteFailed,
        ]
        .map(N::Settings),
    );
    all.extend(
        [
            Start::Failed,
            Start::NotFound("GitHub Copilot CLI"),
            Start::Exited("GitHub Copilot CLI"),
            Start::UnsupportedShell("GitHub Copilot CLI"),
            Start::Refused(Refusal::NotAtPrompt),
            Start::Refused(Refusal::Unmarked),
            Start::Refused(Refusal::SessionTooOld),
            Start::Refused(Refusal::TerminalTooOld),
            Start::Unknown,
            Start::PanelBusy,
        ]
        .map(N::AgentStart),
    );
    all.extend(
        [
            F::Denied,
            F::InvalidIntent,
            F::SessionMismatch,
            F::Unavailable,
            F::Busy,
            F::Conflict,
            F::NotFound,
            F::StaleSource,
            F::PhysicalUnknown,
        ]
        .map(N::CommandRefused),
    );
    for agent in crate::agent_provider::IntegrationId::ALL
        .map(Some)
        .into_iter()
        .chain([None])
    {
        all.extend(
            [Why::PromptUnconfirmed, Why::TerminalTooOld].map(|why| N::AgentNotResumed(agent, why)),
        );
    }
    all.extend(
        [
            Layout::Reopened {
                workspaces: u32::MAX,
                tabs: u32::MAX,
            },
            Layout::NothingToReopen,
            Layout::NotFound,
            Layout::ResourceLimit,
            Layout::Busy,
            Layout::Denied,
            Layout::Empty,
            Layout::Unknown,
            Layout::Unavailable,
        ]
        .map(N::EarlierLayout),
    );
    for notice in &all {
        match notice {
            N::Unsupported(_)
            | N::RecordUnusable(_, Problem::NotUpdated | Problem::Newer)
            | N::Settings(
                Settings::SourceChanged
                | Settings::Failed
                | Settings::Unknown
                | Settings::Busy
                | Settings::PresentationUnavailable
                | Settings::TooSmall
                | Settings::FileBusy
                | Settings::InvalidValue
                | Settings::Unsupported
                | Settings::SyntaxError
                | Settings::ReadOnly
                | Settings::TooLarge
                | Settings::UnusableFile
                | Settings::Unreadable
                | Settings::WriteFailed,
            )
            | N::AgentStart(
                Start::Failed
                | Start::NotFound(_)
                | Start::Exited(_)
                | Start::UnsupportedShell(_)
                | Start::Refused(
                    Refusal::NotAtPrompt
                    | Refusal::Unmarked
                    | Refusal::SessionTooOld
                    | Refusal::TerminalTooOld,
                )
                | Start::Unknown
                | Start::PanelBusy,
            )
            | N::CommandRefused(
                F::Denied
                | F::InvalidIntent
                | F::SessionMismatch
                | F::Unavailable
                | F::Busy
                | F::Conflict
                | F::NotFound
                | F::StaleSource
                | F::PhysicalUnknown,
            )
            | N::AgentNotResumed(_, Why::PromptUnconfirmed | Why::TerminalTooOld)
            | N::EarlierLayout(
                Layout::Reopened { .. }
                | Layout::NothingToReopen
                | Layout::NotFound
                | Layout::ResourceLimit
                | Layout::Busy
                | Layout::Denied
                | Layout::Empty
                | Layout::Unknown
                | Layout::Unavailable,
            )
            | N::MenuUnavailable
            | N::LocalBusy
            | N::NoTabAtPosition(_)
            | N::CreateNotCreated
            | N::CreateOrderConflict
            | N::LaunchRequestTooLarge
            | N::DuplicatePending
            | N::DuplicateNotStarted
            | N::DuplicateOutcomeUnknown
            | N::MoveNotMoved
            | N::MoveOrderConflict
            | N::PanelNotApplied
            | N::RenameNotRenamed
            | N::TerminalClearing
            | N::TerminalCleared
            | N::TerminalClearUnconfirmed
            | N::TerminalClearRefused
            | N::TerminalClearUnavailable
            | N::TerminalClearUnknown
            | N::ScrollbackEditorUnavailable
            | N::ScrollbackEditorTooSmall
            | N::CloseRequestNotApplied
            | N::WorkspaceCloseUnavailable
            | N::WorkspaceChanged
            | N::AgentUnavailable
            | N::NoWorkspace
            | N::WorkspaceNotReady
            | N::ConfirmationCancelled
            | N::DraftCancelled
            | N::DialogRefused
            | N::ModalOwnsInput
            | N::HistoryRecovering
            | N::HistoryUnavailable
            | N::HistoryPositionLost
            | N::CopyModeUnavailable
            | N::CopyModeBusy
            | N::CopyModeInvalidated
            | N::CopyModeQueryFailed
            | N::CopyModeNoMatch
            | N::CopyModeSearchLimit
            | N::CopyModeNoSelection
            | N::FindUnavailable
            | N::FindStopped
            | N::FindFailed
            | N::SelectionEvicted
            | N::CopyFailed
            | N::CopyUnknown
            | N::BellFailed
            | N::BellUnknown
            | N::PositionChanged
            | N::PositionChangedEmpty
            | N::PositionLocationChanged
            | N::PositionLocationChangedEmpty
            | N::PositionUnavailable
            | N::PopupUnavailable
            | N::CommandAccepted
            | N::CommandFailed
            | N::CommandUnknown
            | N::LaunchProfileSelected
            | N::LaunchProfileUnavailable
            | N::RestoreDirectoryUnavailable
            | N::RestoreAgentNotResumed
            | N::RestoreNotStarted
            | N::LinkOpened
            | N::LinkHandled
            | N::LinkNoLink
            | N::LinkBusy
            | N::LinkFailed
            | N::LinkUnknown => {}
        }
    }
    all
}

/// Every Client notice, with the widest value of each count. The exhaustive match at the end fails
/// to compile for a new variant until this list names it.
fn every_client_notice() -> Vec<Notice> {
    use crate::machine_refusal::Refusal;
    use BackendAction as Action;

    let mut all = vec![
        Notice::OnboardingSaveFailed,
        Notice::WorkspaceNavigationUnavailable,
        Notice::NoNotificationTarget,
        Notice::NavigatorChanged,
        Notice::NavigatorTooSmall,
        Notice::ResizeBusy,
        Notice::ResizeFailed,
        Notice::ResizeUnknown,
        Notice::ResizeContended,
        Notice::ResizeNoSplit,
        Notice::ResizeLimit,
        Notice::CheckpointRecovered,
        Notice::CheckpointEmpty,
        Notice::ConfigurationPublished,
        Notice::ConfigurationWarning,
        Notice::SettingsFileUnusable,
        Notice::ConfigurationFailed,
        Notice::ConfigurationUnknown,
        Notice::WindowTitleUnavailable,
        Notice::WindowTitleUnsupported,
        Notice::NotificationUnavailable,
        Notice::NotificationUnsupported,
        Notice::TerminalInputUnavailable,
        Notice::NavigationInputHeld,
        Notice::UnknownPrefix,
        Notice::PointerInputDropped,
        Notice::PasswordNotSaved,
    ];
    all.extend(
        [
            Refusal::OtherEnvironment,
            Refusal::ClientTooOld,
            Refusal::SessionTooOld,
            Refusal::DevelopmentBuildMismatch,
        ]
        .map(Notice::MachineRefused),
    );
    all.extend(
        [
            Action::ReloadConfiguration,
            Action::CreateWorkspace,
            Action::QuickCreateWorkspace,
            Action::CreateWorktree,
            Action::OpenWorktree,
            Action::CloseWorkspace,
            Action::ConfigureWorkspaceDirectory,
            Action::RenameWorkspace,
            Action::RenameProject,
            Action::RenameTab,
            Action::CloseTab,
            Action::MovePreviousTab,
            Action::MoveNextTab,
            Action::CreateTab,
            Action::SelectNextTab,
            Action::SelectPreviousTab,
            Action::SelectTab(u8::MAX),
            Action::SelectPreviousContext,
            Action::SelectNextContext,
            Action::SplitPanel,
            Action::ClosePanel,
            Action::MovePanel,
        ]
        .map(Notice::BackendUnavailable),
    );
    for notice in &all {
        match notice {
            Notice::MachineRefused(
                Refusal::OtherEnvironment
                | Refusal::ClientTooOld
                | Refusal::SessionTooOld
                | Refusal::DevelopmentBuildMismatch,
            )
            | Notice::BackendUnavailable(
                Action::ReloadConfiguration
                | Action::CreateWorkspace
                | Action::QuickCreateWorkspace
                | Action::CreateWorktree
                | Action::OpenWorktree
                | Action::CloseWorkspace
                | Action::ConfigureWorkspaceDirectory
                | Action::RenameWorkspace
                | Action::RenameProject
                | Action::RenameTab
                | Action::CloseTab
                | Action::MovePreviousTab
                | Action::MoveNextTab
                | Action::CreateTab
                | Action::SelectNextTab
                | Action::SelectPreviousTab
                | Action::SelectTab(_)
                | Action::SelectPreviousContext
                | Action::SelectNextContext
                | Action::SplitPanel
                | Action::ClosePanel
                | Action::MovePanel,
            )
            | Notice::OnboardingSaveFailed
            | Notice::WorkspaceNavigationUnavailable
            | Notice::NoNotificationTarget
            | Notice::NavigatorChanged
            | Notice::NavigatorTooSmall
            | Notice::ResizeBusy
            | Notice::ResizeFailed
            | Notice::ResizeUnknown
            | Notice::ResizeContended
            | Notice::ResizeNoSplit
            | Notice::ResizeLimit
            | Notice::CheckpointRecovered
            | Notice::CheckpointEmpty
            | Notice::ConfigurationPublished
            | Notice::ConfigurationWarning
            | Notice::SettingsFileUnusable
            | Notice::ConfigurationFailed
            | Notice::ConfigurationUnknown
            | Notice::WindowTitleUnavailable
            | Notice::WindowTitleUnsupported
            | Notice::NotificationUnavailable
            | Notice::NotificationUnsupported
            | Notice::TerminalInputUnavailable
            | Notice::NavigationInputHeld
            | Notice::UnknownPrefix
            | Notice::PointerInputDropped
            | Notice::PasswordNotSaved => {}
        }
    }
    all
}

/// The status chip's row on a host `columns` wide with no side panel, as the shell places it.
fn painted(
    view: &ClientView,
    columns: u16,
    facts: StatusFacts<'_>,
    locale: Locale,
    theme: &Theme,
) -> String {
    let host = Rect::new(0, 0, columns, COMPACT_MINIMUM_ROWS);
    let areas = project_shell_areas(host, SidebarSurface::Hidden);
    let chip = status_chip_area(&areas);
    let mut terminal = Terminal::new(TestBackend::new(host.width, host.height)).unwrap();
    terminal
        .draw(|frame| {
            render(
                frame, chip, view, None, locale, facts, areas.tier, theme, None,
            );
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    let mut row = String::new();
    let mut x = 0;
    while x < host.width {
        let symbol = buffer[(x, chip.y)].symbol();
        row.push_str(symbol);
        x += u16::try_from(display_width(symbol).max(1)).unwrap();
    }
    row
}

/// Whether `row` shows one of `forms` whole, or, with `cut`, the start of the shortest form ended
/// with the ellipsis after `marker`.
fn shows(row: &str, forms: &[String], marker: &str, cut: bool, theme: &Theme) -> bool {
    if forms.iter().any(|form| row.contains(form.as_str())) {
        return true;
    }
    let shown = row.trim();
    let shown = shown.strip_prefix(marker).unwrap_or(shown);
    cut && shown
        .strip_suffix(theme.glyphs.ellipsis)
        .is_some_and(|start| {
            !start.trim().is_empty() && forms.last().is_some_and(|form| form.starts_with(start))
        })
}

#[test]
fn every_notice_shows_whole_at_the_compact_minimum_and_is_cut_below_it() {
    let theme = Theme::rgb_test_dark();
    let marker = format!("{} ", theme.glyphs.status_bullet);
    for locale in [Locale::EnUs, Locale::ZhCn] {
        // A Workbench notice beside another status fact leads with its marker, its narrowest
        // presentation; a refused dialog's notice takes the same place.
        let mut marked = every_workbench_notice()
            .into_iter()
            .map(|notice| {
                (
                    format!("{notice:?}"),
                    locale.native_notice_forms(notice, None),
                )
            })
            .collect::<Vec<_>>();
        let shell = locale.shell();
        marked.push((
            "overlay refused".to_owned(),
            vec![
                shell.overlay_refused.to_owned(),
                shell.overlay_refused_short.to_owned(),
            ],
        ));
        for (columns, cut) in [
            (COMPACT_MINIMUM_COLUMNS, false),
            (TOO_SMALL_MINIMUM_COLUMNS, true),
        ] {
            let view = ClientView::new(columns);
            for (name, forms) in &marked {
                let facts = StatusFacts {
                    secondary_notice: forms.first().map(String::as_str),
                    secondary_shorter: forms.get(1..).unwrap_or_default(),
                    ..facts("fact")
                };
                let row = painted(&view, columns, facts, locale, &theme);
                assert!(
                    shows(&row, forms, &marker, cut, &theme),
                    "{locale:?} {name} at {columns} columns: {row:?}"
                );
            }
            // A Client notice replaces the fact it would follow.
            for notice in every_client_notice() {
                let mut view = ClientView::new(columns);
                view.startup_notice(notice);
                let forms = std::iter::once(locale.shell_notice(Some(notice)))
                    .chain(locale.shell_notice_shorter(notice))
                    .map(str::to_owned)
                    .collect::<Vec<_>>();
                let row = painted(&view, columns, facts("fact"), locale, &theme);
                assert!(
                    shows(&row, &forms, "", cut, &theme),
                    "{locale:?} {notice:?} at {columns} columns: {row:?}"
                );
            }
        }
    }
}
