//! Owns packet-level keyboard, pointer, modal, and host-reply routing against current capture
//! claims and the last drawn interaction map. It preserves release and unread-tail ownership
//! without executing backend commands or bypassing feature admission.

use std::time::Instant;

use crate::client_view::{
    ClientAction, ClientActionDisposition, ClientKey, ClientView, KeyCode, KeyKind, Modifiers,
    PrefixInput, ShellCommand,
};
use crate::tui_composition::{
    CaptureOwner, ClientCaptureClaims, WorkbenchCaptureOwner, client_chrome_overlay_allowed,
    project_capture,
};
use crate::tui_hit_map::{FrameHitMap, InteractionTarget, PrimaryContract};
use crate::tui_pointer::{
    CancelledModalEvent, MotionObservability, PointerOutcome, PointerState, ReleaseOwnership,
};
use crate::workbench_state::workspace_setup::SetupStage;

use super::{
    BoardAction, CopyAction, DispatchOutcome, HostReply, LayoutAction, MenuAction, ModeRequest,
    PaneAdmission, PaneInput, PointerCaptureRequest, RenameAction, RenameDraftToken, RenameEdit,
    SettingsAction, SetupAction, SetupEdit, SetupToken, SurfaceAction, SurfaceInput, TabAction,
    TerminalInputPort, TerminalObservation, TextEntry, WorktreeAction,
    decoder::{DecodedItem, DecodedKind, LegacyDecoder},
    forward_or_report,
    modal::{ModalCapacity, ModalDecoder, ModalInputEvent},
};

mod fenced_chord;
mod navigator;
mod tab_list;

struct ActionDecision {
    transition: crate::client_view::ClientTransition,
    captured: Option<crate::workbench_client::input::CapturedClientAction>,
}

impl From<crate::client_view::ClientTransition> for ActionDecision {
    fn from(transition: crate::client_view::ClientTransition) -> Self {
        Self {
            transition,
            captured: None,
        }
    }
}

#[derive(Default)]
pub struct InputDispatcher {
    navigator: navigator::Capture,
    graphics: crate::terminal_graphics::Probe,
    pointer_admission: crate::terminal_pointer_admission::Probe,
    host_appearance: crate::terminal_host_appearance::Probe,
    host_background: Option<u32>,
    pointer_capture: PointerCaptureRequest,
    /// A real button-free report is independent of optional DECRQM support on
    /// the admitted Windows console endpoint. It grants presentation only.
    native_motion_observed: bool,
    /// A failed level switch may have emitted a prefix. Runtime must exit and
    /// lifecycle cleanup must settle it; input never retries that transition.
    pointer_transition_failed: bool,
    graphics_replies: Vec<crate::terminal_graphics::Reply>,
    graphics_reply_overflow: bool,
    decoder: ModalDecoder,
    keys: LegacyDecoder,
    epoch: Option<ModalEpoch>,
    /// A modal press owns its release even after the modal has submitted.
    modal_release: Option<super::MouseButton>,
    /// Decoded bytes of the in-flight packet that still owe an ordered
    /// forward; always drained before `dispatch`/`synchronize` returns.
    pending: Vec<u8>,
    /// An open confirmation captured the current packet whole, so every
    /// later item of the same read stays swallowed.
    confirm_packet: bool,
    /// A visible Client control owns the rest of its decision packet.
    pointer_packet: bool,
    machine_packet: bool,
    sidebar_packet: bool,
    sidebar_owned: bool,
    sidebar_cancel_capture: bool,
    popup_packet: bool,
    copy_packet: bool,
    copy_owned: bool,
    settings_packet: bool,
    settings_cancel_capture: bool,
    menu_packet: bool,
    /// A cancelled menu was still the painted input surface. Capture reads until a fresh frame.
    menu_cancel_capture: bool,
    menu_release: Option<super::MouseButton>,
    /// Prefix chords a decision packet fenced; they resolve once the keyboard is free again.
    fenced_chords: fenced_chord::FencedChords,
    popup_owned: Option<orchbee_workbench_protocol::WorkbenchRequestId>,
    /// Pointer ownership: capture flag, last-drawn hit map, gesture
    /// lifecycle, and the bounded wheel stream.
    pointer: PointerState,
    /// Conservative repaint decision for the most recently completed input packet.
    dispatch_redraw: bool,
    /// Set only by the ordinary pointer path for one button-free motion item.
    item_idle_motion: Option<bool>,
    /// Last observed result of the shared stateless compositor. This only
    /// detects presentation-map edges; owner selection is always recomputed.
    capture_owner: Option<CaptureOwner>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ModalEpoch {
    Rename(RenameDraftToken),
    Setup(SetupToken),
    Worktree(crate::workbench_state::worktree_launch::WorktreeToken),
}

struct ProjectedCaptureItem {
    owner: CaptureOwner,
    item: DecodedItem,
    now: Instant,
}

#[cfg(test)]
mod tests;

/// Adapts one decoded item to the confirmation port's decision vocabulary. Only
/// legal single keys convert; every other complete item (modified keys,
/// navigation keys, paste runs, non-decision sequences) maps to `Other` so an
/// open confirmation swallows it without a decision — never a tail leak.
fn client_key(item: &DecodedItem) -> ClientKey {
    match item.kind {
        DecodedKind::Key(event) if event.kind != KeyKind::Release => {
            let (code, mods) = (event.code, event.mods);
            // Legacy CR is the canonical Ctrl+M alias. Named protocol Enter
            // must itself be unmodified; Ctrl+Enter is a distinct report.
            if (code == KeyCode::Enter && mods == Modifiers::NONE)
                || (code == KeyCode::Char('m') && mods == Modifiers::CTRL)
            {
                ClientKey::Enter
            } else if mods == Modifiers::NONE {
                match code {
                    KeyCode::Char(c) => ClientKey::Plain(c),
                    KeyCode::Esc => ClientKey::Escape,
                    _ => ClientKey::Other,
                }
            } else if code == KeyCode::Char('b') && mods == Modifiers::CTRL {
                ClientKey::ControlB
            } else {
                ClientKey::Other
            }
        }
        _ => ClientKey::Other,
    }
}

/// The single C0 byte a literal-prefix forward emits for the chord.
fn literal_chord_byte(item: &DecodedItem) -> Option<u8> {
    match item.kind {
        DecodedKind::Key(event) => match event.normalized() {
            (KeyCode::Char(c @ 'a'..='z'), mods) if mods == Modifiers::CTRL => Some(c as u8 - 0x60),
            (KeyCode::Char(' '), mods) if mods == Modifiers::CTRL => Some(0x00),
            _ => None,
        },
        _ => None,
    }
}

impl InputDispatcher {
    /// Called between frames; retire an unknown write rather than replaying it.
    pub fn synchronize_host_background<E>(
        &mut self,
        theme: &crate::tui_theme::Theme,
        mode: &mut impl FnMut(ModeRequest) -> Result<(), E>,
    ) -> Result<(), E> {
        let background = theme.host_background();
        if self.host_background == background {
            return Ok(());
        }
        self.host_background = background;
        mode(ModeRequest::HostBackground(background))
    }

    fn projected_capture(
        _view: &ClientView,
        port: &(impl TerminalInputPort + ?Sized),
    ) -> Option<CaptureOwner> {
        let client = ClientCaptureClaims::default();
        let client = client
            .with_machine(_view.machines.open)
            .with_resize(_view.resize_mode());
        let native = port
            .native_capture_claims()
            .with_copy(port.copy_mode_active());

        project_capture(client, native)
    }

    fn modal_remaining(port: &mut (impl TerminalInputPort + ?Sized), epoch: ModalEpoch) -> usize {
        use crate::workbench_client::CaptureContext;

        match epoch {
            ModalEpoch::Rename(token) => {
                match port.native_capture_context(WorkbenchCaptureOwner::Rename) {
                    Some(crate::workbench_client::CaptureContext::Rename(context))
                        if context.token == token =>
                    {
                        context.remaining_scalars
                    }
                    _ => 0,
                }
            }
            ModalEpoch::Setup(token) => {
                match port.native_capture_context(WorkbenchCaptureOwner::Setup) {
                    Some(CaptureContext::Setup(context)) if context.token == token => {
                        if matches!(
                            context.stage,
                            SetupStage::DirectoryPath | SetupStage::QuickDirectory
                        ) {
                            context.remaining_bytes
                        } else {
                            context.remaining_scalars
                        }
                    }
                    _ => 0,
                }
            }
            ModalEpoch::Worktree(token) => {
                match port.native_capture_context(WorkbenchCaptureOwner::Worktree) {
                    Some(CaptureContext::Worktree(context)) if context.token == token => {
                        context.remaining_bytes
                    }
                    _ => 0,
                }
            }
        }
    }

    fn settle_settings_closed(&mut self, port: &mut (impl TerminalInputPort + ?Sized)) {
        self.settings_packet = true;
        self.settings_cancel_capture = true;
        self.keys.capture_pending();
        self.pointer.invalidate_map_with_port(port);
    }
    fn settle_action_menu_closed(&mut self, port: &mut (impl TerminalInputPort + ?Sized)) {
        self.menu_packet = true;
        self.menu_cancel_capture = true;
        self.keys.capture_pending();
        self.pointer.invalidate_map_with_port(port);
    }
    /// Completes input decided before this packet boundary: an action menu choice that became
    /// available, then prefix chords a decision packet fenced.
    pub fn complete_deferred_input<Port: TerminalInputPort + ?Sized, Error>(
        &mut self,
        view: &mut ClientView,
        port: &mut Port,
        now: Instant,
        after_layout: &mut impl FnMut(&mut ClientView, &mut Port) -> Result<bool, Error>,
        mode: &mut impl FnMut(ModeRequest) -> Result<(), Error>,
    ) -> Result<Option<DispatchOutcome>, Error> {
        if let Some(outcome) = self.complete_action_menu(view, port, now, after_layout, mode)? {
            return Ok(Some(outcome));
        }
        self.replay_fenced_chords(view, port, now, after_layout, mode)
    }

    fn complete_action_menu<Port: TerminalInputPort + ?Sized, Error>(
        &mut self,
        view: &mut ClientView,
        port: &mut Port,
        now: Instant,
        after_layout: &mut impl FnMut(&mut ClientView, &mut Port) -> Result<bool, Error>,
        mode: &mut impl FnMut(ModeRequest) -> Result<(), Error>,
    ) -> Result<Option<DispatchOutcome>, Error> {
        let Some(action) = port.take_action_menu_action(view.pointer_released()) else {
            return Ok(None);
        };
        self.settle_action_menu_closed(port);
        let (intent, captured) = action.into_parts();
        let transition = view.decide_action(intent);
        let decision = ActionDecision {
            transition,
            captured,
        };
        self.apply_transition(view, port, now, decision, None, after_layout, mode)
            .map(Some)
    }
    fn host_reply(
        &mut self,
        reply: HostReply,
        now: Instant,
    ) -> Option<crate::host_appearance::HostAppearance> {
        match reply {
            HostReply::Graphics(reply) => {
                self.graphics.reply(reply, now);
                if matches!(reply, crate::terminal_graphics::Reply::Kitty { .. }) {
                    if self.graphics_replies.len() < 128 {
                        self.graphics_replies.push(reply);
                    } else {
                        self.graphics_reply_overflow = true;
                    }
                }
                None
            }
            HostReply::Pointer(reply) => {
                self.pointer_admission.observe(reply, now);
                None
            }
            HostReply::Appearance(reply) => self.host_appearance.observe(reply, now),
        }
    }

    /// Starts the endpoint's only host-color query before its first frame is drawn.
    pub fn begin_host_appearance<Error>(
        &mut self,
        now: Instant,
        mode: &mut impl FnMut(ModeRequest) -> Result<(), Error>,
    ) -> Result<(), Error> {
        if let Some(query) = self.host_appearance.begin(now) {
            mode(ModeRequest::AppearanceQuery(query))?;
        }
        Ok(())
    }
    pub fn take_graphics_replies(&mut self) -> (Vec<crate::terminal_graphics::Reply>, bool) {
        (
            std::mem::take(&mut self.graphics_replies),
            std::mem::take(&mut self.graphics_reply_overflow),
        )
    }
    pub fn activate_graphics_probe(&mut self, id: u32) {
        self.graphics.activate(id);
    }
    pub fn graphics_facts(&self) -> crate::terminal_graphics::Facts {
        self.graphics.facts()
    }
    pub fn query_graphics<Error>(
        &mut self,
        cells: (u16, u16),
        now: Instant,
        mode: &mut impl FnMut(ModeRequest) -> Result<(), Error>,
    ) -> Result<bool, Error> {
        let previous = self.graphics.facts();
        if let Some(query) = self.graphics.query(cells, now) {
            mode(ModeRequest::GraphicsQuery(query))?;
        }
        Ok(self.graphics.facts() != previous)
    }

    /// Starts this endpoint's sole pointer capability query after the
    /// ButtonOnly baseline is known to be active.
    pub fn begin_pointer_admission<Error>(
        &mut self,
        now: Instant,
        mode: &mut impl FnMut(ModeRequest) -> Result<(), Error>,
    ) -> Result<(), Error> {
        if self.pointer_capture == PointerCaptureRequest::ButtonOnly
            && let Some(query) = self.pointer_admission.begin(now)
        {
            mode(ModeRequest::PointerAdmissionQuery(query))?;
        }
        Ok(())
    }

    fn admitted_pointer_capture(&self) -> PointerCaptureRequest {
        if self.pointer_admission.any_motion_admitted() {
            PointerCaptureRequest::AnyMotion
        } else {
            PointerCaptureRequest::ButtonOnly
        }
    }

    fn pointer_observability(&self, request: PointerCaptureRequest) -> MotionObservability {
        match request {
            PointerCaptureRequest::AnyMotion => MotionObservability::AnyMotion,
            PointerCaptureRequest::MenuMotion => MotionObservability::AnyMotion,
            PointerCaptureRequest::ButtonOnly if self.native_motion_observed => {
                MotionObservability::AnyMotion
            }
            PointerCaptureRequest::Release | PointerCaptureRequest::ButtonOnly => {
                MotionObservability::ButtonOnly
            }
        }
    }

