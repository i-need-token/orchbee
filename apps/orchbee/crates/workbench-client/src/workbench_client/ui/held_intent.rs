//! Holds one user invocation whose admission needs the refresh slot while a background
//! publication read owns it, or while the user's own switch is still landing, and replays it
//! through its ordinary entry point once that settles, before another background read starts.
//! Background work and a landing switch never make the action unavailable and never drop it: a
//! hold that outlives its bound ends with the visible busy refusal.

use super::WorkbenchClientUi;
use super::worktree_launch::WorktreeMode;
use crate::client_view::{ClientAction, ClientActionDisposition, ClientKey, PanelDirection};
use crate::workbench_state::workspace_close::{WorkspaceCloseToken, WorkspaceDeleteRequest};
use crate::workbench_state::{ClientNotice, PanelRequestKind, WorkbenchClientState};
use orchbee_workbench_protocol::{
    WorkbenchPanelId, WorkbenchTabMoveDirection, WorkbenchWorkspaceId,
};
use std::time::{Duration, Instant};

/// A background read settles within one read round trip, and a switch within its terminal's
/// attach, which takes a few seconds on a loaded host. A hold past this bound no longer reads as
/// a response to the key, so it ends with a refusal instead.
const HOLD: Duration = Duration::from_secs(5);

/// An invocation whose effect waits for the refresh slot.
#[derive(Clone, Debug)]
pub(super) enum HeldIntent {
    /// The drawn Workspace or Project close confirmation was confirmed.
    WorkspaceClose(WorkspaceCloseToken),
    /// A conditional Workspace close request, rebased on replay.
    WorkspaceDelete(Box<WorkspaceDeleteRequest>),
    /// Close of a named Workspace requested before a landing switch let it be captured.
    WorkspaceCloseAt(WorkbenchWorkspaceId),
    /// Close of the current Workspace: the one the landing switch lands on, resolved on replay.
    CurrentWorkspaceClose,
    /// A Worktree form or listing was requested.
    Worktree(WorktreeMode),
    /// The next or previous Context was requested. Terminal input waits with it.
    Context { next: bool },
    /// A Tab or Panel action of the current Workspace, resolved against the landed projection.
    Structure(StructureAction),
}

/// A Tab or Panel action of the current Workspace. Its admission needs a settled projection, so
/// a background read or the user's own landing switch holds it instead of refusing it.
#[derive(Clone, Debug)]
pub(super) enum StructureAction {
    Zoom,
    /// Terminal input waits with a held focus move, as it does with a held Context step.
    Focus(PanelDirection),
    FocusPanel(WorkbenchPanelId),
    /// `SelectTab`, `SelectNextTab` or `SelectPreviousTab`; terminal input waits with it.
    SelectTab(ClientAction),
    MoveTab(WorkbenchTabMoveDirection),
    Panel(PanelRequestKind),
    CreateTab,
}

struct Hold {
    intent: HeldIntent,
    until: Instant,
}

/// The single held invocation and the bounded pause it imposes on background reads.
#[derive(Default)]
pub(super) struct HeldIntents {
    held: Option<Hold>,
    /// A user navigation retries behind the read in flight; no further read may overtake it.
    navigation_until: Option<Instant>,
    /// A replayed action changed the drawn panes since the Shell last re-observed them.
    layout_replayed: bool,
}

impl WorkbenchClientUi {
    /// Holds `intent` behind the background read that owns the refresh slot, or behind the
    /// landing switch. A later invocation replaces the earlier one, as a second key press would.
    pub(super) fn hold_intent(&mut self, intent: HeldIntent) {
        let until = self.clock.now() + HOLD;
        self.held_intents.held = Some(Hold { intent, until });
    }

    /// Escape on the confirmation also withdraws its held confirmation.
    pub(super) fn release_held_workspace_close(&mut self, token: WorkspaceCloseToken) {
        if self.held_intents.held.as_ref().is_some_and(
            |held| matches!(held.intent, HeldIntent::WorkspaceClose(held) if held == token),
        ) {
            self.held_intents.held = None;
        }
    }

    /// A user navigation found only a background read in its way and retries next tick.
    pub(super) fn yield_background_reads_to_navigation(&mut self) {
        if self.held_intents.navigation_until.is_none() {
            self.held_intents.navigation_until = Some(self.clock.now() + HOLD);
        }
    }

