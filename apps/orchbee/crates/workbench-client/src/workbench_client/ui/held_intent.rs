//! Holds one user invocation whose admission needs the refresh slot while a background
//! publication read owns it, or while the user's own switch is still landing, and replays it
//! through its ordinary entry point once that settles, before another background read starts.
//! Background work and a landing switch never make the action unavailable and never drop it: a
//! hold that outlives its bound ends with the visible busy refusal. A switch lands from the
//! request on, including a direct navigation the shell still holds behind an earlier switch or
//! read, so a key pressed meanwhile never acts on the Context being left. A click the Client
//! could not resolve while the switch lands may have chosen another Workspace: until the switch
//! landed and a hold bound passed, a key that names "current" is refused instead of acting on a
//! Workspace the user may not want, once after the landing so that trying again works.
//!
//! A confirmed Tab or Panel close waits the same way while another close's status read or
//! exchange, or a refresh, holds the slot its submission needs: its confirmation stays open and
//! armed, and the close goes out as soon as the slot is free, within the same bound.
//!
//! The Move Tab keys are serialized: a press made while this Client's earlier Tab change is still
//! on its way waits as well, and the presses held together run oldest first, one request at a
//! time, each against the order the one before it produced.

use super::WorkbenchClientUi;
use super::worktree_launch::WorktreeMode;
use crate::client_view::{ClientAction, ClientActionDisposition, ClientKey, PanelDirection};
use crate::workbench_projection::{ClientContextLocation, ClientContextTarget};
use crate::workbench_state::workspace_close::{WorkspaceCloseToken, WorkspaceDeleteRequest};
use crate::workbench_state::{
    ClientNotice, CloseConfirmationToken, MAXIMUM_CLOSE_RECORDS, PanelCloseConfirmationToken,
    PanelRequestKind, WorkbenchClientState,
};
use orchbee_workbench_protocol::{
    WorkbenchPanelId, WorkbenchTabId, WorkbenchTabMoveDirection, WorkbenchWorkspaceId,
};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// A background read settles within one read round trip, and a switch within its terminal's
/// attach, which takes a few seconds on a loaded host. A hold past this bound no longer reads as
/// a response to the key, so it ends with a refusal instead.
const HOLD: Duration = Duration::from_secs(5);

/// Tab moves held together. A longer run is no longer something a person follows press by press,
/// so the next press is refused as busy.
const MAXIMUM_HELD_TAB_MOVES: usize = 32;

/// An invocation whose effect waits for the refresh slot, or a confirmed close for the slot its
/// submission needs.
#[derive(Clone, Debug)]
pub(super) enum HeldIntent {
    /// The drawn Workspace or Project close confirmation was confirmed.
    WorkspaceClose(WorkspaceCloseToken),
    /// A conditional Workspace close request, rebased on replay.
    WorkspaceDelete(Box<WorkspaceDeleteRequest>),
    /// Close of a named Workspace requested before a landing switch let it be captured.
    WorkspaceCloseAt(WorkbenchWorkspaceId),
    /// Close of the current Workspace, aimed at the Context the landing switch was headed for when
    /// the key was pressed. It runs only once the switch landed there; anywhere else it is refused.
    CurrentWorkspaceClose(ClientContextLocation),
    /// A Worktree form or listing was requested, for a create from the Start Agent picker with the
    /// Agent its new Workspace's first shell runs.
    Worktree(WorktreeMode, Option<&'static str>),
    /// The next or previous Context was requested. Terminal input waits with it.
    Context { next: bool },
    /// A Tab or Panel action of the current Workspace, resolved against the landed projection.
    Structure(StructureAction),
    /// Move Tab key presses, oldest first. Each runs once the change before it is installed.
    TabMoves(VecDeque<WorkbenchTabMoveDirection>),
    /// The drawn Tab or Panel close confirmation was confirmed.
    Close(ConfirmedClose),
    /// Last Panel, whose target is chosen from the focus history once the projection settled.
    LastPanel,
    /// A notification's Agent target, revalidated in full when it replays. Terminal input waits
    /// with it, as it does with a held focus move.
    AgentTarget(Box<crate::machine_agents::Target>),
}

/// The close confirmation whose `Enter` a held close replays.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ConfirmedClose {
    Tab(CloseConfirmationToken),
    Panel(PanelCloseConfirmationToken),
}

