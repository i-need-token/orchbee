//! Earlier layouts: lists the layouts the Server kept, opens the picker over that listing and
//! reopens the chosen one. One request is in flight at a time; a reopen is sent once and an
//! unconfirmed outcome is reported, never repeated. After a reopen this interactive Client selects
//! the first reopened Tab once the Server revision that holds it has landed.

use super::held_intent::StructureAction;
use super::{ClientActionDisposition, WorkbenchClientUi};
use crate::{
    action_menu::{MenuAction, MenuTarget},
    client_view::ClientAction,
    local_client::{LocalClientOrigin, subscription::Request},
    localization::{EarlierLayoutNotice, WorkbenchClientNotice},
    workbench_projection::project_native_context_tabs,
};
use orchbee_workbench_protocol::{
    LayoutHistoryAction, LayoutHistoryEntry, LayoutHistoryFocus, LayoutHistoryOutcome,
    ProtocolVersion, WorkbenchLayoutHistoryRequest, WorkbenchRequest, WorkbenchResponse,
    WorkbenchRevision,
};
use std::time::{Duration, Instant};

/// How long one request waits for its reply; a reopen's launches run inside it.
const REPLY: Duration = Duration::from_secs(30);
/// How long an outcome notice shows.
const NOTICE: Duration = Duration::from_secs(5);
/// How long the first reopened Tab waits for its revision before selection is given up.
const FOCUS: Duration = Duration::from_secs(5);

#[derive(Default)]
pub(super) struct EarlierLayouts {
    origin: Option<LocalClientOrigin>,
    flight: Option<Flight>,
    /// The listing the picker shows, newest first.
    entries: Vec<LayoutHistoryEntry>,
    focus: Option<PendingFocus>,
}

struct Flight {
    request: WorkbenchLayoutHistoryRequest,
    reply: Request,
    deadline: Instant,
}

struct PendingFocus {
    revision: WorkbenchRevision,
    focus: LayoutHistoryFocus,
    deadline: Instant,
}

impl EarlierLayouts {
    pub(super) fn set_origin(&mut self, origin: LocalClientOrigin) {
        self.origin = Some(origin);
    }

    /// The picker's rows, one per listed layout.
    pub(super) fn rows(&self) -> Vec<MenuAction> {
        self.entries
            .iter()
            .cloned()
            .map(MenuAction::ReopenEarlierLayout)
            .collect()
    }

    pub(super) fn wake(&self) -> Option<Instant> {
        let flight = self.flight.as_ref().map(|flight| flight.deadline);
        let focus = self.focus.as_ref().map(|focus| focus.deadline);
        flight.into_iter().chain(focus).min()
    }
}

impl WorkbenchClientUi {
    pub(super) fn request_earlier_layouts(&mut self) -> ClientActionDisposition {
        self.send_earlier_layouts(LayoutHistoryAction::List {})
    }

    pub(super) fn reopen_earlier_layout(&mut self, saved_at: u64) -> ClientActionDisposition {
        self.send_earlier_layouts(LayoutHistoryAction::Reopen { saved_at })
    }

    fn send_earlier_layouts(&mut self, action: LayoutHistoryAction) -> ClientActionDisposition {
        if self.earlier_layouts.flight.is_some() {
            return ClientActionDisposition::KeepNavigator;
        }
        let now = self.clock.now();
        let (Some(state), Some(origin), Some(Ok(request_id))) = (
            self.state.as_ref(),
            self.earlier_layouts.origin.as_ref(),
            self.mailbox
                .as_ref()
                .map(|mailbox| mailbox.allocate_workbench_request_id()),
        ) else {
            self.note_earlier_layouts(EarlierLayoutNotice::Unavailable, now);
            return ClientActionDisposition::KeepNavigator;
        };
        let request = WorkbenchLayoutHistoryRequest {
            protocol_version: ProtocolVersion,
            request_id,
            session_id: state.cache().session_id().clone(),
            action,
        };
        let deadline = now + REPLY;
        match origin.request(
            &WorkbenchRequest::LayoutHistory(request.clone()),
            deadline,
            self.clock.clone(),
        ) {
            Ok(reply) => {
                self.earlier_layouts.flight = Some(Flight {
                    request,
                    reply,
                    deadline,
                });
            }
            Err(_) => self.note_earlier_layouts(EarlierLayoutNotice::Unavailable, now),
        }
        ClientActionDisposition::KeepNavigator
    }