    pub(super) fn end_navigation_yield(&mut self) {
        self.held_intents.navigation_until = None;
    }

    /// Background reads wait while a held invocation or retrying navigation needs the slot.
    pub(super) fn background_reads_yield(&self) -> bool {
        self.held_intents.held.is_some()
            || self
                .held_intents
                .navigation_until
                .is_some_and(|until| self.clock.now() < until)
    }

    /// Whether a requested Context, Tab or Panel focus change is held: input belongs to no
    /// terminal until it lands.
    pub(super) fn held_navigation(&self) -> bool {
        self.held_intents.held.as_ref().is_some_and(|held| {
            matches!(
                held.intent,
                HeldIntent::Context { .. }
                    | HeldIntent::Structure(
                        StructureAction::Focus(_)
                            | StructureAction::FocusPanel(_)
                            | StructureAction::SelectTab(_)
                    )
            )
        })
    }

    /// Runs a Tab or Panel action, or holds it while a background read or the user's own landing
    /// switch takes the refresh slot that its admission needs.
    pub(super) fn structure_action(&mut self, action: StructureAction) -> ClientActionDisposition {
        if !self
            .state
            .as_ref()
            .is_some_and(WorkbenchClientState::settling_for_user_action)
        {
            return self.apply_structure_action(action);
        }
        // Nothing is drawn differently until the replay; a request answers the key at once.
        let disposition = match action {
            StructureAction::Zoom | StructureAction::Focus(_) | StructureAction::FocusPanel(_) => {
                ClientActionDisposition::KeepNavigator
            }
            StructureAction::SelectTab(_)
            | StructureAction::MoveTab(_)
            | StructureAction::Panel(_)
            | StructureAction::CreateTab => ClientActionDisposition::DismissNavigator,
        };
        self.hold_intent(HeldIntent::Structure(action));
        disposition
    }

    fn apply_structure_action(&mut self, action: StructureAction) -> ClientActionDisposition {
        match action {
            StructureAction::Zoom => self.toggle_zoom(),
            StructureAction::Focus(direction) => self.move_panel_focus(direction),
            StructureAction::FocusPanel(panel) => self.move_panel_focus_to(&panel),
            StructureAction::SelectTab(action) => self.select_tab(action),
            StructureAction::MoveTab(direction) => self.move_tab(direction),
            StructureAction::Panel(kind) => self.panel_operation(kind),
            StructureAction::CreateTab => self.create_tab(),
        }
    }

    /// Whether a replayed action changed the drawn panes; the Shell then re-observes geometry and
    /// the pointer map, as it does after a dispatched one.
    pub(in crate::workbench_client) fn take_replayed_layout_change(&mut self) -> bool {
        std::mem::take(&mut self.held_intents.layout_replayed)
    }

    /// Replays the held invocation through its ordinary entry point once the background read
    /// or the landing switch settled. Replay revalidates everything; it never reuses an earlier
    /// admission.
    pub(super) fn replay_held_intent(&mut self) -> bool {
        let Some(held) = self.held_intents.held.as_ref() else {
            return false;
        };
        if self
            .state
            .as_ref()
            .is_some_and(WorkbenchClientState::settling_for_user_action)
        {
            if self.clock.now() < held.until {
                return false;
            }
            self.held_intents.held = None;
            if let Some(state) = self.state.as_mut() {
                state.set_notice(ClientNotice::LocalBusy);
            }
            return true;
        }
        let Some(Hold { intent, .. }) = self.held_intents.held.take() else {
            return false;
        };
        match intent {
            HeldIntent::WorkspaceClose(token) => {
                self.capture_workspace_close_key(token, ClientKey::Enter);
            }
            HeldIntent::WorkspaceDelete(request) => {
                let _disposition = self.request_sidebar_workspace_delete(&request);
            }
            HeldIntent::WorkspaceCloseAt(workspace_id) => {
                let _disposition = self.request_workspace_close_at(&workspace_id);
            }
            HeldIntent::CurrentWorkspaceClose => {
                let _disposition = self.request_workspace_close();
            }
            HeldIntent::Worktree(mode) => {
                let _disposition = self.request_worktree(mode);
            }
            HeldIntent::Context { next } => {
                let _disposition = self.select_context(next);
            }
            HeldIntent::Structure(action) => {
                let disposition = self.apply_structure_action(action);
                self.held_intents.layout_replayed |= disposition.changes_layout();
            }
        }
        true
    }
}