    /// Global motion follows capability admission. An already armed capturing menu may also
    /// request motion with button fallback; a released pointer is never implicitly re-armed.
    /// Every mode effect precedes publication, including menu exit and late probe settlement.
    fn synchronize_pointer_admission<Port: TerminalInputPort + ?Sized, Error>(
        &mut self,
        view: &ClientView,
        port: &Port,
        mode: &mut impl FnMut(ModeRequest) -> Result<(), Error>,
    ) -> Result<(), Error> {
        if self.pointer_transition_failed
            || matches!(
                self.pointer_capture,
                PointerCaptureRequest::Release | PointerCaptureRequest::AnyMotion
            )
        {
            return Ok(());
        }
        let request = self.admitted_pointer_capture();
        let request = if request == PointerCaptureRequest::ButtonOnly
            && Self::projected_capture(view, port)
                == Some(CaptureOwner::Native(WorkbenchCaptureOwner::ActionMenu))
        {
            PointerCaptureRequest::MenuMotion
        } else {
            request
        };

        if self.pointer_capture == request {
            return Ok(());
        }
        if let Err(error) = mode(ModeRequest::PointerCapture(request)) {
            self.pointer_transition_failed = true;
            return Err(error);
        }
        self.pointer_capture = request;
        self.pointer
            .set_motion_observability(self.pointer_observability(request));
        Ok(())
    }
    pub fn machine_surface_changed(&mut self) {
        self.keys.capture_pending();
        self.pointer.invalidate_map();
    }

    /// The shell settles the armed flag after the terminal mode write
    /// succeeded, so client state never claims a capture the host did not
    /// actually take.
    #[cfg(any(test, feature = "test-support"))]
    pub fn set_pointer_capture(
        &mut self,
        view: &mut ClientView,
        port: &mut (impl TerminalInputPort + ?Sized),
        enabled: bool,
    ) {
        self.settle_pointer_capture(
            view,
            port,
            if enabled {
                PointerCaptureRequest::ButtonOnly
            } else {
                PointerCaptureRequest::Release
            },
        );
    }

    /// Publishes the exact tracking level after its lifecycle request has
    /// completed. Direct fixture callers retain the ButtonOnly helper above.
    pub fn settle_pointer_capture(
        &mut self,
        view: &mut ClientView,
        port: &mut (impl TerminalInputPort + ?Sized),
        request: PointerCaptureRequest,
    ) {
        let enabled = request != PointerCaptureRequest::Release;
        if !enabled {
            let closing = port
                .native_capture_claims()
                .contains(WorkbenchCaptureOwner::ActionMenu);
            port.surface_action(SurfaceAction::ActionMenu(MenuAction::Cancel));
            self.menu_release = None;
            if closing {
                self.settle_action_menu_closed(port);
            }
        }
        self.pointer
            .set_motion_observability(self.pointer_observability(request));
        self.pointer.set_capture_with_port(port, enabled);
        view.settle_pointer_released(!enabled);
        self.pointer_capture = request;
        self.pointer_transition_failed = false;
    }

    #[cfg(test)]
    pub(crate) fn set_pointer_motion_observability(
        &mut self,
        observability: crate::tui_pointer::MotionObservability,
    ) {
        self.pointer.set_motion_observability(observability);
    }

    pub fn pointer_projection(&self) -> crate::tui_hit_map::PointerProjection {
        self.pointer.projection()
    }

    pub fn take_pointer_projection_redraw(&mut self) -> bool {
        self.pointer.take_projection_redraw()
    }

    pub const fn dispatch_requested_redraw(&self) -> bool {
        self.dispatch_redraw
    }

    /// The renderer's finished frame map becomes the resolution authority.
    #[cfg(any(test, feature = "test-support"))]
    pub fn publish_hit_map(&mut self, map: FrameHitMap) {
        let _ = self.publish_hit_map_with_help(map);
    }

    /// Publishes the map and returns its exact Help draw proof to the
    /// client-owned surface authority. Other callers do not need to observe it.
    pub fn publish_hit_map_with_help(
        &mut self,
        map: FrameHitMap,
    ) -> Option<crate::client_view::help::HelpDrawSource> {
        let help = map.help_draw_source().copied();
        self.settings_cancel_capture = false;
        {
            if self.navigator.owned {
                self.navigator.unpresented = map.navigator_paint().is_none();
            } else {
                self.navigator.cancel_capture = false;
            }
            self.menu_cancel_capture = false;
            self.sidebar_cancel_capture = false;
        }
        self.pointer.publish(map);
        help
    }

    /// A layout change or pending redraw drops the recorded regions:
    /// until the next publish, pointer events drop instead of misrouting.
    pub fn invalidate_hit_map(&mut self, port: &mut (impl TerminalInputPort + ?Sized)) {
        self.pointer.invalidate_map_with_port(port);
    }

    /// A Client control step changed what the next frame paints: the recorded regions drop as
    /// for any layout change, while an admitted divider drag waits for that frame to keep or
    /// end it.
    pub fn invalidate_hit_map_for_control(&mut self, port: &mut (impl TerminalInputPort + ?Sized)) {
        self.pointer.invalidate_map_for_control(port);
    }

    /// Ends an admitted pointer selection before the shell exits while the
    /// surface that owns it is still available.
    pub fn cancel_pointer(&mut self, port: &mut (impl TerminalInputPort + ?Sized)) {
        self.pointer.cancel_selection(port);
        {
            self.pointer.cancel_divider();
            self.pointer.cancel_sidebar_width();
            self.pointer.cancel_tab_drag();
            self.pointer.cancel_sidebar_reorder();
        }
    }

    pub fn sidebar_reorder_preview(&self) -> Option<crate::tui_pointer::SidebarReorderPreview> {
        self.pointer.sidebar_reorder_preview()
    }

    /// Before a frame paints, shows a sidebar drag's live order, or restores the native one once
    /// no drag shows it. Running after the native observation lets an admitted drop hand its order
    /// to the pending-move projection without passing through the old one.
    pub fn show_sidebar_reorder(&self, view: &mut ClientView) -> bool {
        view.machines.preview_sidebar_reorder(
            self.pointer.sidebar_reorder_order(),
            self.pointer.sidebar_reorder_held(),
        )
    }

    pub fn resize_projection(&self) -> crate::tui_layout::ResizeProjection {
        self.pointer.resize_projection()
    }

    pub fn resize_presentation_allowed(&self) -> bool {
        self.pointer.resize_presentation_allowed()
    }

    /// Immutable preview supplied by the gesture owner for one paint pass.
    pub fn divider_preview(&self) -> Option<&crate::tui_layout::DividerDrag> {
        self.pointer.divider_preview()
    }

    pub fn tab_drag_preview(&self) -> Option<crate::tui_hit_map::TabDragPreview> {
        self.pointer.tab_drag_preview()
    }

    pub fn sidebar_width_preview(&self) -> Option<&crate::tui_layout::SidebarWidthDrag> {
        self.pointer.sidebar_width_preview()
    }

    /// Detects an edge in the shared composition result. The cached value is
    /// only a map/packet fence; the next owner is always recomputed from the
    /// current claims rather than selected from this history.
    fn settle_capture_edge<Port: TerminalInputPort + ?Sized>(
        &mut self,
        view: &ClientView,
        port: &mut Port,
    ) -> bool {
        let current = Self::projected_capture(view, port);
        let popup = if current == Some(CaptureOwner::Native(WorkbenchCaptureOwner::Popup)) {
            match port.native_capture_context(WorkbenchCaptureOwner::Popup) {
                Some(crate::workbench_client::CaptureContext::Popup(context)) => {
                    Some(context.id().clone())
                }
                _ => None,
            }
        } else {
            None
        };
        let popup_changed = popup != self.popup_owned;
        {
            self.popup_owned = popup;
        }
        if current == self.capture_owner && !popup_changed {
            return false;
        }

        let previous = std::mem::replace(&mut self.capture_owner, current);
        if current.is_some() && self.pointer.retire_covered_chrome_gesture() {
            self.pointer_packet = true;
        }
        if previous == Some(CaptureOwner::Native(WorkbenchCaptureOwner::Settings))
            && current != previous
        {
            self.settle_settings_closed(port);
        }
        if previous == Some(CaptureOwner::Native(WorkbenchCaptureOwner::ActionMenu))
            && current != previous
        {
            self.settle_action_menu_closed(port);
        }
        if previous == Some(CaptureOwner::Native(WorkbenchCaptureOwner::Popup))
            && (current != previous || popup_changed)
        {
            self.popup_packet = true;
        }
        if previous == Some(CaptureOwner::Machine) && current != previous {
            self.machine_packet = true;
        }
        if matches!(
            previous,
            Some(CaptureOwner::Native(
                WorkbenchCaptureOwner::PanelClose
                    | WorkbenchCaptureOwner::TabClose
                    | WorkbenchCaptureOwner::WorkspaceClose
            ))
        ) && current != previous
        {
            self.confirm_packet = true;
        }
        if previous == Some(CaptureOwner::Native(WorkbenchCaptureOwner::AgentBoard))
            && current != previous
        {
            self.pointer_packet = true;
        }
        self.keys.capture_pending();
        // A newly covering compositor owner revokes painted chrome authority,
        // but it does not own an already admitted pane gesture. Preserve that
        // gesture for its live-identity settlement while dropping the old map
        // and any queued list work.
        self.pointer.cancel_list_scroll();
        self.pointer.stale_map();
        true
    }

    #[cfg(test)]
    pub(crate) const fn pointer_capture_enabled(&self) -> bool {
        self.pointer.capture_enabled()
    }

    pub fn synchronize<Port: TerminalInputPort + ?Sized, Error>(
        &mut self,
        view: &mut ClientView,
        port: &mut Port,
        now: Instant,
        mode: &mut impl FnMut(ModeRequest) -> Result<(), Error>,
    ) -> Result<bool, Error> {
        self.synchronize_input(view, port, now, mode, true)
    }

    /// A decision in the current packet, or a cancellation not yet repainted, owns the tail.
    fn packet_fenced(&self) -> bool {
        let fenced = self.confirm_packet || self.settings_packet || self.pointer_packet;
        fenced
            || self.navigator.packet
            || self.machine_packet
            || self.sidebar_packet
            || self.popup_packet
            || self.copy_packet
            || self.menu_packet
    }

    fn synchronize_input<Port: TerminalInputPort + ?Sized, Error>(
        &mut self,
        view: &mut ClientView,
        port: &mut Port,
        now: Instant,
        mode: &mut impl FnMut(ModeRequest) -> Result<(), Error>,
        idle: bool,
    ) -> Result<bool, Error> {
        self.graphics.tick(now);
        self.pointer_admission.tick(now);
        self.synchronize_pointer_admission(view, port, mode)?;
        let appearance_changed = if self.host_appearance.timeout(now).is_some() {
            let fallback = crate::terminal_host_appearance::system_fallback();

            let appearance = self
                .host_appearance
                .settle_fallback(fallback)
                .expect("appearance timeout owns one platform fallback settlement");
            view.configure_host_appearance(appearance)
        } else {
            false
        };
        // Compare the settled owner from the preceding boundary with the
        // post-reconcile projection below. An asynchronously installed owner
        // is already visible to the pure projector here, but it has not yet
        // acquired the host paste lease.
        let owned_input = self.decoder.owns_input() || self.capture_owner.is_some();
        let owned_input = owned_input || view.surface_owns_input();
        // An idle read is a packet boundary too: a `Released` guard left
        // over from an earlier packet ends here so the lease is returned
        // even when no further bytes ever arrive. A release produced by
        // this call's own reconcile still guards the current packet tail.
        self.decoder.end_released(now);
        let mut changed = appearance_changed | self.reconcile(view, port, now);
        if idle {
            let events = self.decoder.tick(now);
            changed |= !events.is_empty();
            self.apply_events(view, port, events, now);
            self.synchronize_pointer_admission(view, port, mode)?;
        }
        self.confirm_packet = false;
        self.pointer_packet = false;
        self.settings_packet = self.settings_cancel_capture;
        {
            self.machine_packet = false;
            self.sidebar_packet = self.sidebar_cancel_capture;
            self.popup_packet = false;
            self.copy_packet = false;
            self.menu_packet = self.menu_cancel_capture;
            self.navigator.packet = self.navigator.cancel_capture;
        }
        if idle {
            for item in self.keys.tick(now) {
                changed = true;
                self.handle_item(view, port, now, item, &mut |_, _| Ok(false), mode)?;
            }
        }
        self.flush_pending(view, port);
        if idle {
            changed |= self.pointer.flush_list_scroll(view, port);
        }
        let pointer_owner = Self::projected_capture(view, port);
        changed |= self.pointer.tick(view, port, now, pointer_owner);
        changed |= self.reconcile(view, port, now);
        let capturing = self.decoder.owns_input() || Self::projected_capture(view, port).is_some();
        let capturing = capturing || view.surface_owns_input();
        if !capturing {
            mode(ModeRequest::BracketedPaste(false))?;
        } else if !owned_input {
            // A later wizard step can arrive after the previous decoder released its lease.
            mode(ModeRequest::BracketedPaste(true))?;
        }
        self.synchronize_pointer_admission(view, port, mode)?;
        Ok(changed)
    }

