//! A Panel a restore held because its saved directory was unavailable shows a placeholder with
//! Retry, Start in home folder and Close. This owner keeps the placeholder's keyboard focus, sends
//! one restore request at a time and answers its outcome with a short notice. The Server decides
//! whether the directory is back; this Client never retries on its own.

use super::{ClientActionDisposition, WorkbenchClientUi};
use crate::{
    local_client::{LocalClientOrigin, subscription::Request},
    localization::{HeldRestoreAction, WorkbenchClientNotice},
    tui_hit_map::HeldRestoreTarget,
    workbench_state::{ClientPhaseKind, WorkbenchClientState},
};
use orchbee_workbench_protocol::{
    ProtocolVersion, TerminalPresentationId, WorkbenchAction, WorkbenchCommand,
    WorkbenchCommandOutcome, WorkbenchCommandUnavailableReason, WorkbenchPanelId, WorkbenchRequest,
    WorkbenchRequestId, WorkbenchResponse, WorkbenchRestoreDirectory, WorkbenchTabId,
    WorkbenchTerminalContextId, WorkbenchTerminalOutcome,
};
use std::time::{Duration, Instant};

/// How long one restore waits for its reply; the launch itself runs inside it.
const REPLY: Duration = Duration::from_secs(30);
/// How long an outcome notice shows.
const NOTICE: Duration = Duration::from_secs(5);
/// The placeholder's actions in reading order.
const ACTIONS: [HeldRestoreAction; 3] = [
    HeldRestoreAction::Retry,
    HeldRestoreAction::StartInHome,
    HeldRestoreAction::Close,
];

#[derive(Default)]
pub(super) struct HeldRestore {
    origin: Option<LocalClientOrigin>,
    flight: Option<Flight>,
    /// The focused action of the one placeholder that last held keyboard focus.
    focus: Option<(TerminalPresentationId, HeldRestoreAction)>,
    changed: bool,
}

struct Flight {
    request_id: WorkbenchRequestId,
    request: Request,
    presentation: TerminalPresentationId,
    directory: WorkbenchRestoreDirectory,
    agent_resume: bool,
    deadline: Instant,
}

/// The current hold of one Panel in the active Tab.
struct Held {
    terminal_context_id: WorkbenchTerminalContextId,
    tab_id: WorkbenchTabId,
    panel_id: WorkbenchPanelId,
    presentation_id: TerminalPresentationId,
    agent_resume: bool,
}

/// One key the focused placeholder owns.
#[derive(Clone, Copy)]
enum Key {
    Activate,
    Next,
    Previous,
}

impl HeldRestore {
    pub(super) fn set_origin(&mut self, origin: LocalClientOrigin) {
        self.origin = Some(origin);
    }

    /// The action holding keyboard focus on `presentation`'s placeholder.
    pub(super) fn focused(&self, presentation: &TerminalPresentationId) -> HeldRestoreAction {
        self.focus
            .as_ref()
            .filter(|(focused, _)| focused == presentation)
            .map_or(HeldRestoreAction::Retry, |(_, action)| *action)
    }

    /// Whether a restore request is in flight; the placeholder then admits no action.
    pub(super) fn pending(&self, presentation: &TerminalPresentationId) -> bool {
        self.flight
            .as_ref()
            .is_some_and(|flight| &flight.presentation == presentation)
    }

    /// When the flight needs a turn without a reply: its deadline.
    pub(super) fn wake(&self) -> Option<Instant> {
        self.flight.as_ref().map(|flight| flight.deadline)
    }
}