    /// Settles the flight's reply and a pending selection; reports whether anything changed.
    pub(super) fn drive_earlier_layouts(&mut self) -> bool {
        let changed = self.settle_earlier_layouts_reply();
        changed | self.select_reopened_tab()
    }

    fn settle_earlier_layouts_reply(&mut self) -> bool {
        let Some(reply) = self
            .earlier_layouts
            .flight
            .as_ref()
            .and_then(|flight| flight.reply.poll())
        else {
            return false;
        };
        let flight = self
            .earlier_layouts
            .flight
            .take()
            .expect("a polled flight is present");
        let now = self.clock.now();
        let listing = matches!(flight.request.action, LayoutHistoryAction::List {});
        let result = match reply {
            Ok(WorkbenchResponse::LayoutHistory(result))
                if result.answers(&flight.request) && result.validate().is_ok() =>
            {
                result
            }
            // A list is only a read; a reopen may have been applied, so it is never repeated.
            _ => {
                let notice = if listing {
                    EarlierLayoutNotice::Unavailable
                } else {
                    EarlierLayoutNotice::Unknown
                };
                self.note_earlier_layouts(notice, now);
                return true;
            }
        };
        match result.outcome {
            LayoutHistoryOutcome::Listed { entries } if entries.is_empty() => {
                self.note_earlier_layouts(EarlierLayoutNotice::Empty, now);
            }
            LayoutHistoryOutcome::Listed { entries } => {
                self.earlier_layouts.entries = entries;
                if !self.open_action_menu(MenuTarget::EarlierLayouts, None, false) {
                    self.command_notice
                        .set(Some((WorkbenchClientNotice::MenuUnavailable, now + NOTICE)));
                }
            }
            outcome => {
                if let LayoutHistoryOutcome::Reopened {
                    revision,
                    focus: Some(focus),
                    ..
                } = &outcome
                {
                    self.earlier_layouts.focus = Some(PendingFocus {
                        revision: *revision,
                        focus: focus.clone(),
                        deadline: now + FOCUS,
                    });
                }
                let notice = EarlierLayoutNotice::from_outcome(&outcome)
                    .unwrap_or(EarlierLayoutNotice::Unavailable);
                self.note_earlier_layouts(notice, now);
            }
        }
        true
    }

    /// Selects the first reopened Tab once its revision has landed, through the ordinary Tab
    /// switch or context selection. A Tab that is gone by then, or a revision that never lands,
    /// selects nothing.
    fn select_reopened_tab(&mut self) -> bool {
        let Some(pending) = self.earlier_layouts.focus.as_ref() else {
            return false;
        };
        if self.clock.now() >= pending.deadline {
            self.earlier_layouts.focus = None;
            return true;
        }
        let Some(state) = self.state.as_ref() else {
            return false;
        };
        if state.cache().revision() < pending.revision
            || self.navigation_pending()
            || self.switch_landing()
        {
            return false;
        }
        let pending = self
            .earlier_layouts
            .focus
            .take()
            .expect("a pending selection is present");
        let LayoutHistoryFocus {
            terminal_context_id,
            tab_id,
        } = pending.focus;
        let current = state.cache().location();
        if current.terminal_context_id() == Some(&terminal_context_id) {
            let Some(index) = state.cache().tab_index(&tab_id) else {
                return true;
            };
            let position = (0..=index).filter(|index| state.tab_shown(*index)).count();
            if let Ok(position) = u8::try_from(position) {
                let _disposition = self.structure_action(StructureAction::SelectTab(
                    ClientAction::SelectTab(position),
                ));
            }
            return true;
        }
        let Some(location) = state
            .cache()
            .navigation()
            .iter()
            .map(|context| &context.location)
            .find(|location| location.terminal_context_id() == Some(&terminal_context_id))
            .cloned()
        else {
            return true;
        };
        let Some(index) = state.display_position(&location) else {
            return true;
        };
        let panel = project_native_context_tabs(state.cache().canonical_snapshot(), &location)
            .ok()
            .and_then(|tabs| {
                let tab = tabs.tabs().get(tabs.tab_index(&tab_id)?)?;
                Some(tab.panel(0)?.panel_id().clone())
            });
        if let (Some(panel), Some(state)) = (panel, self.state.as_mut()) {
            state.prefer_panel(&location, &tab_id, &panel);
        }
        let _disposition = self.select_context_at(index);
        true
    }

    fn note_earlier_layouts(&mut self, notice: EarlierLayoutNotice, now: Instant) {
        self.command_notice.set(Some((
            WorkbenchClientNotice::EarlierLayout(notice),
            now + NOTICE,
        )));
    }
}