    pub fn dispatch<Port: TerminalInputPort + ?Sized, Error>(
        &mut self,
        view: &mut ClientView,
        input: &[u8],
        port: &mut Port,
        now: Instant,
        // True reports an observed viewport/visibility change, including within this read.
        mut after_layout: impl FnMut(&mut ClientView, &mut Port) -> Result<bool, Error>,
        mode: &mut impl FnMut(ModeRequest) -> Result<(), Error>,
    ) -> Result<DispatchOutcome, Error> {
        self.dispatch_redraw = true;
        self.item_idle_motion = None;
        let mut redraw = self.synchronize_input(view, port, now, mode, input.is_empty())?;
        let mut saw_item = false;
        let mut only_idle_motion = true;
        // Ready bytes resolve pending frames before idle timeouts, and a newly
        // available reversal precedes draining the previous list burst.
        if let Some(outcome) =
            self.complete_deferred_input(view, port, now, &mut after_layout, mode)?
        {
            redraw = true;
            only_idle_motion = false;
            if outcome == DispatchOutcome::Exit {
                return Ok(DispatchOutcome::Exit);
            }
        }
        // A new read packet begins: the modal's packet-tail guard ends at
        // this boundary, while an in-flight frame or an open ESC
        // disambiguation keeps ownership under its own rules.
        self.decoder.begin_packet(now);
        for byte in input.iter().copied() {
            redraw |= self.reconcile(view, port, now);
            let remaining = self
                .epoch
                .map_or(0, |epoch| Self::modal_remaining(port, epoch));
            let (captured, events) = self.decoder.push(byte, now, remaining);
            if captured || !events.is_empty() {
                redraw = true;
                only_idle_motion = false;
                self.flush_pending(view, port);
                self.apply_events(view, port, events, now);
                self.synchronize_pointer_admission(view, port, mode)?;
                redraw |= self.reconcile(view, port, now);
                if captured {
                    continue;
                }
            }
            // A frame opened inside a captured packet carries `owned`
            // provenance: it stays captured even when it completes in a
            // later read, while genuinely later input decodes normally.
            let owned = self.packet_fenced();
            for item in self.keys.push(byte, now, owned) {
                saw_item = true;
                self.item_idle_motion = None;
                if let DispatchOutcome::Exit =
                    self.handle_item(view, port, now, item, &mut after_layout, mode)?
                {
                    self.flush_pending(view, port);
                    return Ok(DispatchOutcome::Exit);
                }
                self.synchronize_pointer_admission(view, port, mode)?;
                match self.item_idle_motion.take() {
                    Some(changed) => redraw |= changed,
                    None => {
                        redraw = true;
                        only_idle_motion = false;
                    }
                }
            }
        }
        self.flush_pending(view, port);
        // Queued pointer gestures and the wheel stream advance at the same
        // packet boundary the modal leases do.
        redraw |= self.pointer.flush_list_scroll(view, port);
        let pointer_owner = Self::projected_capture(view, port);
        redraw |= self.pointer.tick(view, port, now, pointer_owner);
        // The final pointer report may retire an input owner without a
        // following byte. Settle that edge before a frame can be published.
        redraw |= self.reconcile(view, port, now);
        if let Some(outcome) =
            self.complete_deferred_input(view, port, now, &mut after_layout, mode)?
        {
            redraw = true;
            only_idle_motion = false;
            if outcome == DispatchOutcome::Exit {
                return Ok(DispatchOutcome::Exit);
            }
        }
        self.synchronize_pointer_admission(view, port, mode)?;
        self.dispatch_redraw = redraw || !saw_item || !only_idle_motion;
        Ok(DispatchOutcome::Continue)
    }

    fn handle_projected_capture<Port: TerminalInputPort + ?Sized, Error>(
        &mut self,
        view: &mut ClientView,
        port: &mut Port,
        projected: ProjectedCaptureItem,
        after_layout: &mut impl FnMut(&mut ClientView, &mut Port) -> Result<bool, Error>,
        mode: &mut impl FnMut(ModeRequest) -> Result<(), Error>,
    ) -> Result<DispatchOutcome, Error> {
        use crate::workbench_client::CaptureContext;

        let ProjectedCaptureItem { owner, item, now } = projected;

        if owner == CaptureOwner::Machine {
            let mut regions_changed = false;
            match item.kind {
                DecodedKind::Key(key) => {
                    // Keyboard input settles at most one exposed wheel chunk
                    // at the common pre-dispatch boundary, then cancels the
                    // remainder before this edit reaches the Picker.
                    view.machines.key(key);
                    regions_changed = true;
                }
                DecodedKind::Paste { text, .. } => {
                    view.machines.paste_workspace_query(&text);
                    regions_changed = true;
                }
                DecodedKind::Mouse(event) if self.pointer.capture_enabled() => {
                    self.pointer.external_capture_event(event);
                    match self.pointer.machine_picker_event(event) {
                        PointerOutcome::Interaction {
                            target: InteractionTarget::MachinePicker(target),
                            contract: PrimaryContract::ActivateOnRelease,
                        } => {
                            regions_changed = view.machines.activate_drawn_picker(&target);
                            self.machine_packet = true;
                            self.keys.capture_pending();
                        }
                        PointerOutcome::Interaction { .. } | PointerOutcome::InteractionEnded => {
                            // The drawn Picker target owns this release even
                            // when exact source revalidation refuses its effect.
                            self.machine_packet = true;
                            self.keys.capture_pending();
                        }
                        PointerOutcome::RegionsChanged => regions_changed = true,
                        PointerOutcome::Ignored
                        | PointerOutcome::Handled
                        | PointerOutcome::IdleMotion { .. } => {}
                        PointerOutcome::TabGestureEnded
                        | PointerOutcome::SidebarReorderEnded
                        | PointerOutcome::SidebarWidthEnded
                        | PointerOutcome::MenuRequested { .. }
                        | PointerOutcome::ChromeGestureEnded => {
                            unreachable!("Machine Picker routing cannot produce chrome output")
                        }
                    }
                }
                _ => {}
            }
            if let Some(id) = view.machines.copy_id_request.take()
                && !port.copy_picker_identifier(id)
            {
                view.machines.error = Some("copy-id-unavailable");
            }
            if !view.machines.open || view.machines.request.is_some() {
                self.machine_packet = true;
                self.keys.capture_pending();
                regions_changed = true;
            }
            if regions_changed {
                self.pointer.invalidate_map_with_port(port);
            }
            return Ok(DispatchOutcome::Continue);
        }

        if owner == CaptureOwner::Copy {
            if !self.copy_packet {
                match &item.kind {
                    DecodedKind::Mouse(event) => {
                        if after_layout(view, port)? {
                            self.pointer.invalidate_map_with_port(port);
                        }
                        let decision = self.pointer.copy_event(view, port, *event);
                        self.capture_modal_release(*event);
                        if event.action == super::MouseAction::Press
                            && matches!(
                                event.button,
                                super::MouseButton::Left
                                    | super::MouseButton::Right
                                    | super::MouseButton::Middle
                            )
                        {
                            self.modal_release = Some(event.button);
                        }
                        if decision {
                            self.copy_packet = true;
                            self.keys.capture_pending();
                        }
                    }
                    DecodedKind::Key(key) if !view.copy_help_key(*key) => {
                        port.surface_input(SurfaceInput::CopyMode(TextEntry::Key(*key)));
                    }
                    DecodedKind::Paste { text } => {
                        port.surface_input(SurfaceInput::CopyMode(TextEntry::Paste(Some(text))));
                    }
                    DecodedKind::RejectedPaste => {
                        port.surface_input(SurfaceInput::CopyMode(TextEntry::Paste(None)));
                    }
                    _ => {}
                }
                if !port.copy_mode_active() {
                    self.copy_packet = true;
                    self.keys.capture_pending();
                }
            }
            view.settle_copy_mode(port.copy_mode_active());
            return Ok(DispatchOutcome::Continue);
        }

        if owner == CaptureOwner::Resize {
            if let DecodedKind::Mouse(event) = item.kind {
                if after_layout(view, port)? {
                    self.pointer.invalidate_map_with_port(port);
                }
                let decision = self.pointer.resize_event(view, port, event);
                self.capture_modal_release(event);
                if event.action == super::MouseAction::Press
                    && matches!(
                        event.button,
                        super::MouseButton::Left
                            | super::MouseButton::Right
                            | super::MouseButton::Middle
                    )
                {
                    self.modal_release = Some(event.button);
                }
                if decision {
                    self.pointer_packet = true;
                    self.keys.capture_pending();
                }
                return Ok(DispatchOutcome::Continue);
            }
            if matches!(item.kind, DecodedKind::Key(event) if event.kind == KeyKind::Release) {
                return Ok(DispatchOutcome::Continue);
            }
            return self.decide(view, port, now, item, after_layout, mode);
        }

        let native_owner = match owner {
            CaptureOwner::Machine => unreachable!("the machine capture returned above"),
            CaptureOwner::Copy | CaptureOwner::Resize => {
                unreachable!("the client capture returned above")
            }
            CaptureOwner::Native(owner) => owner,
        };
        let context = port.native_capture_context(native_owner);
        match (native_owner, context) {
            (WorkbenchCaptureOwner::Settings, Some(CaptureContext::Settings(expected))) => {
                match item.kind {
                    DecodedKind::Paste { text } => {
                        let _consumed = port.surface_input(SurfaceInput::Settings {
                            expected,
                            entry: TextEntry::Paste(Some(&text)),
                        });
                        self.pointer.invalidate_map_with_port(port);
                    }
                    DecodedKind::RejectedPaste => {
                        let _consumed = port.surface_input(SurfaceInput::Settings {
                            expected,
                            entry: TextEntry::Paste(None),
                        });
                        self.pointer.invalidate_map_with_port(port);
                    }
                    // Settings stays the owner after Enter or Esc unless it closed, so the rest
                    // of the packet is typed-ahead page input in order; only a close fences it.
                    DecodedKind::Key(event) => {
                        let _consumed = port.surface_input(SurfaceInput::Settings {
                            expected,
                            entry: TextEntry::Key(event),
                        });
                        self.pointer.invalidate_map_with_port(port);
                    }
                    DecodedKind::Mouse(event) => match self.pointer.settings_event(event) {
                        PointerOutcome::IdleMotion { changed } => {
                            self.item_idle_motion = Some(changed);
                        }
                        PointerOutcome::Interaction { target, contract } => {
                            if contract == PrimaryContract::ActivateOnRelease {
                                match target {
                                    InteractionTarget::SettingsRow { token, row } => {
                                        port.surface_action(SurfaceAction::Settings(
                                            SettingsAction::Row { token, row },
                                        ));
                                    }
                                    InteractionTarget::SettingsControl { token, control } => {
                                        port.surface_action(SurfaceAction::Settings(
                                            SettingsAction::Control { token, control },
                                        ));
                                    }
                                    _ => {}
                                }
                            }
                            self.settings_packet = true;
                            self.keys.capture_pending();
                            self.pointer.invalidate_map_with_port(port);
                        }
                        PointerOutcome::InteractionEnded => {
                            self.settings_packet = true;
                            self.keys.capture_pending();
                        }
                        _ => {}
                    },
                    _ => {}
                }
                if Self::projected_capture(view, port)
                    != Some(CaptureOwner::Native(WorkbenchCaptureOwner::Settings))
                {
                    self.settle_settings_closed(port);
                    if Self::projected_capture(view, port).is_none() && !view.surface_owns_input() {
                        mode(ModeRequest::BracketedPaste(false))?;
                    }
                }
            }
            (WorkbenchCaptureOwner::ActionMenu, Some(CaptureContext::ActionMenu(expected))) => {
                match item.kind {
                    DecodedKind::Key(event) => {
                        let _consumed = port.surface_input(SurfaceInput::ActionMenu {
                            expected,
                            key: event,
                        });
                        if crate::client_view::Binding::new(KeyCode::Enter, Modifiers::NONE)
                            .matches(event)
                            || crate::client_view::Binding::new(KeyCode::Esc, Modifiers::NONE)
                                .matches(event)
                        {
                            self.menu_packet = true;
                            self.keys.capture_pending();
                        }
                    }
                    DecodedKind::Mouse(event) if self.pointer.capture_enabled() => {
                        let outcome = self.pointer.action_menu_event(port, event, now);
                        if event.action == super::MouseAction::Release
                            && self.menu_release.is_some_and(|button| {
                                event.button == button || event.button == super::MouseButton::None
                            })
                        {
                            self.menu_release = None;
                        }
                        if event.action == super::MouseAction::Press
                            && matches!(
                                event.button,
                                super::MouseButton::Left | super::MouseButton::Right
                            )
                        {
                            self.menu_release = Some(event.button);
                        }
                        match outcome {
                            PointerOutcome::IdleMotion { changed } => {
                                self.item_idle_motion = Some(changed);
                            }
                            PointerOutcome::Interaction { target, contract } => {
                                if contract == PrimaryContract::ActivateOnRelease {
                                    match target {
                                        InteractionTarget::ActionMenuRow { source, token, row } => {
                                            port.surface_action(SurfaceAction::ActionMenu(
                                                MenuAction::Row {
                                                    source: &source,
                                                    token,
                                                    row,
                                                },
                                            ));
                                        }
                                        InteractionTarget::ActionMenuControl {
                                            source,
                                            token,
                                            control,
                                        } => {
                                            port.surface_action(SurfaceAction::ActionMenu(
                                                MenuAction::Control {
                                                    source: &source,
                                                    token,
                                                    control,
                                                },
                                            ));
                                        }
                                        _ => {}
                                    }
                                }
                                self.menu_packet = true;
                                self.keys.capture_pending();
                                self.pointer.invalidate_map_with_port(port);
                            }
                            PointerOutcome::InteractionEnded => {
                                self.menu_packet = true;
                                self.keys.capture_pending();
                            }
                            PointerOutcome::RegionsChanged => {
                                self.menu_packet = true;
                                self.keys.capture_pending();
                                self.pointer.stale_map();
                            }
                            PointerOutcome::Ignored | PointerOutcome::Handled => {}
                            _ => {}
                        }
                    }
                    _ => {}
                }
                if Self::projected_capture(view, port)
                    != Some(CaptureOwner::Native(WorkbenchCaptureOwner::ActionMenu))
                {
                    self.settle_action_menu_closed(port);
                    if Self::projected_capture(view, port).is_none() && !view.surface_owns_input() {
                        mode(ModeRequest::BracketedPaste(false))?;
                    }
                }
            }
            (WorkbenchCaptureOwner::Popup, Some(CaptureContext::Popup(expected))) => {
                if let DecodedKind::Mouse(event) = &item.kind {
                    self.pointer.external_capture_event(*event);
                    match self.pointer.popup_event(port, *event) {
                        PointerOutcome::Interaction { target, contract } => {
                            self.popup_packet = true;
                            self.keys.capture_pending();
                            if let (
                                InteractionTarget::PopupClose(target),
                                PrimaryContract::ActivateOnRelease,
                            ) = (target, contract)
                            {
                                let _activated =
                                    port.surface_action(SurfaceAction::PopupClose(&target));
                            }
                            // Accepted and stale exact releases both retire this
                            // painted authority. Neither may re-hit a replacement.
                            self.pointer.invalidate_map_with_port(port);
                        }
                        PointerOutcome::InteractionEnded => {
                            self.popup_packet = true;
                            self.keys.capture_pending();
                        }
                        PointerOutcome::IdleMotion { changed } => {
                            self.item_idle_motion = Some(changed);
                        }
                        PointerOutcome::Ignored
                        | PointerOutcome::Handled
                        | PointerOutcome::RegionsChanged
                        | PointerOutcome::TabGestureEnded
                        | PointerOutcome::SidebarReorderEnded
                        | PointerOutcome::SidebarWidthEnded
                        | PointerOutcome::ChromeGestureEnded
                        | PointerOutcome::MenuRequested { .. } => {}
                    }
                } else if expected.interactive() {
                    port.surface_input(SurfaceInput::Popup {
                        id: expected.id(),
                        item: &item,
                    });
                }
                if Self::projected_capture(view, port)
                    != Some(CaptureOwner::Native(WorkbenchCaptureOwner::Popup))
                {
                    self.popup_packet = true;
                    self.keys.capture_pending();
                    self.pointer.invalidate_map_with_port(port);
                }
            }
            (WorkbenchCaptureOwner::Worktree, Some(CaptureContext::Worktree(_)))
            | (WorkbenchCaptureOwner::Setup, Some(CaptureContext::Setup(_)))
            | (WorkbenchCaptureOwner::Rename, Some(CaptureContext::Rename(_))) => {
                // The modal decoder owns all interactive and recovery input for
                // these surfaces. A complete legacy item here means its exact
                // context disappeared between byte reconciliation and dispatch;
                // consume it without selecting a lower owner.
            }

            (WorkbenchCaptureOwner::PanelClose, Some(CaptureContext::PanelClose(expected))) => {
                if let DecodedKind::Mouse(event) = &item.kind {
                    self.capture_confirmation_mouse(port, *event);
                } else {
                    let _consumed = port.surface_input(SurfaceInput::PanelClose {
                        token: expected.token,
                        key: client_key(&item),
                    });
                    self.confirm_packet = true;
                    self.keys.capture_pending();
                }
            }
            (WorkbenchCaptureOwner::TabClose, Some(CaptureContext::TabClose(expected))) => {
                if let DecodedKind::Mouse(event) = &item.kind {
                    self.capture_confirmation_mouse(port, *event);
                } else {
                    let _consumed = port.surface_input(SurfaceInput::TabClose {
                        token: expected.token,
                        key: client_key(&item),
                    });
                    self.confirm_packet = true;
                    self.keys.capture_pending();
                }
            }
            (
                WorkbenchCaptureOwner::WorkspaceClose,
                Some(CaptureContext::WorkspaceClose(expected)),
            ) => {
                if let DecodedKind::Mouse(event) = &item.kind {
                    self.capture_confirmation_mouse(port, *event);
                } else {
                    let _consumed = port.surface_input(SurfaceInput::WorkspaceClose {
                        token: expected.token,
                        key: client_key(&item),
                    });
                    self.confirm_packet = true;
                    self.keys.capture_pending();
                }
            }
            (WorkbenchCaptureOwner::AgentBoard, Some(CaptureContext::AgentBoard(expected))) => {
                if let DecodedKind::Mouse(event) = item.kind {
                    let outcome = self.pointer.board_event(port, event);
                    self.capture_modal_release(event);
                    if event.action == super::MouseAction::Press
                        && matches!(
                            event.button,
                            super::MouseButton::Left
                                | super::MouseButton::Right
                                | super::MouseButton::Middle
                        )
                    {
                        self.modal_release = Some(event.button);
                    }
                    match outcome {
                        PointerOutcome::IdleMotion { changed } => {
                            self.item_idle_motion = Some(changed);
                        }
                        PointerOutcome::Interaction { target, contract } => {
                            match (target, contract) {
                                (
                                    InteractionTarget::AgentBoardRow(target),
                                    PrimaryContract::SelectOnPress,
                                ) => {
                                    let _accepted =
                                        port.surface_input(SurfaceInput::AgentBoardSelect(&target));
                                }
                                (
                                    InteractionTarget::AgentBoardControl(target),
                                    PrimaryContract::ActivateOnRelease,
                                ) => {
                                    let _accepted = port.surface_action(SurfaceAction::AgentBoard(
                                        BoardAction::Control(&target),
                                    ));
                                }
                                _ => {}
                            }
                            self.pointer_packet = true;
                            self.keys.capture_pending();
                            self.pointer.invalidate_map_with_port(port);
                        }
                        PointerOutcome::InteractionEnded => {
                            self.pointer_packet = true;
                            self.keys.capture_pending();
                        }
                        PointerOutcome::RegionsChanged => {
                            self.pointer_packet = true;
                            self.keys.capture_pending();
                            self.pointer.stale_map();
                        }
                        PointerOutcome::Ignored | PointerOutcome::Handled => {}
                        _ => {}
                    }
                } else if let DecodedKind::Key(event) = item.kind {
                    let _consumed = port.surface_input(SurfaceInput::AgentBoard {
                        episode: expected.episode(),
                        key: event,
                    });
                }
                if Self::projected_capture(view, port)
                    != Some(CaptureOwner::Native(WorkbenchCaptureOwner::AgentBoard))
                {
                    self.pointer_packet = true;
                    self.keys.capture_pending();
                    self.pointer.invalidate_map_with_port(port);
                }
            }
            // An owner changed its exact context between the claim and this
            // lookup. The selected surface still consumes this item; the next
            // complete item obtains a fresh projection.
            _ => {}
        }
        Ok(DispatchOutcome::Continue)
    }