impl WorkbenchClientUi {
    /// A key for the focused placeholder. `None` leaves the bytes to ordinary passthrough, which
    /// refuses them: a held Panel has no terminal.
    pub(super) fn held_restore_key(&mut self, input: &[u8]) -> Option<bool> {
        let panel = self
            .state
            .as_ref()
            .and_then(WorkbenchClientState::focus_panel_id)?
            .clone();
        let held = self.held(&panel)?;
        let keys = keys(input)?;
        for key in keys {
            let current = self.held_restore.focused(&held.presentation_id);
            let index = ACTIONS
                .iter()
                .position(|action| *action == current)
                .unwrap_or(0);
            match key {
                Key::Next | Key::Previous => {
                    let step = if matches!(key, Key::Next) {
                        1
                    } else {
                        ACTIONS.len() - 1
                    };
                    let action = ACTIONS[(index + step) % ACTIONS.len()];
                    self.held_restore.focus = Some((held.presentation_id.clone(), action));
                    self.held_restore.changed = true;
                }
                Key::Activate => {
                    let _disposition = self.run_held_restore(&panel, current);
                    break;
                }
            }
        }
        Some(true)
    }

    /// Activates one painted placeholder action while its Presentation is still held there.
    pub(super) fn activate_held_restore(
        &mut self,
        target: &HeldRestoreTarget,
    ) -> ClientActionDisposition {
        let current = self
            .held(&target.panel)
            .is_some_and(|held| held.presentation_id == target.presentation);
        if !current {
            return ClientActionDisposition::KeepNavigator;
        }
        self.held_restore.focus = Some((target.presentation.clone(), target.action));
        self.run_held_restore(&target.panel, target.action)
    }

    fn run_held_restore(
        &mut self,
        panel: &WorkbenchPanelId,
        action: HeldRestoreAction,
    ) -> ClientActionDisposition {
        let directory = match action {
            HeldRestoreAction::Retry => WorkbenchRestoreDirectory::Saved,
            HeldRestoreAction::StartInHome => WorkbenchRestoreDirectory::Home,
            // Close is the ordinary Panel close of this Panel.
            HeldRestoreAction::Close => {
                let focused = |ui: &Self| {
                    ui.state
                        .as_ref()
                        .and_then(WorkbenchClientState::focus_panel_id)
                        == Some(panel)
                };
                if !focused(self) {
                    let _disposition = self.structure_action(
                        super::held_intent::StructureAction::FocusPanel(panel.clone()),
                    );
                }
                return if focused(self) {
                    self.request_panel_close_confirmation()
                } else {
                    ClientActionDisposition::KeepNavigator
                };
            }
        };
        self.send_held_restore(panel, directory)
    }

    fn send_held_restore(
        &mut self,
        panel: &WorkbenchPanelId,
        directory: WorkbenchRestoreDirectory,
    ) -> ClientActionDisposition {
        if self.held_restore.flight.is_some() {
            return ClientActionDisposition::KeepNavigator;
        }
        let Some(held) = self.held(panel) else {
            return ClientActionDisposition::KeepNavigator;
        };
        let now = self.clock.now();
        let (Some(state), Some(origin), Some(Ok(request_id))) = (
            self.state.as_ref(),
            self.held_restore.origin.as_ref(),
            self.mailbox
                .as_ref()
                .map(|mailbox| mailbox.allocate_workbench_request_id()),
        ) else {
            self.note_held_restore(WorkbenchClientNotice::RestoreNotStarted, now);
            return ClientActionDisposition::KeepNavigator;
        };
        let command = WorkbenchCommand {
            protocol_version: ProtocolVersion,
            request_id: request_id.clone(),
            session_id: state.cache().session_id().clone(),
            expected_revision: state.cache().revision(),
            action: WorkbenchAction::PanelRestoreTerminal {
                terminal_context_id: held.terminal_context_id,
                tab_id: held.tab_id,
                panel_id: held.panel_id,
                presentation_id: held.presentation_id.clone(),
                directory,
            },
        };
        let deadline = now + REPLY;
        match origin.request(
            &WorkbenchRequest::Command(command),
            deadline,
            self.clock.clone(),
        ) {
            Ok(request) => {
                self.held_restore.flight = Some(Flight {
                    request_id,
                    request,
                    presentation: held.presentation_id,
                    directory,
                    agent_resume: held.agent_resume,
                    deadline,
                });
                self.held_restore.changed = true;
            }
            Err(_) => self.note_held_restore(WorkbenchClientNotice::RestoreNotStarted, now),
        }
        ClientActionDisposition::KeepNavigator
    }