/// A deferred Tab or Panel close initiation revalidated against the settled projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CloseRequestIntent {
    DrawnTabClose {
        location: ClientContextLocation,
        tab: WorkbenchTabId,
    },
    TabMenuClose {
        location: ClientContextLocation,
        action: crate::action_menu::MenuAction,
        tab: WorkbenchTabId,
        expected_ids: Vec<WorkbenchTabId>,
    },
    CloseSelectedTab,
    CloseFocusedPanel,
}

struct HeldCloseRequest {
    request: CloseRequestIntent,
    until: Instant,
}

/// What became of one submission of a confirmed close.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CloseSubmission {
    /// The close is on its way.
    Submitted,
    /// Another close's status read or exchange, or a refresh, holds the slot the close needs.
    Waiting,
    /// The confirmation ended without a close; whatever ended it said why.
    Ended,
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
    Panel(PanelRequestKind),
    CreateTab,
    /// A new Tab whose shell runs this Agent as its first command.
    CreateAgentTab(&'static str),
    /// A split of the focused Panel whose new shell runs this Agent as its first command.
    SplitAgent(PanelDirection, &'static str),
}

struct Hold {
    intent: HeldIntent,
    until: Instant,
}

/// The single held invocation and the bounded pause it imposes on background reads.
#[derive(Default)]
pub(super) struct HeldIntents {
    held: Option<Hold>,
    /// Queued close initiations held per target/intent behind transient settling so sequential
    /// clicks or shortcuts never overwrite each other or get dropped by a held navigation.
    close_requests: VecDeque<HeldCloseRequest>,
    /// A user navigation retries behind the read in flight; no further read may overtake it.
    navigation_until: Option<Instant>,
    /// A replayed action changed the drawn panes since the Shell last re-observed them.
    layout_replayed: bool,
    /// The destination of the user's direct navigation that the shell still holds for this
    /// Client because an earlier switch or a read keeps it from admission.
    direct_navigation: Option<ClientContextLocation>,
    /// A click aimed at the frame the landing switch kept on screen reached nothing, so the
    /// switch's destination may not be what the user wants. It stays uncertain while the switch
    /// lands and until this bound, which a person pressing a key meanwhile cannot outrun.
    uncertain_until: Option<Instant>,
}

impl WorkbenchClientUi {
    /// The shell reports the user's direct navigation it still holds for this Client, or `None`
    /// once this Client admitted it or the selection ended.
    pub(in crate::workbench_client) fn observe_direct_navigation(
        &mut self,
        target: Option<&ClientContextTarget>,
    ) {
        self.held_intents.direct_navigation = target.map(|target| target.location.clone());
    }

    /// A pointer click found no presented frame to resolve it. While the user's own switch lands,
    /// the shell keeps the previous frame on screen without targets, so the click was a choice
    /// the Client never saw, such as another Workspace's row: the switch's destination becomes
    /// uncertain. Returns whether that happened, so the caller reports the dropped click.
    pub(in crate::workbench_client) fn unresolved_press(&mut self) -> bool {
        let landing = self.switch_landing();
        if landing {
            self.held_intents.uncertain_until = Some(self.clock.now() + HOLD);
        }
        landing
    }

    /// An accepted close replaced the current Workspace with its neighbour, which the user did not
    /// choose and may not have seen yet. A close of the current Workspace pressed before that is
    /// refused once, within the same bound, so a repeated key never closes the neighbour.
    pub(super) fn mark_destination_uncertain(&mut self) {
        self.held_intents.uncertain_until = Some(self.clock.now() + HOLD);
    }

    fn destination_uncertain(&self) -> bool {
        self.held_intents
            .uncertain_until
            .is_some_and(|until| self.switch_landing() || self.clock.now() < until)
    }

    /// Ends an uncertainty whose switch landed and whose bound passed, so that no later switch
    /// inherits it. Each tick calls this before anything consults the uncertainty.
    fn lapse_destination_uncertainty(&mut self) {
        if !self.switch_landing()
            && self
                .held_intents
                .uncertain_until
                .is_some_and(|until| self.clock.now() >= until)
        {
            self.held_intents.uncertain_until = None;
        }
    }

    /// Whether a "current" target is uncertain once the switch landed; the refusal that answers
    /// it ends the uncertainty, so the user can try again on the Workspace now shown.
    pub(super) fn take_destination_uncertainty(&mut self) -> bool {
        let uncertain = self.destination_uncertain();
        self.held_intents.uncertain_until = None;
        uncertain
    }

    /// Whether the user's own switch has not landed: this Client's switch still reads or
    /// attaches, or a direct navigation still waits for admission.
    pub(super) fn switch_landing(&self) -> bool {
        self.held_intents.direct_navigation.is_some()
            || self
                .state
                .as_ref()
                .is_some_and(WorkbenchClientState::switch_landing)
    }

    /// Where the user's landing switch is headed, or `None` once a press it never saw made that
    /// uncertain. A waiting direct navigation is the newer intent, so it outranks this Client's
    /// own switch in flight.
    pub(super) fn landing_destination(&self) -> Option<ClientContextLocation> {
        if self.destination_uncertain() {
            return None;
        }
        self.held_intents
            .direct_navigation
            .clone()
            .or_else(|| self.state.as_ref()?.landing_destination().cloned())
    }

    /// What a held invocation waits out before it runs through its ordinary entry point.
    pub(super) fn settling_for_user_action(&self) -> bool {
        self.held_intents.direct_navigation.is_some()
            || self
                .state
                .as_ref()
                .is_some_and(WorkbenchClientState::settling_for_user_action)
    }

    /// Holds `intent` behind the background read that owns the refresh slot, or behind the
    /// landing switch. A later invocation replaces the earlier one, as a second key press would.
    pub(super) fn hold_intent(&mut self, intent: HeldIntent) {
        let until = self.clock.now() + HOLD;
        self.held_intents.held = Some(Hold { intent, until });
    }

    /// Queues a close request behind transient settling up to `MAXIMUM_CLOSE_RECORDS`, keeping
    /// distinct targets in FIFO order without colliding with `hold_intent`.
    pub(super) fn hold_close_request(&mut self, request: CloseRequestIntent) -> bool {
        if self
            .held_intents
            .close_requests
            .iter()
            .any(|held| held.request == request)
        {
            return true;
        }
        if self.held_intents.close_requests.len() >= MAXIMUM_CLOSE_RECORDS {
            return false;
        }
        self.held_intents
            .close_requests
            .push_back(HeldCloseRequest {
                request,
                until: self.clock.now() + HOLD,
            });
        true
    }

    /// Drops held close requests when the Client carrier retires so dead deadlines never wake it.
    pub(super) fn clear_held_close_requests(&mut self) {
        self.held_intents.close_requests.clear();
    }

    /// When the held invocation's bound ends, so that its refusal shows even when nothing else
    /// wakes the Client meanwhile.
    pub(super) fn held_intent_wake(&self) -> Option<Instant> {
        self.held_intents
            .held
            .as_ref()
            .map(|held| held.until)
            .into_iter()
            .chain(
                self.held_intents
                    .close_requests
                    .iter()
                    .map(|held| held.until),
            )
            .min()
    }

    /// Escape on the confirmation also withdraws its held confirmation.
    pub(super) fn release_held_workspace_close(&mut self, token: WorkspaceCloseToken) {
        if self.held_intents.held.as_ref().is_some_and(
            |held| matches!(held.intent, HeldIntent::WorkspaceClose(held) if held == token),
        ) {
            self.held_intents.held = None;
        }
    }

    /// Submits the confirmed close, or holds it while another close's status read or exchange, or
    /// a refresh, holds the slot its submission needs. The confirmation stays open and armed while
    /// the close waits.
    pub(super) fn confirm_close_decision(
        &mut self,
        close: ConfirmedClose,
    ) -> ClientActionDisposition {
        match self.submit_confirmed_close(close) {
            CloseSubmission::Submitted => ClientActionDisposition::DismissNavigator,
            CloseSubmission::Waiting => {
                self.hold_intent(HeldIntent::Close(close));
                ClientActionDisposition::KeepNavigator
            }
            CloseSubmission::Ended => ClientActionDisposition::KeepNavigator,
        }
    }

    fn submit_confirmed_close(&mut self, close: ConfirmedClose) -> CloseSubmission {
        // A confirmation whose frozen target drifted while its close waited cancels with its
        // notice instead of closing.
        if let Some(state) = self.state.as_mut() {
            let _settled = state.settle_stale_close_confirmations();
        }
        if !self.close_confirmation_armed(close) {
            return CloseSubmission::Ended;
        }
        match close {
            ConfirmedClose::Tab(_) => self.submit_tab_close(),
            ConfirmedClose::Panel(_) => self.submit_panel_close(),
        }
    }

    /// Whether the confirmation `close` names is still the armed one.
    fn close_confirmation_armed(&self, close: ConfirmedClose) -> bool {
        let Some(view) = self.state.as_ref().map(WorkbenchClientState::view) else {
            return false;
        };
        match close {
            ConfirmedClose::Tab(token) => view
                .close_confirmation()
                .is_some_and(|prompt| prompt.token() == token),
            ConfirmedClose::Panel(token) => view
                .panel_close_confirmation()
                .is_some_and(|prompt| prompt.token() == token),
        }
    }

    /// Cancelling a close confirmation also withdraws the confirmed close it holds; nothing was
    /// sent.
    pub(super) fn release_held_close(&mut self) {
        if self
            .held_intents
            .held
            .as_ref()
            .is_some_and(|held| matches!(held.intent, HeldIntent::Close(_)))
        {
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

    /// Background reads wait while a held invocation, a chosen menu row or retrying navigation
    /// needs the slot.
    pub(super) fn background_reads_yield(&self) -> bool {
        self.held_intents.held.is_some()
            || !self.held_intents.close_requests.is_empty()
            || self.action_menu_awaits_refresh_slot()
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
                    | HeldIntent::LastPanel
                    | HeldIntent::AgentTarget(_)
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
        if !self.settling_for_user_action() {
            return self.apply_structure_action(action);
        }
        // Nothing is drawn differently until the replay; a request answers the key at once.
        let disposition = match action {
            StructureAction::Zoom | StructureAction::Focus(_) | StructureAction::FocusPanel(_) => {
                ClientActionDisposition::KeepNavigator
            }
            StructureAction::SelectTab(_)
            | StructureAction::Panel(_)
            | StructureAction::CreateTab
            | StructureAction::CreateAgentTab(_)
            | StructureAction::SplitAgent(..) => ClientActionDisposition::DismissNavigator,
        };
        self.hold_intent(HeldIntent::Structure(action));
        disposition
    }

    /// A Move Tab key: runs the menu's Move Tab row for the current Tab, or holds the press while
    /// a background read, the user's own landing switch or this Client's earlier Tab change still
    /// settles. Presses held together keep their order and are not replaced by one another, so
    /// fast repeats lose none and the strip only ever shows an order the Server committed.
    pub(super) fn request_tab_move(
        &mut self,
        direction: WorkbenchTabMoveDirection,
    ) -> ClientActionDisposition {
        let until = self.clock.now() + HOLD;
        if let Some(Hold {
            intent: HeldIntent::TabMoves(run),
            until: bound,
        }) = &mut self.held_intents.held
        {
            if run.len() < MAXIMUM_HELD_TAB_MOVES {
                run.push_back(direction);
                *bound = until;
                return ClientActionDisposition::DismissNavigator;
            }
            if let Some(state) = self.state.as_mut() {
                state.set_notice(ClientNotice::LocalBusy);
            }
            return ClientActionDisposition::KeepNavigator;
        }
        if self.tab_move_waits() {
            self.hold_intent(HeldIntent::TabMoves(VecDeque::from([direction])));
            return ClientActionDisposition::DismissNavigator;
        }
        if self.move_tab(direction) {
            ClientActionDisposition::DismissNavigator
        } else {
            ClientActionDisposition::KeepNavigator
        }
    }

    /// What a Move Tab key press waits out: the next move is computed from the installed order.
    fn tab_move_waits(&self) -> bool {
        self.settling_for_user_action()
            || self
                .state
                .as_ref()
                .is_some_and(WorkbenchClientState::tab_configuration_in_flight)
    }

    /// The Tab change the held presses followed did not land and said so; they end with it
    /// instead of acting on an order the user has not seen change.
    pub(super) fn end_held_tab_moves(&mut self) {
        if self
            .held_intents
            .held
            .as_ref()
            .is_some_and(|held| matches!(held.intent, HeldIntent::TabMoves(_)))
        {
            self.held_intents.held = None;
        }
    }

    /// Steps back to the last focused Panel, or holds the step while a background read or the
    /// user's own landing switch takes the refresh slot, so the target is chosen where it lands.
    pub(super) fn request_last_panel(&mut self) -> ClientActionDisposition {
        if !self.settling_for_user_action() {
            return self.focus_last_panel();
        }
        self.hold_intent(HeldIntent::LastPanel);
        ClientActionDisposition::KeepNavigator
    }

    fn apply_structure_action(&mut self, action: StructureAction) -> ClientActionDisposition {
        match action {
            StructureAction::Zoom => self.toggle_zoom(),
            StructureAction::Focus(direction) => self.move_panel_focus(direction),
            StructureAction::FocusPanel(panel) => self.move_panel_focus_to(&panel),
            StructureAction::SelectTab(action) => self.select_tab(action),
            StructureAction::Panel(kind) => self.panel_operation(kind),
            StructureAction::CreateTab => self.create_tab(),
            StructureAction::CreateAgentTab(provider) => self.create_agent_tab(provider),
            StructureAction::SplitAgent(direction, provider) => {
                self.panel_operation_starting(PanelRequestKind::Split { direction }, Some(provider))
            }
        }
    }

    /// Whether a replayed action changed the drawn panes; the Shell then re-observes geometry and
    /// the pointer map, as it does after a dispatched one.
    pub(in crate::workbench_client) fn take_replayed_layout_change(&mut self) -> bool {
        std::mem::take(&mut self.held_intents.layout_replayed)
    }

    /// Replays held invocations through their ordinary entry points once settling completes.
    /// Replay revalidates everything; it never reuses an earlier admission.
    pub(super) fn replay_held_intent(&mut self) -> bool {
        self.lapse_destination_uncertainty();
        let mut changed = false;
        if let Some((until, close, tab_moves)) = self.held_intents.held.as_ref().map(|held| {
            let close = match held.intent {
                HeldIntent::Close(close) => Some(close),
                _ => None,
            };
            let tab_moves = matches!(held.intent, HeldIntent::TabMoves(_));
            (held.until, close, tab_moves)
        }) {
            // A confirmed close waits for its busy slot up to its bound; Tab moves also wait for
            // this Client's Tab change in flight; any other held action waits while the
            // background read or landing switch settles.
            let settling = if tab_moves {
                self.tab_move_waits()
            } else {
                self.settling_for_user_action()
            };
            if settling || (close.is_some() && self.clock.now() >= until) {
                if self.clock.now() >= until {
                    self.held_intents.held = None;
                    // A confirmation that already ended said why; the close it held goes with it.
                    if close.is_none_or(|close| self.close_confirmation_armed(close))
                        && let Some(state) = self.state.as_mut()
                    {
                        state.set_notice(ClientNotice::LocalBusy);
                    }
                    changed = true;
                }
            } else if let Some(Hold { intent, .. }) = self.held_intents.held.take() {
                match intent {
                    HeldIntent::WorkspaceClose(token) => {
                        self.capture_workspace_close_key(token, ClientKey::Enter);
                        changed = true;
                    }
                    HeldIntent::WorkspaceDelete(request) => {
                        let _disposition = self.request_sidebar_workspace_delete(&request);
                        changed = true;
                    }
                    HeldIntent::WorkspaceCloseAt(workspace_id) => {
                        let _disposition = self.request_workspace_close_at(&workspace_id);
                        changed = true;
                    }
                    HeldIntent::CurrentWorkspaceClose(destination) => {
                        if self
                            .state
                            .as_ref()
                            .is_some_and(|state| state.cache().location() == &destination)
                        {
                            // An uncertain destination is refused there, like a key pressed now.
                            let _disposition = self.request_workspace_close();
                        } else if let Some(state) = self.state.as_mut() {
                            // The switch the key aimed at landed elsewhere or not at all, or the
                            // user pressed on a frame the Client could not resolve meanwhile; the
                            // Workspace shown now is not certainly the one the key named.
                            state.set_notice(ClientNotice::WorkspaceChanged);
                        }
                        changed = true;
                    }
                    HeldIntent::Worktree(mode, agent) => {
                        let _disposition = self.request_worktree_starting(mode, agent);
                        changed = true;
                    }
                    HeldIntent::Context { next } => {
                        let _disposition = self.select_context(next);
                        changed = true;
                    }
                    HeldIntent::Structure(action) => {
                        let disposition = self.apply_structure_action(action);
                        self.held_intents.layout_replayed |= disposition.changes_layout();
                        changed = true;
                    }
                    HeldIntent::TabMoves(mut run) => {
                        // A change that was accepted but could not be shown blocks the next one
                        // and says so; the presses behind it end there.
                        let shown = self
                            .state
                            .as_ref()
                            .is_some_and(|state| !state.setup_blocks_structure());
                        if shown
                            && let Some(direction) = run.pop_front()
                            && self.move_tab(direction)
                            && !run.is_empty()
                        {
                            self.held_intents.held = Some(Hold {
                                intent: HeldIntent::TabMoves(run),
                                until,
                            });
                        }
                        changed = true;
                    }
                    HeldIntent::Close(close) => {
                        if self.submit_confirmed_close(close) == CloseSubmission::Waiting {
                            // The slot is still taken: the close keeps waiting under its first
                            // bound, and nothing on screen changed.
                            self.held_intents.held = Some(Hold {
                                intent: HeldIntent::Close(close),
                                until,
                            });
                        } else {
                            changed = true;
                        }
                    }
                    HeldIntent::LastPanel => {
                        let disposition = self.focus_last_panel();
                        self.held_intents.layout_replayed |= disposition.changes_layout();
                        changed = true;
                    }
                    HeldIntent::AgentTarget(target) => {
                        let opening = self.open_notification_target(&target);
                        self.held_intents.layout_replayed |=
                            opening == crate::client_view::notifications::TargetOpening::Opened;
                        changed = true;
                    }
                }
            }
        }
        if self.retirement_failure.is_some() {
            if !self.held_intents.close_requests.is_empty() {
                self.clear_held_close_requests();
                changed = true;
            }
            return changed;
        }
        let now = self.clock.now();
        if self.close_initiation_settling() || self.surface_blocks_close_initiation() {
            let before = self.held_intents.close_requests.len();
            self.held_intents
                .close_requests
                .retain(|entry| now < entry.until);
            if self.held_intents.close_requests.len() < before {
                self.set_close_busy_notice();
                changed = true;
            }
        } else {
            while !self.close_initiation_settling()
                && !self.surface_blocks_close_initiation()
                && let Some(entry) = self.held_intents.close_requests.pop_front()
            {
                match self.replay_close_request_intent(entry.request) {
                    super::tab_close::CloseRequestOutcome::Opened => {
                        self.held_intents.layout_replayed = true;
                        changed = true;
                        break;
                    }
                    super::tab_close::CloseRequestOutcome::Refused => {
                        changed = true;
                    }
                    super::tab_close::CloseRequestOutcome::Held
                    | super::tab_close::CloseRequestOutcome::NotApplicable => {}
                }
            }
        }
        changed
    }
}