    /// Routes one decoded item, noting whether it raised the packet fence: chords in the tail of a
    /// pointer decision resolve after it, while a key decision consumes its tail whole.
    fn handle_item<Port: TerminalInputPort + ?Sized, Error>(
        &mut self,
        view: &mut ClientView,
        port: &mut Port,
        now: Instant,
        item: DecodedItem,
        after_layout: &mut impl FnMut(&mut ClientView, &mut Port) -> Result<bool, Error>,
        mode: &mut impl FnMut(ModeRequest) -> Result<(), Error>,
    ) -> Result<DispatchOutcome, Error> {
        let fenced = self.packet_fenced();
        let pointer = matches!(item.kind, DecodedKind::Mouse(_));
        let outcome = self.route_item(view, port, now, item, after_layout, mode)?;
        self.switch_to_held_workspace(view, port, now, after_layout)?;
        if !fenced && self.packet_fenced() {
            self.fenced_chords.fence_raised(pointer);
        }
        Ok(outcome)
    }

    /// A drag the person ended, by a release or Escape, leaves the Workspace it held current, as
    /// a click on that row would. The sidebar stays open for further organizing.
    fn switch_to_held_workspace<Port: TerminalInputPort + ?Sized, Error>(
        &mut self,
        view: &mut ClientView,
        port: &mut Port,
        now: Instant,
        after_layout: &mut impl FnMut(&mut ClientView, &mut Port) -> Result<bool, Error>,
    ) -> Result<(), Error> {
        let Some(held) = self.pointer.take_sidebar_reorder_switch() else {
            return Ok(());
        };
        if view.machines.switch_to_held_workspace(&held) {
            view.machines.leave_sidebar_navigation();
            self.machine_packet = true;
            self.reconcile(view, port, now);
            after_layout(view, port)?;
        }
        Ok(())
    }