    /// Settles the flight's reply and reports whether the placeholder needs a repaint.
    pub(super) fn drive_held_restore(&mut self) -> bool {
        let mut changed = std::mem::take(&mut self.held_restore.changed);
        let Some(flight) = self.held_restore.flight.as_ref() else {
            return changed;
        };
        let Some(reply) = flight.request.poll() else {
            return changed;
        };
        let flight = self
            .held_restore
            .flight
            .take()
            .expect("a polled flight is present");
        changed = true;
        let notice = match reply {
            Ok(WorkbenchResponse::Command(result)) if result.request_id == flight.request_id => {
                match result.outcome {
                    WorkbenchCommandOutcome::PanelTerminalCreated { .. }
                        if flight.directory == WorkbenchRestoreDirectory::Home
                            && flight.agent_resume =>
                    {
                        Some(WorkbenchClientNotice::RestoreAgentNotResumed)
                    }
                    // A failed launch shows its own guidance in the Panel.
                    WorkbenchCommandOutcome::PanelTerminalCreated {
                        terminal_outcome:
                            WorkbenchTerminalOutcome::Started { .. }
                            | WorkbenchTerminalOutcome::Failed { .. },
                        ..
                    } => None,
                    WorkbenchCommandOutcome::Unavailable {
                        reason: WorkbenchCommandUnavailableReason::DirectoryUnavailable,
                    } => Some(WorkbenchClientNotice::RestoreDirectoryUnavailable),
                    _ => Some(WorkbenchClientNotice::RestoreNotStarted),
                }
            }
            Ok(_) | Err(_) => Some(WorkbenchClientNotice::RestoreNotStarted),
        };
        if let Some(notice) = notice {
            self.note_held_restore(notice, self.clock.now());
        }
        changed
    }

    fn note_held_restore(&mut self, notice: WorkbenchClientNotice, now: Instant) {
        self.command_notice.set(Some((notice, now + NOTICE)));
    }

    /// The hold of `panel` in the active Tab, while this Client may act on it.
    fn held(&self, panel: &WorkbenchPanelId) -> Option<Held> {
        if self.retirement_failure.is_some() {
            return None;
        }
        let state = self.state.as_ref()?;
        if !matches!(
            state.phase_kind(),
            ClientPhaseKind::Ready | ClientPhaseKind::SelectedWithoutAttachment
        ) {
            return None;
        }
        let active = state.active()?;
        let tab = state.cache().tabs().get(active.index())?;
        let panel = tab.panel(tab.panel_index(panel)?)?;
        let member = state.selected_presentation(panel);
        let (_, agent_resume) = member.held_directory()?;
        Some(Held {
            terminal_context_id: state.cache().terminal_context_id()?.clone(),
            tab_id: tab.tab_id().clone(),
            panel_id: panel.panel_id().clone(),
            presentation_id: member.presentation_id().clone(),
            agent_resume,
        })
    }
}

/// The placeholder's keys in `input`, or `None` when any byte means something else.
fn keys(mut input: &[u8]) -> Option<Vec<Key>> {
    const TABLE: [(&[u8], Key); 10] = [
        (b"\r\n", Key::Activate),
        (b"\r", Key::Activate),
        (b"\n", Key::Activate),
        (b" ", Key::Activate),
        (b"\t", Key::Next),
        (b"\x1b[C", Key::Next),
        (b"\x1bOC", Key::Next),
        (b"\x1b[Z", Key::Previous),
        (b"\x1b[D", Key::Previous),
        (b"\x1bOD", Key::Previous),
    ];
    let mut keys = Vec::new();
    while !input.is_empty() {
        let (bytes, key) = TABLE.iter().find(|(bytes, _)| input.starts_with(bytes))?;
        keys.push(*key);
        input = &input[bytes.len()..];
    }
    (!keys.is_empty()).then_some(keys)
}