    /// Routes one decoded item through the ownership order: an open
    /// confirmation packet consumes the whole rest of the chunk, an armed
    /// prefix resolves the item against the binding registry, and anything
    /// else joins `self.pending` so its source bytes forward in order.
    /// the configured prefix arms its layer; configured direct bindings run
    /// after capture and history ownership.
    fn route_item<Port: TerminalInputPort + ?Sized, Error>(
        &mut self,
        view: &mut ClientView,
        port: &mut Port,
        now: Instant,
        item: DecodedItem,
        after_layout: &mut impl FnMut(&mut ClientView, &mut Port) -> Result<bool, Error>,
        mode: &mut impl FnMut(ModeRequest) -> Result<(), Error>,
    ) -> Result<DispatchOutcome, Error> {
        if let DecodedKind::HostReply(reply) = item.kind {
            self.flush_pending(view, port);
            if let Some(appearance) = self.host_reply(reply, now) {
                view.configure_host_terminal(appearance, self.host_appearance.colors());
            }
            self.synchronize_pointer_admission(view, port, mode)?;
            return Ok(DispatchOutcome::Continue);
        }
        if let DecodedKind::Focus(focused) = item.kind {
            // Preserve the active-drag edge before focus teardown installs its
            // cleanup tombstone; that edge owns the rest of this host packet.
            let tab_drag_cancelled = !focused
                && (self.pointer.cancel_tab_drag() | self.pointer.cancel_sidebar_reorder());
            self.pointer.set_host_focus(focused);
            if !focused {
                view.cancel_prefix();
                self.dismiss_fenced_introducer();
                self.pointer.cancel_list_scroll();
                self.pointer.cancel_modal_press();
                self.pointer.cancel_scrollbar();
            }
            if !focused {
                self.menu_release = None;
                self.navigator.release = None;
                if tab_drag_cancelled {
                    self.menu_packet = true;
                    self.keys.capture_pending();
                }
            }
            self.flush_pending(view, port);
            port.observe_terminal(TerminalObservation::Focus(focused));
            self.settle_capture_edge(view, port);
            return Ok(DispatchOutcome::Continue);
        }
        if let DecodedKind::Mouse(event) = item.kind {
            if self.pointer_capture != PointerCaptureRequest::Release
                && !self.pointer_transition_failed
                && event.action == super::MouseAction::Motion
                && event.button == super::MouseButton::None
                && event.mods == Modifiers::NONE
            {
                // Native console movement is already delivered independently of
                // the VT query. Do not discard it or change any host capture mode.
                self.native_motion_observed = true;
                self.pointer
                    .set_motion_observability(MotionObservability::AnyMotion);
            }
            match self.pointer.cancelled_modal_event(event) {
                CancelledModalEvent::Ended => {
                    // The painted owner and the packet-level button debt are
                    // one gesture. Settle both before a newer surface can
                    // reinterpret the same physical release.
                    self.modal_release = None;
                    {
                        self.menu_release = None;
                    }
                    self.pointer_packet = true;
                    self.keys.capture_pending();
                    return Ok(DispatchOutcome::Continue);
                }
                CancelledModalEvent::Captured => return Ok(DispatchOutcome::Continue),
                CancelledModalEvent::Retired => {
                    // A fresh press proves the old release can no longer
                    // arrive. It remains eligible for the current owner.
                    self.modal_release = None;
                    {
                        self.menu_release = None;
                    }
                }
                CancelledModalEvent::Ignored => {}
            }
            match self.pointer.cancelled_ui_event(event) {
                PointerOutcome::InteractionEnded => {
                    // A click whose painted authority ended before its release reached nothing.
                    // When the user's own switch began holding its frame meanwhile, it is
                    // reported like a press on that frame.
                    if port.unresolved_press() {
                        view.report_input_held_for_navigation();
                    }
                    self.modal_release = None;
                    {
                        // A generic menu interaction and the legacy packet
                        // release latch describe the same physical button.
                        // Retire both when the generic owner settles it.
                        self.menu_release = None;
                    }
                    self.pointer_packet = true;
                    self.keys.capture_pending();
                    return Ok(DispatchOutcome::Continue);
                }
                PointerOutcome::ChromeGestureEnded => {
                    self.pointer_packet = true;
                    self.keys.capture_pending();
                    return Ok(DispatchOutcome::Continue);
                }
                PointerOutcome::TabGestureEnded | PointerOutcome::SidebarReorderEnded => {
                    self.menu_packet = true;
                    self.keys.capture_pending();
                    self.pointer.invalidate_map_with_port(port);
                    return Ok(DispatchOutcome::Continue);
                }
                PointerOutcome::Handled => return Ok(DispatchOutcome::Continue),
                PointerOutcome::Ignored => {}
                _ => unreachable!("cancelled UI cleanup cannot produce a semantic effect"),
            }
            if self.capture_modal_release(event) {
                return Ok(DispatchOutcome::Continue);
            }
        }
        if matches!(
            &item.kind,
            DecodedKind::Key(_) | DecodedKind::Paste { .. } | DecodedKind::RejectedPaste
        ) {
            // The key closes a pointer tooltip and continues to its owner below.
            self.pointer.dismiss_tooltip();
            self.pointer.flush_list_scroll(view, port);
            self.pointer.cancel_list_scroll();
            self.pointer.cancel_modal_press();
            self.pointer.cancel_scrollbar();
        }
        if let DecodedKind::Mouse(event) = item.kind
            && event.action == super::MouseAction::Press
            && !matches!(
                event.button,
                super::MouseButton::WheelUp
                    | super::MouseButton::WheelDown
                    | super::MouseButton::WheelLeft
                    | super::MouseButton::WheelRight
            )
            && !self.menu_packet
            && !item.owned
            && self.menu_release.is_some()
        {
            // A new physical press proves that an unreported old menu release can no
            // longer complete that gesture. The current owner may immediately arm a
            // new debt for this press below.
            self.menu_release = None;
        }
        if let DecodedKind::Mouse(event) = item.kind
            && event.action == super::MouseAction::Release
            && Self::projected_capture(view, port)
                != Some(CaptureOwner::Native(WorkbenchCaptureOwner::ActionMenu))
            && self.menu_release.is_some_and(|button| {
                event.button == button || event.button == super::MouseButton::None
            })
        {
            self.menu_release = None;
            return Ok(DispatchOutcome::Continue);
        }
        // A confirmation owns its complete decision packet, including keys
        // and paste, but an accepted painted press must still see its exact
        // matching release before that packet fence swallows the tail.
        if let DecodedKind::Mouse(event) = item.kind
            && event.action == super::MouseAction::Release
            && self.modal_release.is_some_and(|button| {
                event.button == button || event.button == super::MouseButton::None
            })
            && self.pointer.modal_release_ownership() == ReleaseOwnership::Active
            && let Some(
                owner @ CaptureOwner::Native(
                    WorkbenchCaptureOwner::PanelClose
                    | WorkbenchCaptureOwner::TabClose
                    | WorkbenchCaptureOwner::WorkspaceClose,
                ),
            ) = Self::projected_capture(view, port)
        {
            self.flush_pending(view, port);
            return self.handle_projected_capture(
                view,
                port,
                ProjectedCaptureItem { owner, item, now },
                after_layout,
                mode,
            );
        }
        // An accepted generic Help press owns its exact release even when
        // intervening packet bytes were fenced. Route that release before
        // `item.owned` can consume it as ordinary tail.
        if let DecodedKind::Mouse(event) = item.kind
            && event.action == super::MouseAction::Release
            && self.modal_release.is_some_and(|button| {
                event.button == button || event.button == super::MouseButton::None
            })
            && self.pointer.modal_release_ownership() == ReleaseOwnership::Active
            && view.overlay() == Some(crate::client_view::Overlay::ShortcutHelp)
            && client_chrome_overlay_allowed(Self::projected_capture(view, port))
        {
            self.flush_pending(view, port);
            let captured = self.capture_help_mouse(view, port, event);
            debug_assert!(captured, "an active Help release remains Help-owned");
            return Ok(DispatchOutcome::Continue);
        }
        if self.capture_navigator_release(&item) {
            return Ok(DispatchOutcome::Continue);
        }
        if item.owned {
            // The item completes a frame that began inside a captured
            // packet: consumed by that capture, never forwarded or decided.
            self.flush_pending(view, port);
            self.fence_chord(view, &item);
            return Ok(DispatchOutcome::Continue);
        }
        let exclusive_packet = self.confirm_packet || self.settings_packet;
        let exclusive_packet = exclusive_packet
            || self.machine_packet
            || self.popup_packet
            || self.copy_packet
            || self.menu_packet;
        if exclusive_packet {
            self.flush_pending(view, port);
            self.fence_chord(view, &item);
            return Ok(DispatchOutcome::Continue);
        }
        let projected = Self::projected_capture(view, port);
        if view.overlay() == Some(crate::client_view::Overlay::ShortcutHelp)
            && client_chrome_overlay_allowed(projected)
        {
            match &item.kind {
                DecodedKind::Key(event) if event.kind == KeyKind::Release => {
                    self.flush_pending(view, port);
                    return Ok(DispatchOutcome::Continue);
                }
                DecodedKind::Key(_) | DecodedKind::Nondecision => {
                    self.flush_pending(view, port);
                    return self.decide(view, port, now, item, after_layout, mode);
                }
                DecodedKind::Paste { .. } | DecodedKind::RejectedPaste => {
                    self.flush_pending(view, port);
                    return Ok(DispatchOutcome::Continue);
                }
                DecodedKind::Ignored
                | DecodedKind::Focus(_)
                | DecodedKind::HostReply(_)
                | DecodedKind::Mouse(_) => {}
            }
        }
        if projected == Some(CaptureOwner::Native(WorkbenchCaptureOwner::AgentBoard))
            && matches!(
                &item.kind,
                DecodedKind::Key(event)
                    if event.kind != KeyKind::Release
                        && (view.prefix_active() || view.bindings().prefix.matches(*event))
            )
        {
            self.flush_pending(view, port);
            let DecodedKind::Key(event) = item.kind else {
                unreachable!("the Agent Board prefix route accepts only keys")
            };
            let transition = view.decide_agent_board_prefix(PrefixInput::Key(event));
            return self.apply_transition(
                view,
                port,
                now,
                transition,
                Some(&item),
                after_layout,
                mode,
            );
        }
        if let Some(owner) = projected {
            if client_chrome_overlay_allowed(Some(owner))
                && let DecodedKind::Mouse(event) = &item.kind
                && self.capture_help_mouse(view, port, *event)
            {
                self.flush_pending(view, port);
                return Ok(DispatchOutcome::Continue);
            }
            self.flush_pending(view, port);
            return self.handle_projected_capture(
                view,
                port,
                ProjectedCaptureItem { owner, item, now },
                after_layout,
                mode,
            );
        }
        if view.machines.direct_selection_pending()
            && let DecodedKind::Key(event) = item.kind
            && crate::client_view::Binding::new(KeyCode::Esc, Modifiers::NONE).matches(event)
            && event.kind != KeyKind::Release
        {
            self.flush_pending(view, port);
            view.machines.cancel_selection();
            self.machine_packet = true;
            self.keys.capture_pending();
            self.pointer.invalidate_map_with_port(port);
            return Ok(DispatchOutcome::Continue);
        }
        if match &item.kind {
            DecodedKind::Key(event) => event.kind != KeyKind::Release,
            DecodedKind::Paste { .. } | DecodedKind::RejectedPaste => true,
            DecodedKind::Mouse(event) => {
                event.action != super::mouse::MouseAction::Release
                    && event.button != super::mouse::MouseButton::None
            }
            _ => false,
        } {
            port.observe_terminal(TerminalObservation::InputIntent);
        }
        if let DecodedKind::Mouse(event) = &item.kind
            && self.capture_help_mouse(view, port, *event)
        {
            self.flush_pending(view, port);
            return Ok(DispatchOutcome::Continue);
        }
        if self.navigator.packet || view.navigator_active() {
            self.flush_pending(view, port);
            if self.navigator.packet {
                self.fence_chord(view, &item);
            } else {
                if let Some(transition) = self.navigator_item(view, port, &item) {
                    return self.apply_transition(
                        view,
                        port,
                        now,
                        transition,
                        Some(&item),
                        after_layout,
                        mode,
                    );
                }
                self.reconcile(view, port, now);
                after_layout(view, port)?;
            }
            return Ok(DispatchOutcome::Continue);
        }
        if (self.pointer.tab_drag_captured() || self.pointer.sidebar_reorder_captured())
            && !matches!(item.kind, DecodedKind::Mouse(_))
        {
            self.flush_pending(view, port);
            if matches!(item.kind, DecodedKind::Key(event) if event.kind != KeyKind::Release
                && event.code == crate::client_view::KeyCode::Esc && event.mods == crate::client_view::Modifiers::NONE)
            {
                self.pointer.cancel_tab_drag();
                self.pointer.escape_sidebar_reorder();
                self.menu_packet = true;
            }
            self.keys.capture_pending();
            return Ok(DispatchOutcome::Continue);
        }
        if matches!(item.kind, DecodedKind::Key(event) if event.kind != KeyKind::Release
            && event.code == crate::client_view::KeyCode::Esc && event.mods == crate::client_view::Modifiers::NONE)
        {
            let sidebar_ended = self.pointer.cancel_sidebar_width();
            let divider_ended = self.pointer.cancel_divider();
            if sidebar_ended || divider_ended {
                self.flush_pending(view, port);
                if sidebar_ended {
                    self.capture_sidebar_redraw(port);
                } else {
                    self.pointer_packet = true;
                    self.keys.capture_pending();
                }
                return Ok(DispatchOutcome::Continue);
            }
        }
        if Self::tab_list_owns(view, &item) {
            return self.tab_list_item(view, port, now, item, after_layout, mode);
        }
        // The mobile list and the narrow overlay cover the terminal, so a key or paste reaching
        // them returns to the sidebar's navigation instead of reaching the terminal hidden beneath,
        // after a pointer press or a closed dialog left that navigation. The prefix and its binding
        // never reach the terminal, so they decide from where the keyboard is: focusing the
        // sidebar then enters its navigation instead of leaving navigation this key re-entered.
        let binding_key = matches!(item.kind, DecodedKind::Key(key)
            if view.prefix_active() || view.bindings().prefix.matches(key));
        if !self.sidebar_packet
            && !matches!(item.kind, DecodedKind::Mouse(_))
            && !binding_key
            && view.sidebar_covers_terminal()
            && !view.machines.sidebar_navigation_active()
        {
            mode(ModeRequest::BracketedPaste(true))?;
            view.machines.enter_sidebar_navigation();
        }
        if self.sidebar_packet
            || (view.machines.sidebar_navigation_active()
                && !matches!(item.kind, DecodedKind::Mouse(_)))
        {
            self.flush_pending(view, port);
            if self.sidebar_packet {
                self.fence_chord(view, &item);
            } else {
                self.pointer.cancel_sidebar_width();
                // Sidebar navigation keeps ordinary keys and paste, but the configured
                // prefix still opens the shell binding layer. Exclusive captures above
                // this branch retain first refusal, including their packet tails.
                if let DecodedKind::Key(key) = item.kind
                    && (view.prefix_active() || view.bindings().prefix.matches(key))
                {
                    if key.kind == KeyKind::Release {
                        return Ok(DispatchOutcome::Continue);
                    }
                    return self.decide(view, port, now, item, after_layout, mode);
                }
                let intent = match item.kind {
                    DecodedKind::Key(key) if !view.sidebar_help_key(key) => {
                        view.machines.sidebar_key(key)
                    }
                    _ => crate::machine_picker::SidebarIntent::None,
                };
                // Leaving a covering sidebar's navigation, as Esc does, closes it and returns the
                // keyboard to the Panel it covered.
                if !view.machines.sidebar_navigation_active() && view.sidebar_covers_terminal() {
                    view.dismiss_overlay();
                    after_layout(view, port)?;
                }
                match intent {
                    crate::machine_picker::SidebarIntent::NewWorkspace => {
                        self.sidebar_packet = true;
                        self.keys.capture_pending();
                        self.pointer.invalidate_map_with_port(port);
                        let transition = view.decide_action(ClientAction::RequestQuickWorkspace);
                        return self.apply_transition(
                            view,
                            port,
                            now,
                            transition,
                            None,
                            after_layout,
                            mode,
                        );
                    }
                    crate::machine_picker::SidebarIntent::Search
                        if view.capabilities().offers_search() =>
                    {
                        view.machines.leave_sidebar_navigation();
                        view.dismiss_overlay();
                        view.machines.show_workspaces();
                        self.machine_packet = true;
                        self.keys.capture_pending();
                        self.pointer.invalidate_map_with_port(port);
                    }
                    crate::machine_picker::SidebarIntent::Resize(change) => {
                        if let Some(target) = self.pointer.sidebar_width_target(view)
                            && view
                                .machines
                                .set_sidebar_width(target.preferred(), target.adjusted(change))
                        {
                            self.capture_sidebar_redraw(port);
                            after_layout(view, port)?;
                        }
                    }
                    // A withheld Search key behaves as an unbound key.
                    crate::machine_picker::SidebarIntent::None
                    | crate::machine_picker::SidebarIntent::Search => {
                        // Focus/window changes invalidate the row geometry until repaint.
                        self.pointer.invalidate_map_with_port(port);
                    }
                    crate::machine_picker::SidebarIntent::Help => {
                        let transition = view.decide_action(ClientAction::OpenShortcutHelp);
                        view.commit(transition, ClientActionDisposition::KeepNavigator);
                        self.pointer.invalidate_map_with_port(port);
                    }
                    crate::machine_picker::SidebarIntent::Activate(selected) => {
                        view.machines.activate_sidebar_selection(&selected);
                        view.machines.leave_sidebar_navigation();
                        view.dismiss_overlay();
                        self.machine_packet = true;
                        self.keys.capture_pending();
                        self.pointer.invalidate_map_with_port(port);
                        after_layout(view, port)?;
                    }
                    crate::machine_picker::SidebarIntent::Reorder { target, up } => {
                        if target.machine() == &view.machines.current
                            && let Some(crate::action_menu::MenuTarget::Drawn {
                                source: painted,
                                ..
                            }) = self.pointer.sidebar_menu_target(view, &target)
                            && let Some(source) = port.sidebar_reorder_source()
                            && source.matches_menu_source(&painted)
                            && source.matches_sidebar(
                                &view.machines.sidebar_source(),
                                &view.machines.current,
                            )
                            && let Some(operation) = source.step(&target, up)
                        {
                            port.surface_action(SurfaceAction::Layout(
                                LayoutAction::SidebarReorder {
                                    source: &source,
                                    operation,
                                },
                            ));
                        }
                    }
                    crate::machine_picker::SidebarIntent::OpenMenu(target) => {
                        // A keyboard row menu opens beside the row that holds the focus.
                        let anchor =
                            self.pointer
                                .sidebar_menu_anchor(&target)
                                .map(|(column, row)| crate::action_menu::MenuAnchor::Row {
                                    column,
                                    row,
                                });
                        if let Some(target) = self.pointer.sidebar_menu_target(view, &target) {
                            mode(ModeRequest::BracketedPaste(true))?;
                            port.observe_sidebar_git(view.machines.sidebar_git_directory(&target));
                            if port.open_action_menu(target, anchor, view.pointer_released()) {
                                self.menu_packet = true;
                                self.keys.capture_pending();
                                self.pointer.invalidate_map_with_port(port);
                            } else {
                                mode(ModeRequest::BracketedPaste(view.surface_owns_input()))?;
                            }
                        }
                    }
                }
            }
            self.reconcile(view, port, now);
            return Ok(DispatchOutcome::Continue);
        }
        let hover = matches!(&item.kind, DecodedKind::Mouse(event)
            if fenced_chord::button_free_motion(event.action, event.button));
        if view.prefix_active() && !hover {
            self.flush_pending(view, port);
            match item.kind {
                DecodedKind::Key(event) if event.kind != KeyKind::Release => {}
                DecodedKind::Key(_) => {
                    // A release under an armed prefix is consumed without a
                    // decision; the prefix stays armed for the next press.
                    return Ok(DispatchOutcome::Continue);
                }
                DecodedKind::Paste { .. } => {
                    // Paste is exclusive-capture data, never a binding key;
                    // the pane owns it in original order. The interrupted
                    // chord retires before later typed input reaches the pane.
                    view.cancel_prefix();
                    forward_or_report(view, &item.raw, port);
                    return Ok(DispatchOutcome::Continue);
                }
                DecodedKind::RejectedPaste => {
                    // A refusal is not a prefix decision; the next real key still owns it.
                    return Ok(DispatchOutcome::Continue);
                }
                DecodedKind::Mouse(_) => {}
                DecodedKind::Ignored | DecodedKind::Focus(_) | DecodedKind::HostReply(_) => {}
                DecodedKind::Nondecision => {}
            }
            return self.decide(view, port, now, item, after_layout, mode);
        }
        match item.kind {
            DecodedKind::Key(event)
                if event.kind != KeyKind::Release && view.bindings().prefix.matches(event) =>
            {
                self.flush_pending(view, port);
                return self.decide(view, port, now, item, after_layout, mode);
            }
            DecodedKind::Key(event) if event.kind == KeyKind::Release => {
                // Releases are consumed, never dispatched or forwarded.
            }
            DecodedKind::Mouse(event) => {
                // Pointer reports are semantic input, never raw-forwarded:
                // pending bytes flush first so pane-bound reports preserve
                // packet order, then the pointer state resolves the event
                // against the last drawn frame's hit map. A handled event
                // syncs the viewport like a keypress; a region-changing one
                // also stales the map while dedicated gesture owners retain
                // their cleanup path.
                self.flush_pending(view, port);
                let capture_changed = self.settle_capture_edge(view, port);
                match self.pointer.event(view, port, now, event) {
                    PointerOutcome::Ignored => {}
                    PointerOutcome::IdleMotion { changed } => {
                        let layout_changed = after_layout(view, port)?;
                        if layout_changed {
                            self.pointer.invalidate_map_with_port(port);
                        }
                        self.item_idle_motion = Some(changed || layout_changed || capture_changed);
                    }
                    PointerOutcome::Interaction { target, contract } => {
                        self.pointer_packet = true;
                        self.keys.capture_pending();
                        self.pointer.invalidate_map_with_port(port);
                        if contract == PrimaryContract::ActivateOnRelease {
                            match target {
                                InteractionTarget::Presentation(target) => {
                                    let disposition = port
                                        .surface_action(SurfaceAction::Presentation(&target))
                                        .unwrap_or(ClientActionDisposition::KeepNavigator);
                                    view.commit_pointer(disposition);
                                    self.reconcile(view, port, now);
                                    if disposition.changes_layout() {
                                        after_layout(view, port)?;
                                    }
                                }
                                InteractionTarget::HeldRestore(target)
                                    if Self::projected_capture(view, port).is_none() =>
                                {
                                    let disposition = port
                                        .surface_action(SurfaceAction::HeldRestore(&target))
                                        .unwrap_or(ClientActionDisposition::KeepNavigator);
                                    view.commit_pointer(disposition);
                                    self.reconcile(view, port, now);
                                    if disposition.changes_layout() {
                                        after_layout(view, port)?;
                                    }
                                }
                                InteractionTarget::GlobalChrome(action) => {
                                    if Self::projected_capture(view, port).is_none() {
                                        let action = match action {
                                            crate::tui_hit_map::GlobalChromeAction::Onboarding(action) => view.onboarding.visible.then(|| action.client()),
                                            crate::tui_hit_map::GlobalChromeAction::RestoreSidebar
                                                if view.machines.available
                                                    && view.sidebar_surface()
                                                        == crate::client_view::SidebarSurface::Hidden =>
                                            {
                                                Some(ClientAction::ToggleBusinessSidebar)
                                            }
                                            crate::tui_hit_map::GlobalChromeAction::RestoreSidebar => {
                                                None
                                            }
                                            crate::tui_hit_map::GlobalChromeAction::HideSidebar => {
                                                (view.machines.available
                                                    && view.sidebar_surface()
                                                        != crate::client_view::SidebarSurface::Hidden)
                                                    .then_some(ClientAction::ToggleBusinessSidebar)
                                            }
                                            crate::tui_hit_map::GlobalChromeAction::ShortcutHelp => {
                                                (view.overlay()
                                                    != Some(crate::client_view::Overlay::ShortcutHelp))
                                                .then_some(ClientAction::OpenShortcutHelp)
                                            }
                                            // The Tab list is the strip's own presentation; opening
                                            // or leaving it changes no Workbench state.
                                            crate::tui_hit_map::GlobalChromeAction::OpenTabList => {
                                                // Paste reaches the list, never the terminal it covers.
                                                if view.open_tab_list() {
                                                    mode(ModeRequest::BracketedPaste(true))?;
                                                }
                                                None
                                            }
                                            crate::tui_hit_map::GlobalChromeAction::CloseTabList => {
                                                view.close_tab_list();
                                                None
                                            }
                                        };
                                        if let Some(action) = action {
                                            let transition = view.decide_action(action);
                                            return self.apply_transition(
                                                view,
                                                port,
                                                now,
                                                transition,
                                                None,
                                                after_layout,
                                                mode,
                                            );
                                        }
                                    }
                                }

                                // Only the card the press armed opens; a newer card waits for its own.
                                InteractionTarget::NotificationCard(token)
                                    if Self::projected_capture(view, port).is_none()
                                        && view
                                            .notifications
                                            .agent_card()
                                            .is_some_and(|(current, _)| current == token) =>
                                {
                                    let transition =
                                        view.decide_action(ClientAction::OpenNotificationTarget);
                                    return self.apply_transition(
                                        view,
                                        port,
                                        now,
                                        transition,
                                        None,
                                        after_layout,
                                        mode,
                                    );
                                }
                                InteractionTarget::TabCloseRequest { source, tab } => {
                                    self.menu_packet = true;
                                    port.surface_action(SurfaceAction::Tabs(
                                        TabAction::RequestClose {
                                            source: &source,
                                            tab: &tab,
                                        },
                                    ));
                                }
                                InteractionTarget::TabStripPage { source, start } => {
                                    self.menu_packet = true;
                                    port.surface_input(SurfaceInput::TabStripPage {
                                        source: &source,
                                        start: &start,
                                    });
                                }
                                InteractionTarget::TabCreate(target) => {
                                    // Creation can replace the input target. A stale
                                    // exact witness still owns this physical packet.
                                    self.menu_packet = true;
                                    if let Some(action) = port.tab_create_action(&target) {
                                        let transition = view.decide_action(action);
                                        return self.apply_transition(
                                            view,
                                            port,
                                            now,
                                            transition,
                                            None,
                                            after_layout,
                                            mode,
                                        );
                                    }
                                }
                                InteractionTarget::Sidebar(drawn) => {
                                    use crate::machine_picker::SidebarActivation;

                                    let activation = view.machines.sidebar_activation(&drawn);
                                    if let Some(SidebarActivation::CloseWorkspace(workspace)) =
                                        activation
                                    {
                                        // The close owner opens its confirmation; this packet ends here.
                                        self.menu_packet = true;
                                        let transition = view.decide_action(
                                            ClientAction::RequestWorkspaceCloseAt(workspace),
                                        );
                                        return self.apply_transition(
                                            view,
                                            port,
                                            now,
                                            transition,
                                            None,
                                            after_layout,
                                            mode,
                                        );
                                    }
                                    let changed = match activation {
                                        Some(SidebarActivation::ToggleGroup(target)) => {
                                            view.machines.toggle_sidebar_group(&target)
                                        }
                                        Some(SidebarActivation::ToggleContext(target)) => {
                                            view.machines.toggle_sidebar_context_agents(&target)
                                        }
                                        Some(SidebarActivation::Activate(selection)) => {
                                            let changed = view
                                                .machines
                                                .activate_sidebar_selection(&selection);
                                            // Choosing a row closes the mobile list even when it
                                            // is already the current one.
                                            let closes = changed || view.sidebar_screen();
                                            if closes {
                                                view.machines.leave_sidebar_navigation();
                                                view.dismiss_overlay();
                                                self.machine_packet = true;
                                            }
                                            closes
                                        }
                                        Some(SidebarActivation::CloseWorkspace(_)) | None => false,
                                    };
                                    if changed {
                                        self.reconcile(view, port, now);
                                        after_layout(view, port)?;
                                    }
                                }
                                InteractionTarget::SidebarFixed(drawn) => {
                                    use crate::machine_picker::SidebarFixedActivation;

                                    match view.machines.sidebar_fixed_activation(&drawn) {
                                        Some(SidebarFixedActivation::NewWorkspace) => {
                                            let transition = view
                                                .decide_action(ClientAction::RequestQuickWorkspace);
                                            return self.apply_transition(
                                                view,
                                                port,
                                                now,
                                                transition,
                                                None,
                                                after_layout,
                                                mode,
                                            );
                                        }
                                        Some(SidebarFixedActivation::Search)
                                            if view.capabilities().offers_search() =>
                                        {
                                            view.machines.leave_sidebar_navigation();
                                            view.dismiss_overlay();
                                            view.machines.show_workspaces();
                                            self.machine_packet = true;
                                            self.reconcile(view, port, now);
                                            after_layout(view, port)?;
                                        }
                                        Some(SidebarFixedActivation::Settings) => {
                                            let transition =
                                                view.decide_action(ClientAction::OpenSettings);
                                            return self.apply_transition(
                                                view,
                                                port,
                                                now,
                                                transition,
                                                None,
                                                after_layout,
                                                mode,
                                            );
                                        }
                                        Some(SidebarFixedActivation::Search) | None => {}
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    PointerOutcome::InteractionEnded => {
                        self.pointer_packet = true;
                        self.keys.capture_pending();
                    }
                    outcome => {
                        if after_layout(view, port)? {
                            // A host sample can change dimensions inside this read,
                            // even when the Client viewport class stays unchanged.
                            self.pointer.invalidate_map_with_port(port);
                        }
                        if matches!(
                            outcome,
                            PointerOutcome::TabGestureEnded | PointerOutcome::SidebarReorderEnded
                        ) {
                            self.menu_packet = true;
                            self.keys.capture_pending();
                            self.pointer.invalidate_map_with_port(port);
                        }
                        if outcome == PointerOutcome::ChromeGestureEnded {
                            self.pointer_packet = true;
                            self.keys.capture_pending();
                        }
                        if let PointerOutcome::MenuRequested {
                            target,
                            anchor,
                            button,
                        } = &outcome
                        {
                            mode(ModeRequest::BracketedPaste(true))?;
                            port.observe_sidebar_git(view.machines.sidebar_git_directory(target));
                            if port.open_action_menu(
                                target.clone(),
                                Some(crate::action_menu::MenuAnchor::from(*anchor)),
                                view.pointer_released(),
                            ) {
                                self.menu_packet = true;
                                self.menu_release = Some(*button);
                                self.keys.capture_pending();
                                self.pointer.invalidate_map_with_port(port);
                            } else {
                                mode(ModeRequest::BracketedPaste(view.surface_owns_input()))?;
                            }
                        }
                        if outcome == PointerOutcome::RegionsChanged {
                            self.pointer.stale_map();
                        }
                        if outcome == PointerOutcome::SidebarWidthEnded {
                            self.capture_sidebar_redraw(port);
                        }
                    }
                }
            }
            DecodedKind::Ignored | DecodedKind::RejectedPaste => {
                // Rejected protocol input never becomes keys or pending pane bytes.
            }
            // A non-Live focus pane owns its history navigation keys
            // locally: the port scrolls or exits without encoding
            // anything, then pending pane bytes flush in order — a stale
            // read outcome the flush's recovery turn produces is
            // discarded by the core. Live panes always report `false`, so
            // their keys keep the ordinary byte path untouched.
            DecodedKind::Key(event)
                if event.kind != KeyKind::Release
                    && port.pane_input(PaneInput::HistoryKey(&event))
                        == PaneAdmission::Admitted =>
            {
                self.flush_pending(view, port);
            }
            DecodedKind::Key(event) if view.bindings().find(event, false).is_some() => {
                self.flush_pending(view, port);
                return self.decide(view, port, now, item, after_layout, mode);
            }
            _ => {
                self.pending.extend_from_slice(&item.raw);
            }
        }
        Ok(DispatchOutcome::Continue)
    }

    /// Applies one `ClientView` decision plus the action shell effects that
    /// used to be inlined in the byte loop.
    fn decide<Port: TerminalInputPort + ?Sized, Error>(
        &mut self,
        view: &mut ClientView,
        port: &mut Port,
        now: Instant,
        item: DecodedItem,
        after_layout: &mut impl FnMut(&mut ClientView, &mut Port) -> Result<bool, Error>,
        mode: &mut impl FnMut(ModeRequest) -> Result<(), Error>,
    ) -> Result<DispatchOutcome, Error> {
        let input = match item.kind {
            DecodedKind::Key(event) if event.kind != KeyKind::Release => PrefixInput::Key(event),
            _ => PrefixInput::NonKey,
        };
        let transition = view.decide(input);
        self.apply_transition(view, port, now, transition, Some(&item), after_layout, mode)
    }

    #[allow(clippy::too_many_arguments)]
    fn apply_transition<Port: TerminalInputPort + ?Sized, Error>(
        &mut self,
        view: &mut ClientView,
        port: &mut Port,
        now: Instant,
        decision: impl Into<ActionDecision>,
        item: Option<&DecodedItem>,
        after_layout: &mut impl FnMut(&mut ClientView, &mut Port) -> Result<bool, Error>,
        mode: &mut impl FnMut(ModeRequest) -> Result<(), Error>,
    ) -> Result<DispatchOutcome, Error> {
        let ActionDecision {
            transition,
            captured,
        } = decision.into();
        let action = transition.action();
        if action == Some(ClientAction::DismissOnboarding) {
            view.commit(transition, ClientActionDisposition::KeepNavigator);
            if view.onboarding.dismiss().is_err() {
                view.startup_notice(crate::client_view::Notice::OnboardingSaveFailed);
            }
            self.pointer.invalidate_map_with_port(port);
            return Ok(DispatchOutcome::Continue);
        }
        if action != Some(ClientAction::OpenActionMenu) && !transition.prefix_active() {
            // A prefix has not chosen an action yet. Preserve the active drag
            // until the decision, so menu admission can observe its owner.
            self.pointer.cancel_divider();
            self.pointer.cancel_sidebar_width();
        }
        let requests_copy = action == Some(ClientAction::EnterCopyMode);

        let requests_navigator = action == Some(ClientAction::OpenWorkbenchNavigator);

        let board_action_handoff = matches!(
            action.as_ref(),
            Some(ClientAction::OpenActionMenu | ClientAction::FocusBusinessSidebar)
        ) && Self::projected_capture(view, port)
            == Some(CaptureOwner::Native(WorkbenchCaptureOwner::AgentBoard));
        if action == Some(ClientAction::EnterCopyMode) {
            self.cancel_pointer(port);
            self.copy_packet = true;
            self.keys.capture_pending();
        }
        let layout_changed = transition.layout_changed();
        let shell = transition.shell();
        let requests_modal = matches!(
            action,
            Some(
                ClientAction::RequestRenameTab
                    | ClientAction::RequestRenamePanel
                    | ClientAction::OpenSettings
                    | ClientAction::ToggleAgentBoard
                    | ClientAction::StartAgent
                    | ClientAction::RequestWorkspaceCreate
                    | ClientAction::RequestQuickWorkspace
                    | ClientAction::RequestWorkspaceAtDirectory
                    | ClientAction::RequestProjectQuickWorkspace
                    | ClientAction::RequestWorkspaceDirectory
                    | ClientAction::RequestWorkspaceRename
                    | ClientAction::RequestProjectRename
                    | ClientAction::RequestTargetedProjectRename
                    | ClientAction::RequestWorkspaceOrganization
                    | ClientAction::RequestProjectCloseAll(_)
                    | ClientAction::RequestWorkspaceCloseAt(_)
                    | ClientAction::RequestSidebarWorkspaceDelete
            )
        );
        let requests_modal = requests_modal
            || matches!(
                action,
                Some(
                    ClientAction::RequestWorktreeCreate
                        | ClientAction::RequestWorktreeOpen
                        | ClientAction::OpenActionMenu
                        | ClientAction::FocusBusinessSidebar
                )
            );
        if requests_modal || requests_copy || requests_navigator {
            // The terminal lease must exist before the state can capture any draft authority.
            mode(ModeRequest::BracketedPaste(true))?;
        }
        if requests_navigator {
            self.cancel_pointer(port);
        }
        let applied = action.clone().and_then(|action| {
            // Without an Agent card the action has nothing to open and leaves every surface as is.
            if action == ClientAction::OpenNotificationTarget
                && view.notifications.agent_card().is_none()
            {
                return Some(ClientActionDisposition::KeepNavigator);
            }
            if action == ClientAction::FocusBusinessSidebar {
                return if board_action_handoff {
                    port.apply_client_action(action.into())
                } else {
                    Some(ClientActionDisposition::KeepNavigator)
                };
            }
            if action == ClientAction::OpenActionMenu {
                let disposition = if board_action_handoff {
                    port.apply_client_action(action.clone().into())?
                } else {
                    ClientActionDisposition::KeepNavigator
                };
                if self.pointer.can_open_action_menu()
                    && !view.resize_mode()
                    && !port.copy_mode_active()
                    && Self::projected_capture(view, port).is_none()
                    && port.open_action_menu(
                        crate::action_menu::MenuTarget::Global,
                        None,
                        view.pointer_released(),
                    )
                {
                    self.menu_packet = true;
                    self.keys.capture_pending();
                    self.pointer.invalidate_map_with_port(port);
                }
                return Some(disposition);
            }
            port.apply_client_action(crate::workbench_client::InputAction::for_decision(
                action, captured,
            ))
        });
        view.commit(
            transition,
            applied.unwrap_or(ClientActionDisposition::KeepNavigator),
        );
        let opened = action == Some(ClientAction::OpenNotificationTarget)
            && self.open_notification_target(view, port);
        if action == Some(ClientAction::FocusBusinessSidebar) {
            self.sidebar_packet = true;
            self.keys.capture_pending();
            self.cancel_pointer(port);
            self.pointer.invalidate_map_with_port(port);
        }
        // Opening the mobile list gives it the keyboard, as focusing the sidebar does.
        if action == Some(ClientAction::ToggleBusinessSidebar)
            && view.machines.sidebar_navigation_active()
        {
            mode(ModeRequest::BracketedPaste(true))?;
            self.sidebar_packet = true;
            self.keys.capture_pending();
            self.pointer.invalidate_map_with_port(port);
        }
        if view.machines.available {
            match action {
                Some(ClientAction::OpenMachines)
                    if view
                        .capabilities()
                        .supports(crate::tui_capabilities::Capability::SavedMachines) =>
                {
                    view.machines.show()
                }
                Some(ClientAction::PreviousAgent | ClientAction::NextAgent) => view
                    .machines
                    .navigate_agent(action == Some(ClientAction::PreviousAgent), now),
                Some(ClientAction::FocusAgent(index)) => view.machines.focus_agent(index, now),
                _ => {}
            }
            if matches!(
                action,
                Some(
                    ClientAction::OpenMachines
                        | ClientAction::PreviousAgent
                        | ClientAction::NextAgent
                        | ClientAction::FocusAgent(_)
                )
            ) {
                self.machine_packet = true;
                self.pointer.invalidate_map_with_port(port);
            }
        }
        if action == Some(ClientAction::TogglePointerCapture) {
            // The client-local release entry: the mode write must succeed
            // before the settled flag mirrors it, exactly like the modal
            // leases above.
            let next = if self.pointer.capture_enabled() {
                PointerCaptureRequest::Release
            } else {
                self.admitted_pointer_capture()
            };
            mode(ModeRequest::PointerCapture(next))?;
            self.settle_pointer_capture(view, port, next);
            self.begin_pointer_admission(now, mode)?;
        }
        if applied.is_none()
            && let Some(action) = action.as_ref().and_then(ClientAction::backend_action)
        {
            view.report_backend_unavailable(action);
        }
        self.reconcile(view, port, now);
        let capture_admitted =
            self.epoch.is_some() || Self::projected_capture(view, port).is_some();
        let capture_admitted = capture_admitted || view.surface_owns_input();
        if (requests_modal || requests_copy || requests_navigator || item.is_none())
            && !capture_admitted
        {
            mode(ModeRequest::BracketedPaste(false))?;
        }
        if layout_changed || opened || applied.is_some_and(ClientActionDisposition::changes_layout)
        {
            after_layout(view, port)?;
            // Keyboard-driven layout changes stale the map the same way a
            // pointer navigation does.
            self.pointer.invalidate_map_with_port(port);
        }
        match shell {
            ShellCommand::Exit => {
                self.cancel_pointer(port);
                return Ok(DispatchOutcome::Exit);
            }
            ShellCommand::ForwardTerminal => {
                // The literal chord forwards its canonical C0 encoding, so a
                // future kitty-encoded Ctrl+B still emits the byte a pane
                // without keyboard negotiation understands.
                if let Some(item) = item
                    && let Some(byte) = literal_chord_byte(item)
                {
                    forward_or_report(view, std::slice::from_ref(&byte), port);
                } else if let Some(item) = item {
                    forward_or_report(view, &item.raw, port);
                }
            }
            ShellCommand::None => {}
        }
        Ok(DispatchOutcome::Continue)
    }

    /// Opens the visible Agent card's target through the revalidating port entry, which reports
    /// a held, busy, or unavailable outcome itself. Returns whether the navigation was admitted,
    /// which may change the painted layout.
    fn open_notification_target(
        &mut self,
        view: &mut ClientView,
        port: &mut (impl TerminalInputPort + ?Sized),
    ) -> bool {
        use crate::client_view::notifications::TargetOpening;

        let Some((token, target)) = view.notifications.agent_card() else {
            return false;
        };
        let target = target.clone();
        let opening = port.open_notification_target(&target);
        if opening == TargetOpening::Busy {
            // The busy notice answers; the card stays for the next attempt.
            return false;
        }
        // Opened, held, or refused as stale, the request no longer needs the card. A stale target
        // never falls through to another Panel.
        view.notifications.dismiss(token);
        // The card's cells leave the painted map with it.
        self.pointer.invalidate_map_with_port(port);
        opening == TargetOpening::Opened
    }

    fn flush_pending(
        &mut self,
        view: &mut ClientView,
        port: &mut (impl TerminalInputPort + ?Sized),
    ) {
        if self.pending.is_empty() {
            return;
        }
        let bytes = std::mem::take(&mut self.pending);
        forward_or_report(view, &bytes, port);
    }

    fn capture_sidebar_redraw(&mut self, port: &mut (impl TerminalInputPort + ?Sized)) {
        self.sidebar_packet = true;
        self.sidebar_cancel_capture = true;
        self.keys.capture_pending();
        self.pointer.invalidate_map_with_port(port);
    }

    fn capture_help_mouse(
        &mut self,
        view: &mut ClientView,
        port: &mut (impl TerminalInputPort + ?Sized),
        event: super::MouseEvent,
    ) -> bool {
        let outcome = self.pointer.help_event(view, event);
        if outcome == PointerOutcome::Ignored {
            return false;
        }
        if event.action == super::MouseAction::Press && event.button == super::MouseButton::Left {
            self.modal_release = Some(event.button);
            self.pointer_packet = true;
            self.keys.capture_pending();
        }
        match outcome {
            PointerOutcome::Interaction {
                target: InteractionTarget::Help(target),
                contract: PrimaryContract::ActivateOnRelease,
            } => {
                self.modal_release = None;
                self.pointer_packet = true;
                self.keys.capture_pending();
                if view.activate_drawn_help(&target) {
                    self.pointer.invalidate_map_with_port(port);
                }
            }
            PointerOutcome::Interaction { .. } | PointerOutcome::InteractionEnded => {
                // A stale or mismatched release is still owned by the painted
                // Help control. Consume its packet without re-hitting another surface.
                self.modal_release = None;
                self.pointer_packet = true;
                self.keys.capture_pending();
            }
            PointerOutcome::RegionsChanged => {
                self.pointer_packet = true;
                self.keys.capture_pending();
            }
            PointerOutcome::Handled | PointerOutcome::IdleMotion { .. } => {}
            PointerOutcome::Ignored => unreachable!("ignored Help input returned above"),
            PointerOutcome::TabGestureEnded
            | PointerOutcome::SidebarReorderEnded
            | PointerOutcome::SidebarWidthEnded
            | PointerOutcome::MenuRequested { .. }
            | PointerOutcome::ChromeGestureEnded => {
                unreachable!("Help routing cannot produce a Windows chrome outcome")
            }
        }
        true
    }

    fn reconcile(
        &mut self,
        view: &mut ClientView,
        port: &mut (impl TerminalInputPort + ?Sized),
        now: Instant,
    ) -> bool {
        use crate::workbench_client::CaptureContext;

        let capture_changed = self.settle_capture_edge(view, port);
        let projected = Self::projected_capture(view, port);
        let native_owner = match projected {
            Some(CaptureOwner::Native(owner)) => Some(owner),
            Some(CaptureOwner::Machine | CaptureOwner::Copy | CaptureOwner::Resize) => None,
            None => None,
        };
        let higher_exclusive = matches!(
            projected,
            Some(CaptureOwner::Machine | CaptureOwner::Native(_))
        );
        let context = native_owner.and_then(|owner| port.native_capture_context(owner));
        let setup = match context.as_ref() {
            Some(CaptureContext::Setup(context)) => Some(*context),
            _ => None,
        };
        let worktree = match context.as_ref() {
            Some(CaptureContext::Worktree(context)) => Some(*context),
            _ => None,
        };
        let current = match context.as_ref() {
            Some(CaptureContext::Setup(context)) => Some(ModalEpoch::Setup(context.token)),
            Some(CaptureContext::Rename(context)) => Some(ModalEpoch::Rename(context.token)),
            Some(CaptureContext::Worktree(context)) => Some(ModalEpoch::Worktree(context.token)),
            _ => None,
        };
        if higher_exclusive {
            view.exit_resize_mode();
            port.surface_action(SurfaceAction::CopyMode(CopyAction::Exit));
        }
        let capture_retains_sidebar =
            projected == Some(CaptureOwner::Native(WorkbenchCaptureOwner::ActionMenu));
        let capture_blocks_sidebar = projected.is_some() && !capture_retains_sidebar;
        if capture_blocks_sidebar {
            view.machines.leave_sidebar_navigation();
        }
        let navigator_changed = self.reconcile_navigator(view, port, projected.is_some());

        let sidebar_changed = {
            let owned = view.machines.sidebar_navigation_active();
            let changed = owned != self.sidebar_owned;
            if changed {
                self.sidebar_owned = owned;
                self.keys.capture_pending();
                self.pointer.invalidate_map_with_port(port);
                if !owned {
                    self.sidebar_packet = true;
                    self.sidebar_cancel_capture = true;
                }
            }
            changed
        };

        let choice_keys = setup.is_some_and(|context| {
            matches!(
                context.stage,
                SetupStage::QuickProject
                    | SetupStage::ChooseWorkspaceProject
                    | SetupStage::QuickDirectory
                    | SetupStage::QuickName
            )
        });
        let choice_keys = choice_keys
            || worktree.is_some_and(|context| {
                context.stage == crate::workbench_state::worktree_launch::WorktreeStage::Checkout
            });
        self.decoder.choice_keys(choice_keys);
        {
            if higher_exclusive {
                port.surface_action(SurfaceAction::CopyMode(CopyAction::Exit));
            }
            let owned = port.copy_mode_active();
            if self.copy_owned != owned {
                self.copy_owned = owned;
                self.keys.capture_pending();
                self.pointer.invalidate_map_with_port(port);
                if !owned {
                    self.copy_packet = true;
                }
            }
            view.settle_copy_mode(owned);
        }
        if current == self.epoch {
            return capture_changed || sidebar_changed || navigator_changed;
        }
        if self.epoch.take().is_some() {
            let events = self.decoder.cancel(now);
            self.apply_events(view, port, events, now);
        }
        let capacity = if setup.is_some_and(|context| {
            matches!(
                context.stage,
                SetupStage::DirectoryPath | SetupStage::QuickDirectory
            )
        }) {
            ModalCapacity::Bytes4096
        } else {
            ModalCapacity::Scalars512
        };
        let capacity = if matches!(current, Some(ModalEpoch::Worktree(_))) {
            ModalCapacity::Bytes1024
        } else {
            capacity
        };
        if let Some(token) = current
            && self.decoder.start_with_capacity(capacity)
        {
            view.exit_resize_mode();
            self.epoch = Some(token);
        }
        true
    }

    fn apply_events(
        &mut self,
        view: &mut ClientView,
        port: &mut (impl TerminalInputPort + ?Sized),
        events: Vec<ModalInputEvent>,
        now: Instant,
    ) {
        for event in events {
            if let ModalInputEvent::HostReply(reply) = event {
                if let Some(appearance) = self.host_reply(reply, now) {
                    view.configure_host_terminal(appearance, self.host_appearance.colors());
                }
                continue;
            }
            if let ModalInputEvent::Focus(focused) = event {
                self.pointer.set_host_focus(focused);
                if !focused {
                    view.cancel_prefix();
                    self.pointer.cancel_list_scroll();
                    self.pointer.cancel_modal_press();
                    self.pointer.cancel_scrollbar();
                }
                port.observe_terminal(TerminalObservation::Focus(focused));
                continue;
            }
            if event == ModalInputEvent::InputFailure {
                view.report_terminal_input_unavailable();
                continue;
            }
            if !matches!(
                event,
                ModalInputEvent::Mouse(_) | ModalInputEvent::Completeness(_)
            ) {
                if self.pointer.flush_list_scroll(view, port) {
                    self.epoch = match self.epoch {
                        Some(ModalEpoch::Setup(_)) => {
                            match port.native_capture_context(WorkbenchCaptureOwner::Setup) {
                                Some(crate::workbench_client::CaptureContext::Setup(context)) => {
                                    Some(ModalEpoch::Setup(context.token))
                                }
                                _ => None,
                            }
                        }
                        Some(ModalEpoch::Worktree(_)) => {
                            match port.native_capture_context(WorkbenchCaptureOwner::Worktree) {
                                Some(crate::workbench_client::CaptureContext::Worktree(
                                    context,
                                )) => Some(ModalEpoch::Worktree(context.token)),
                                _ => None,
                            }
                        }
                        other => other,
                    };
                }
                self.pointer.cancel_list_scroll();
            }
            let Some(epoch) = self.epoch else {
                continue;
            };
            if let ModalInputEvent::Mouse(event) = event {
                self.pointer.external_capture_event(event);
                let decision = self.pointer.modal_event(port, event);
                self.capture_modal_release(event);
                if event.action == super::MouseAction::Press
                    && matches!(
                        event.button,
                        super::MouseButton::Left
                            | super::MouseButton::Right
                            | super::MouseButton::Middle
                    )
                {
                    self.modal_release = Some(event.button);
                }
                if decision {
                    self.epoch = None;
                    let _events = self.decoder.cancel(now);
                    self.pointer.stale_map();
                } else {
                    // A wheel edit replaces selection authority but retains the
                    // same input owner. It never acknowledges the new token.
                    self.epoch = match epoch {
                        ModalEpoch::Setup(_) => {
                            match port.native_capture_context(WorkbenchCaptureOwner::Setup) {
                                Some(crate::workbench_client::CaptureContext::Setup(context)) => {
                                    Some(ModalEpoch::Setup(context.token))
                                }
                                _ => None,
                            }
                        }
                        ModalEpoch::Worktree(_) => {
                            match port.native_capture_context(WorkbenchCaptureOwner::Worktree) {
                                Some(crate::workbench_client::CaptureContext::Worktree(
                                    context,
                                )) => Some(ModalEpoch::Worktree(context.token)),
                                _ => None,
                            }
                        }
                        ModalEpoch::Rename(_) => Some(epoch),
                    };
                }
                continue;
            }
            if !matches!(event, ModalInputEvent::Completeness(_)) {
                self.pointer.cancel_modal_press();
            }
            let token = match epoch {
                ModalEpoch::Worktree(token) => {
                    self.apply_worktree_event(port, token, event);
                    continue;
                }
                ModalEpoch::Rename(token) => token,
                ModalEpoch::Setup(token) => {
                    self.apply_setup_event(port, token, event);
                    continue;
                }
            };
            let edit = match event {
                ModalInputEvent::Append(ref text) => RenameEdit::Append(text),
                ModalInputEvent::Paste(ref text) => RenameEdit::Paste(text),
                ModalInputEvent::Backspace => RenameEdit::Backspace,
                ModalInputEvent::Left => RenameEdit::Left,
                ModalInputEvent::Right => RenameEdit::Right,
                ModalInputEvent::Home => RenameEdit::Home,
                ModalInputEvent::End => RenameEdit::End,
                ModalInputEvent::Delete => RenameEdit::Delete,
                ModalInputEvent::Line(key) => RenameEdit::Line(key),
                ModalInputEvent::Fault => RenameEdit::Fault,
                ModalInputEvent::Completeness(complete) => RenameEdit::Completeness(complete),
                ModalInputEvent::Submit => {
                    let _disposition =
                        port.surface_action(SurfaceAction::Rename(RenameAction::Submit(token)));
                    continue;
                }
                ModalInputEvent::Cancel => {
                    port.surface_action(SurfaceAction::Rename(RenameAction::Cancel(token)));
                    continue;
                }
                ModalInputEvent::NextField
                | ModalInputEvent::PreviousChoice
                | ModalInputEvent::NextChoice
                | ModalInputEvent::InputFailure
                | ModalInputEvent::Mouse(_)
                | ModalInputEvent::Focus(_)
                | ModalInputEvent::HostReply(_) => continue,
            };
            let _accepted = port.surface_input(SurfaceInput::Rename { token, edit });
        }
    }

    fn apply_worktree_event(
        &mut self,
        port: &mut (impl TerminalInputPort + ?Sized),
        token: crate::workbench_state::worktree_launch::WorktreeToken,
        event: ModalInputEvent,
    ) {
        use crate::workbench_client::{CaptureContext, CaptureReadiness};
        use crate::workbench_state::worktree_launch::{WorktreeEdit as E, WorktreeStage as S};
        let Some(CaptureContext::Worktree(context)) =
            port.native_capture_context(WorkbenchCaptureOwner::Worktree)
        else {
            return;
        };
        if context.token != token {
            return;
        }
        if context.readiness == CaptureReadiness::Recovery {
            if event == ModalInputEvent::Cancel {
                port.surface_action(SurfaceAction::Worktree(WorktreeAction::Dismiss(token)));
            }
            return;
        }
        if matches!(
            context.stage,
            S::Remove | S::ResumeRemove | S::AbandonRemove
        ) {
            match event {
                ModalInputEvent::Submit => {
                    port.surface_action(SurfaceAction::Worktree(WorktreeAction::Submit(token)));
                }
                ModalInputEvent::Cancel => {
                    port.surface_action(SurfaceAction::Worktree(WorktreeAction::Dismiss(token)));
                }
                _ => {}
            }
            return;
        }
        let selecting = matches!(context.stage, S::Checkout);
        // Choice lists own framed reports in the decoder and have no partial
        // text to submit. A mouse/focus CSI must not revoke its own painted token.
        if selecting && matches!(event, ModalInputEvent::Completeness(_)) {
            return;
        }
        let edit = match event {
            ModalInputEvent::NextField => E::NextField,
            ModalInputEvent::PreviousChoice => E::Previous,
            ModalInputEvent::NextChoice => E::Next,
            ModalInputEvent::Append(ref text) if selecting => match text.as_str() {
                "[" => E::Previous,
                "]" => E::Next,
                _ => return,
            },
            ModalInputEvent::Append(ref text) => E::Append(text),
            ModalInputEvent::Paste(ref text) => E::Paste(text),
            ModalInputEvent::Backspace => E::Backspace,
            ModalInputEvent::Left => E::Left,
            ModalInputEvent::Right => E::Right,
            ModalInputEvent::Home => E::Home,
            ModalInputEvent::End => E::End,
            ModalInputEvent::Delete => E::Delete,
            ModalInputEvent::Line(key) => E::Line(key),
            ModalInputEvent::Fault => E::Fault,
            ModalInputEvent::Completeness(complete) => E::Completeness(complete),
            ModalInputEvent::Submit => {
                port.surface_action(SurfaceAction::Worktree(WorktreeAction::Submit(token)));
                return;
            }
            ModalInputEvent::Cancel => {
                port.surface_action(SurfaceAction::Worktree(WorktreeAction::Dismiss(token)));
                return;
            }
            ModalInputEvent::InputFailure
            | ModalInputEvent::Mouse(_)
            | ModalInputEvent::Focus(_)
            | ModalInputEvent::HostReply(_) => return,
        };
        if port.surface_input(SurfaceInput::Worktree { token, edit })
            && let Some(CaptureContext::Worktree(context)) =
                port.native_capture_context(WorkbenchCaptureOwner::Worktree)
        {
            self.epoch = Some(ModalEpoch::Worktree(context.token));
        }
    }

    fn apply_setup_event(
        &mut self,
        port: &mut (impl TerminalInputPort + ?Sized),
        token: SetupToken,
        event: ModalInputEvent,
    ) {
        use crate::workbench_client::{CaptureContext, CaptureReadiness};

        let Some(CaptureContext::Setup(context)) =
            port.native_capture_context(WorkbenchCaptureOwner::Setup)
        else {
            return;
        };
        if context.token != token {
            return;
        }
        if context.readiness == CaptureReadiness::Recovery {
            if event == ModalInputEvent::Cancel {
                port.surface_action(SurfaceAction::Setup(SetupAction::Cancel(token)));
            }
            return;
        }
        let edit = match event {
            ModalInputEvent::NextField => SetupEdit::NextField,
            ModalInputEvent::PreviousChoice => SetupEdit::SelectPreviousProject,
            ModalInputEvent::NextChoice => SetupEdit::SelectNextProject,
            ModalInputEvent::Append(ref text)
                if matches!(
                    context.stage,
                    SetupStage::QuickProject | SetupStage::ChooseWorkspaceProject
                ) =>
            {
                match text.as_str() {
                    "[" => SetupEdit::SelectPreviousProject,
                    "]" => SetupEdit::SelectNextProject,
                    _ => return,
                }
            }
            ModalInputEvent::Append(ref text) => SetupEdit::Append(text),
            ModalInputEvent::Paste(ref text) => SetupEdit::Paste(text),
            ModalInputEvent::Backspace => SetupEdit::Backspace,
            ModalInputEvent::Left => SetupEdit::Left,
            ModalInputEvent::Right => SetupEdit::Right,
            ModalInputEvent::Home => SetupEdit::Home,
            ModalInputEvent::End => SetupEdit::End,
            ModalInputEvent::Delete => SetupEdit::Delete,
            ModalInputEvent::Line(key) => SetupEdit::Line(key),
            ModalInputEvent::Fault => SetupEdit::Fault,
            ModalInputEvent::Completeness(complete) => SetupEdit::Completeness(complete),
            ModalInputEvent::Submit => {
                let _disposition =
                    port.surface_action(SurfaceAction::Setup(SetupAction::Submit(token)));
                return;
            }
            ModalInputEvent::Cancel => {
                port.surface_action(SurfaceAction::Setup(SetupAction::Cancel(token)));
                return;
            }
            ModalInputEvent::InputFailure
            | ModalInputEvent::Mouse(_)
            | ModalInputEvent::Focus(_)
            | ModalInputEvent::HostReply(_) => return,
        };
        let selecting = matches!(
            edit,
            SetupEdit::SelectPreviousProject | SetupEdit::SelectNextProject | SetupEdit::NextField
        );
        if port.surface_input(SurfaceInput::Setup { token, edit })
            && selecting
            && let Some(CaptureContext::Setup(context)) =
                port.native_capture_context(WorkbenchCaptureOwner::Setup)
            && matches!(
                context.stage,
                SetupStage::ChooseWorkspaceProject
                    | SetupStage::QuickProject
                    | SetupStage::QuickName
                    | SetupStage::QuickDirectory
            )
        {
            // Selection changes draw authority, while this decoder still owns the same picker.
            self.epoch = Some(ModalEpoch::Setup(context.token));
            self.decoder
                .set_capacity(if context.stage == SetupStage::QuickDirectory {
                    ModalCapacity::Bytes4096
                } else {
                    ModalCapacity::Scalars512
                });
        }
    }

    fn capture_modal_release(&mut self, event: super::MouseEvent) -> bool {
        if event.action == super::MouseAction::Release
            && self.modal_release.is_some_and(|button| {
                event.button == button || event.button == super::MouseButton::None
            })
        {
            match self.pointer.modal_release_ownership() {
                ReleaseOwnership::Active | ReleaseOwnership::Cleanup => {
                    // Active controls run first. Cleanup tombstones are
                    // settled by the earlier cancelled-owner passes without
                    // consulting the current surface.
                    return false;
                }
                ReleaseOwnership::None => {}
            }
            self.modal_release = None;
            self.pointer_packet = true;
            self.keys.capture_pending();
            return true;
        }
        if event.action == super::MouseAction::Press
            && !matches!(
                event.button,
                super::MouseButton::WheelUp
                    | super::MouseButton::WheelDown
                    | super::MouseButton::WheelLeft
                    | super::MouseButton::WheelRight
            )
        {
            self.modal_release = None;
        }
        false
    }

    fn capture_confirmation_mouse<Port: TerminalInputPort + ?Sized>(
        &mut self,
        port: &mut Port,
        event: super::MouseEvent,
    ) {
        let decision = self.pointer.confirmation_event(port, event);
        self.capture_modal_release(event);
        if decision == Some(false)
            && event.action == super::MouseAction::Press
            && matches!(
                event.button,
                super::MouseButton::Left | super::MouseButton::Right | super::MouseButton::Middle
            )
        {
            self.modal_release = Some(event.button);
        }
        self.confirm_packet = true;
        self.keys.capture_pending();
    }
}
