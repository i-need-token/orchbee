use std::time::Instant;

use crate::client_view::{
    ClientAction, ClientKey, ClientView, KeyCode, KeyKind, Modifiers, PrefixInput,
    SessionActionDisposition, ShellCommand,
};
use crate::terminal_session::NativeSetupStage;
use crate::tui_composition::{
    CaptureOwner, ClientCaptureClaims, NativeCaptureOwner, client_chrome_overlay_allowed,
    project_capture,
};
use crate::tui_pointer::{
    CancelledModalEvent, FrameHitMap, InteractionTarget, MotionObservability, PointerOutcome,
    PointerState, PrimaryContract, ReleaseOwnership,
};

use super::{
    DispatchOutcome, HostReply, ModeRequest, NativeRenameDraftToken, NativeRenameEdit,
    NativeSetupEdit, NativeSetupToken, PointerCaptureRequest, TerminalInputPort,
    decoder::{DecodedItem, DecodedKind, LegacyDecoder},
    forward_or_report,
    modal::{ModalCapacity, ModalDecoder, ModalInputEvent},
};

#[cfg(windows)]
mod navigator;

#[derive(Default)]
pub(crate) struct InputDispatcher {
    #[cfg(windows)]
    navigator: navigator::Capture,
    graphics: crate::terminal_graphics::Probe,
    pointer_admission: crate::terminal_pointer_admission::Probe,
    pointer_capture: PointerCaptureRequest,
    /// A failed level switch may have emitted a prefix. Runtime must exit and
    /// lifecycle cleanup must settle it; input never retries that transition.
    pointer_transition_failed: bool,
    #[cfg(windows)]
    graphics_replies: Vec<crate::terminal_graphics::Reply>,
    #[cfg(windows)]
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
    #[cfg(windows)]
    machine_packet: bool,
    #[cfg(windows)]
    sidebar_packet: bool,
    #[cfg(windows)]
    sidebar_owned: bool,
    #[cfg(windows)]
    sidebar_cancel_capture: bool,
    #[cfg(windows)]
    popup_packet: bool,
    #[cfg(windows)]
    copy_packet: bool,
    #[cfg(windows)]
    copy_owned: bool,
    settings_packet: bool,
    settings_cancel_capture: bool,
    #[cfg(windows)]
    menu_packet: bool,
    /// A cancelled menu was still the painted input surface. Capture reads until a fresh frame.
    #[cfg(windows)]
    menu_cancel_capture: bool,
    #[cfg(windows)]
    menu_release: Option<super::MouseButton>,
    #[cfg(windows)]
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
    Rename(NativeRenameDraftToken),
    Setup(NativeSetupToken),
    #[cfg(windows)]
    Worktree(crate::terminal_session::WorktreeToken),
}

struct ProjectedCaptureItem {
    owner: CaptureOwner,
    item: DecodedItem,
    now: Instant,
}

#[cfg(test)]
mod tests;

/// Adapts one decoded item to the legacy confirmation-port vocabulary. Only
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
    fn projected_capture(
        _view: &ClientView,
        port: &(impl TerminalInputPort + ?Sized),
    ) -> Option<CaptureOwner> {
        let client = ClientCaptureClaims::default();
        #[cfg(windows)]
        let client = client
            .with_machine(_view.machines.open)
            .with_resize(_view.resize_mode());
        #[cfg(windows)]
        let native = port
            .native_capture_claims()
            .with_copy(port.copy_mode_active());
        #[cfg(not(windows))]
        let native = port.native_capture_claims();
        project_capture(client, native)
    }

    fn modal_remaining(port: &mut (impl TerminalInputPort + ?Sized), epoch: ModalEpoch) -> usize {
        use crate::terminal_session::NativeCaptureContext;

        match epoch {
            ModalEpoch::Rename(token) => {
                match port.native_capture_context(NativeCaptureOwner::Rename) {
                    Some(crate::terminal_session::NativeCaptureContext::Rename(context))
                        if context.token == token =>
                    {
                        context.remaining_scalars
                    }
                    _ => 0,
                }
            }
            ModalEpoch::Setup(token) => {
                match port.native_capture_context(NativeCaptureOwner::Setup) {
                    Some(NativeCaptureContext::Setup(context)) if context.token == token => {
                        if matches!(
                            context.stage,
                            NativeSetupStage::DirectoryPath | NativeSetupStage::QuickDirectory
                        ) {
                            context.remaining_bytes
                        } else {
                            context.remaining_scalars
                        }
                    }
                    _ => 0,
                }
            }
            #[cfg(windows)]
            ModalEpoch::Worktree(token) => {
                match port.native_capture_context(NativeCaptureOwner::Worktree) {
                    Some(NativeCaptureContext::Worktree(context)) if context.token == token => {
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
    #[cfg(windows)]
    fn settle_action_menu_closed(&mut self, port: &mut (impl TerminalInputPort + ?Sized)) {
        self.menu_packet = true;
        self.menu_cancel_capture = true;
        self.keys.capture_pending();
        self.pointer.invalidate_map_with_port(port);
    }
    #[cfg(windows)]
    pub(crate) fn complete_action_menu<Port: TerminalInputPort + ?Sized, Error>(
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
        let transition = view.decide_action(action);
        self.apply_transition(view, port, now, transition, None, after_layout, mode)
            .map(Some)
    }
    fn host_reply(&mut self, reply: HostReply, now: Instant) {
        match reply {
            HostReply::Graphics(reply) => {
                self.graphics.reply(reply, now);
                #[cfg(windows)]
                if matches!(reply, crate::terminal_graphics::Reply::Kitty { .. }) {
                    if self.graphics_replies.len() < 128 {
                        self.graphics_replies.push(reply);
                    } else {
                        self.graphics_reply_overflow = true;
                    }
                }
            }
            HostReply::Pointer(reply) => {
                self.pointer_admission.observe(reply, now);
            }
        }
    }
    #[cfg(windows)]
    pub(crate) fn take_graphics_replies(&mut self) -> (Vec<crate::terminal_graphics::Reply>, bool) {
        (
            std::mem::take(&mut self.graphics_replies),
            std::mem::take(&mut self.graphics_reply_overflow),
        )
    }
    pub(crate) fn activate_graphics_probe(&mut self, id: u32) {
        self.graphics.activate(id);
    }
    pub(crate) fn graphics_facts(&self) -> crate::terminal_graphics::Facts {
        self.graphics.facts()
    }
    pub(crate) fn query_graphics<Error>(
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
    pub(crate) fn begin_pointer_admission<Error>(
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
        #[cfg(windows)]
        let request = if request == PointerCaptureRequest::ButtonOnly
            && Self::projected_capture(view, port)
                == Some(CaptureOwner::Native(NativeCaptureOwner::ActionMenu))
        {
            PointerCaptureRequest::MenuMotion
        } else {
            request
        };
        #[cfg(not(windows))]
        let _ = (view, port);
        if self.pointer_capture == request {
            return Ok(());
        }
        if let Err(error) = mode(ModeRequest::PointerCapture(request)) {
            self.pointer_transition_failed = true;
            return Err(error);
        }
        self.pointer_capture = request;
        self.pointer.set_motion_observability(match request {
            PointerCaptureRequest::AnyMotion => MotionObservability::AnyMotion,
            #[cfg(windows)]
            PointerCaptureRequest::MenuMotion => MotionObservability::AnyMotion,
            PointerCaptureRequest::Release | PointerCaptureRequest::ButtonOnly => {
                MotionObservability::ButtonOnly
            }
        });
        Ok(())
    }
    #[cfg(windows)]
    pub(crate) fn machine_surface_changed(&mut self) {
        self.keys.capture_pending();
        self.pointer.invalidate_map();
    }

    /// The shell settles the armed flag after the terminal mode write
    /// succeeded, so client state never claims a capture the host did not
    /// actually take.
    #[cfg(test)]
    pub(crate) fn set_pointer_capture(
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
    pub(crate) fn settle_pointer_capture(
        &mut self,
        view: &mut ClientView,
        port: &mut (impl TerminalInputPort + ?Sized),
        request: PointerCaptureRequest,
    ) {
        let enabled = request != PointerCaptureRequest::Release;
        #[cfg(windows)]
        if !enabled {
            let closing = port
                .native_capture_claims()
                .contains(NativeCaptureOwner::ActionMenu);
            port.cancel_action_menu();
            self.menu_release = None;
            if closing {
                self.settle_action_menu_closed(port);
            }
        }
        self.pointer.set_motion_observability(match request {
            PointerCaptureRequest::AnyMotion => MotionObservability::AnyMotion,
            #[cfg(windows)]
            PointerCaptureRequest::MenuMotion => MotionObservability::AnyMotion,
            PointerCaptureRequest::Release | PointerCaptureRequest::ButtonOnly => {
                MotionObservability::ButtonOnly
            }
        });
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

    pub(crate) fn pointer_projection(&self) -> crate::tui_pointer::PointerProjection {
        self.pointer.projection()
    }

    pub(crate) fn take_pointer_projection_redraw(&mut self) -> bool {
        self.pointer.take_projection_redraw()
    }

    pub(crate) const fn dispatch_requested_redraw(&self) -> bool {
        self.dispatch_redraw
    }

    /// The renderer's finished frame map becomes the resolution authority.
    #[cfg(test)]
    pub(crate) fn publish_hit_map(&mut self, map: FrameHitMap) {
        let _ = self.publish_hit_map_with_help(map);
    }

    /// Publishes the map and returns its exact Help draw proof to the
    /// client-owned surface authority. Other callers do not need to observe it.
    pub(crate) fn publish_hit_map_with_help(
        &mut self,
        map: FrameHitMap,
    ) -> Option<crate::client_view::help::HelpDrawSource> {
        let help = map.help_draw_source().copied();
        self.settings_cancel_capture = false;
        #[cfg(windows)]
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
    pub(crate) fn invalidate_hit_map(&mut self, port: &mut (impl TerminalInputPort + ?Sized)) {
        self.pointer.invalidate_map_with_port(port);
    }

    /// Ends an admitted pointer selection before the shell exits while the
    /// surface that owns it is still available.
    pub(crate) fn cancel_pointer(&mut self, port: &mut (impl TerminalInputPort + ?Sized)) {
        self.pointer.cancel_selection(port);
        #[cfg(windows)]
        {
            self.pointer.cancel_divider();
            self.pointer.cancel_sidebar_width();
            self.pointer.cancel_tab_drag();
            self.pointer.cancel_sidebar_reorder();
        }
    }

    #[cfg(windows)]
    pub(crate) fn sidebar_reorder_preview(
        &self,
    ) -> Option<crate::tui_pointer::SidebarReorderPreview> {
        self.pointer.sidebar_reorder_preview()
    }

    /// Immutable preview supplied by the gesture owner for one paint pass.
    #[cfg(windows)]
    pub(crate) fn divider_preview(&self) -> Option<&crate::tui_layout::DividerDrag> {
        self.pointer.divider_preview()
    }

    #[cfg(windows)]
    pub(crate) fn tab_drag_preview(&self) -> Option<crate::tui_pointer::TabDragPreview> {
        self.pointer.tab_drag_preview()
    }

    #[cfg(windows)]
    pub(crate) fn sidebar_width_preview(&self) -> Option<&crate::tui_layout::SidebarWidthDrag> {
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
        #[cfg(windows)]
        let popup = if current == Some(CaptureOwner::Native(NativeCaptureOwner::Popup)) {
            match port.native_capture_context(NativeCaptureOwner::Popup) {
                Some(crate::terminal_session::NativeCaptureContext::Popup(context)) => {
                    Some(context.id().clone())
                }
                _ => None,
            }
        } else {
            None
        };
        #[cfg(windows)]
        let popup_changed = popup != self.popup_owned;
        #[cfg(windows)]
        {
            self.popup_owned = popup;
        }
        if current == self.capture_owner && {
            #[cfg(windows)]
            {
                !popup_changed
            }
            #[cfg(not(windows))]
            {
                true
            }
        } {
            return false;
        }

        let previous = std::mem::replace(&mut self.capture_owner, current);
        #[cfg(windows)]
        if current.is_some() && self.pointer.retire_covered_chrome_gesture() {
            self.pointer_packet = true;
        }
        if previous == Some(CaptureOwner::Native(NativeCaptureOwner::Settings))
            && current != previous
        {
            self.settle_settings_closed(port);
        }
        #[cfg(windows)]
        if previous == Some(CaptureOwner::Native(NativeCaptureOwner::ActionMenu))
            && current != previous
        {
            self.settle_action_menu_closed(port);
        }
        #[cfg(windows)]
        if previous == Some(CaptureOwner::Native(NativeCaptureOwner::Popup))
            && (current != previous || popup_changed)
        {
            self.popup_packet = true;
        }
        #[cfg(windows)]
        if previous == Some(CaptureOwner::Machine) && current != previous {
            self.machine_packet = true;
        }
        if matches!(
            previous,
            Some(CaptureOwner::Native(
                NativeCaptureOwner::PanelClose
                    | NativeCaptureOwner::TabClose
                    | NativeCaptureOwner::WorkspaceClose
            ))
        ) && current != previous
        {
            self.confirm_packet = true;
        }
        if previous == Some(CaptureOwner::Native(NativeCaptureOwner::AgentBoard))
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

    pub(crate) fn synchronize<Port: TerminalInputPort + ?Sized, Error>(
        &mut self,
        view: &mut ClientView,
        port: &mut Port,
        now: Instant,
        mode: &mut impl FnMut(ModeRequest) -> Result<(), Error>,
    ) -> Result<bool, Error> {
        self.synchronize_input(view, port, now, mode, true)
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
        // Compare the settled owner from the preceding boundary with the
        // post-reconcile projection below. An asynchronously installed owner
        // is already visible to the pure projector here, but it has not yet
        // acquired the host paste lease.
        let owned_input = self.decoder.owns_input() || self.capture_owner.is_some();
        #[cfg(windows)]
        let owned_input =
            owned_input || view.navigator_active() || view.machines.sidebar_navigation_active();
        // An idle read is a packet boundary too: a `Released` guard left
        // over from an earlier packet ends here so the lease is returned
        // even when no further bytes ever arrive. A release produced by
        // this call's own reconcile still guards the current packet tail.
        self.decoder.end_released(now);
        let mut changed = self.reconcile(view, port, now);
        if idle {
            let events = self.decoder.tick(now);
            changed |= !events.is_empty();
            self.apply_events(view, port, events, now);
            self.synchronize_pointer_admission(view, port, mode)?;
        }
        self.confirm_packet = false;
        self.pointer_packet = false;
        self.settings_packet = self.settings_cancel_capture;
        #[cfg(windows)]
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
        #[cfg(windows)]
        let capturing =
            capturing || view.navigator_active() || view.machines.sidebar_navigation_active();
        if !capturing {
            mode(ModeRequest::BracketedPaste(false))?;
        } else if !owned_input {
            // A later wizard step can arrive after the previous decoder released its lease.
            mode(ModeRequest::BracketedPaste(true))?;
        }
        self.synchronize_pointer_admission(view, port, mode)?;
        Ok(changed)
    }

    pub(crate) fn dispatch<Port: TerminalInputPort + ?Sized, Error>(
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
        #[cfg(windows)]
        if let Some(outcome) =
            self.complete_action_menu(view, port, now, &mut after_layout, mode)?
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
            let owned = self.confirm_packet || self.settings_packet || self.pointer_packet;
            #[cfg(windows)]
            let owned = owned
                || self.navigator.packet
                || self.machine_packet
                || self.sidebar_packet
                || self.popup_packet
                || self.copy_packet
                || self.menu_packet;
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
        #[cfg(windows)]
        if let Some(outcome) =
            self.complete_action_menu(view, port, now, &mut after_layout, mode)?
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
        use crate::terminal_session::NativeCaptureContext;

        let ProjectedCaptureItem { owner, item, now } = projected;

        #[cfg(windows)]
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

        #[cfg(windows)]
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
                        port.capture_copy_mode_key(key);
                    }
                    DecodedKind::Paste { text } => {
                        port.capture_copy_mode_paste(Some(text));
                    }
                    DecodedKind::RejectedPaste => {
                        port.capture_copy_mode_paste(None);
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

        #[cfg(windows)]
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
            #[cfg(windows)]
            CaptureOwner::Machine => unreachable!("the machine capture returned above"),
            #[cfg(windows)]
            CaptureOwner::Copy | CaptureOwner::Resize => {
                unreachable!("the client capture returned above")
            }
            CaptureOwner::Native(owner) => owner,
        };
        let context = port.native_capture_context(native_owner);
        match (native_owner, context) {
            (NativeCaptureOwner::Settings, Some(NativeCaptureContext::Settings(expected))) => {
                match item.kind {
                    DecodedKind::Paste { text } => {
                        let _consumed = port.capture_settings_paste_for(expected, Some(&text));
                        self.pointer.invalidate_map_with_port(port);
                    }
                    DecodedKind::RejectedPaste => {
                        let _consumed = port.capture_settings_paste_for(expected, None);
                        self.pointer.invalidate_map_with_port(port);
                    }
                    DecodedKind::Key(event) => {
                        let _consumed = port.capture_settings_key_for(expected, event);
                        self.pointer.invalidate_map_with_port(port);
                        if crate::client_view::Binding::new(KeyCode::Enter, Modifiers::NONE)
                            .matches(event)
                            || crate::client_view::Binding::new(KeyCode::Esc, Modifiers::NONE)
                                .matches(event)
                        {
                            self.settings_packet = true;
                            self.keys.capture_pending();
                        }
                    }
                    DecodedKind::Mouse(event) => match self.pointer.settings_event(event) {
                        PointerOutcome::IdleMotion { changed } => {
                            self.item_idle_motion = Some(changed);
                        }
                        PointerOutcome::Interaction { target, contract } => {
                            if contract == PrimaryContract::ActivateOnRelease {
                                match target {
                                    InteractionTarget::SettingsRow { token, row } => {
                                        port.click_settings(token, row);
                                    }
                                    InteractionTarget::SettingsControl { token, control } => {
                                        port.click_settings_control(token, control);
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
                    != Some(CaptureOwner::Native(NativeCaptureOwner::Settings))
                {
                    self.settle_settings_closed(port);
                    if Self::projected_capture(view, port).is_none() {
                        mode(ModeRequest::BracketedPaste(false))?;
                    }
                }
            }
            #[cfg(windows)]
            (NativeCaptureOwner::ActionMenu, Some(NativeCaptureContext::ActionMenu(expected))) => {
                match item.kind {
                    DecodedKind::Key(event) => {
                        let _consumed = port.capture_action_menu_key_for(expected, event);
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
                        let outcome = self.pointer.action_menu_event(port, event);
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
                                            port.activate_action_menu_row(&source, token, row)
                                        }
                                        InteractionTarget::ActionMenuControl {
                                            source,
                                            token,
                                            control,
                                        } => port
                                            .activate_action_menu_control(&source, token, control),
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
                    != Some(CaptureOwner::Native(NativeCaptureOwner::ActionMenu))
                {
                    self.settle_action_menu_closed(port);
                    if Self::projected_capture(view, port).is_none()
                        && !view.machines.sidebar_navigation_active()
                    {
                        mode(ModeRequest::BracketedPaste(false))?;
                    }
                }
            }
            #[cfg(windows)]
            (NativeCaptureOwner::Popup, Some(NativeCaptureContext::Popup(expected))) => {
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
                                let _activated = port.activate_drawn_popup_close(&target);
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
                    port.popup_input(expected.id(), &item);
                }
                if Self::projected_capture(view, port)
                    != Some(CaptureOwner::Native(NativeCaptureOwner::Popup))
                {
                    self.popup_packet = true;
                    self.keys.capture_pending();
                    self.pointer.invalidate_map_with_port(port);
                }
            }
            #[cfg(windows)]
            (NativeCaptureOwner::Worktree, Some(NativeCaptureContext::Worktree(_)))
            | (NativeCaptureOwner::Setup, Some(NativeCaptureContext::Setup(_)))
            | (NativeCaptureOwner::Rename, Some(NativeCaptureContext::Rename(_))) => {
                // The modal decoder owns all interactive and recovery input for
                // these surfaces. A complete legacy item here means its exact
                // context disappeared between byte reconciliation and dispatch;
                // consume it without selecting a lower owner.
            }
            #[cfg(not(windows))]
            (NativeCaptureOwner::Setup, Some(NativeCaptureContext::Setup(_)))
            | (NativeCaptureOwner::Rename, Some(NativeCaptureContext::Rename(_))) => {}
            (NativeCaptureOwner::PanelClose, Some(NativeCaptureContext::PanelClose(expected))) => {
                if let DecodedKind::Mouse(event) = &item.kind {
                    self.capture_confirmation_mouse(port, *event);
                } else {
                    let _consumed =
                        port.capture_panel_confirmation_key_for(expected.token, client_key(&item));
                    self.confirm_packet = true;
                    self.keys.capture_pending();
                }
            }
            (NativeCaptureOwner::TabClose, Some(NativeCaptureContext::TabClose(expected))) => {
                if let DecodedKind::Mouse(event) = &item.kind {
                    self.capture_confirmation_mouse(port, *event);
                } else {
                    let _consumed =
                        port.capture_tab_confirmation_key_for(expected.token, client_key(&item));
                    self.confirm_packet = true;
                    self.keys.capture_pending();
                }
            }
            (
                NativeCaptureOwner::WorkspaceClose,
                Some(NativeCaptureContext::WorkspaceClose(expected)),
            ) => {
                if let DecodedKind::Mouse(event) = &item.kind {
                    self.capture_confirmation_mouse(port, *event);
                } else {
                    let _consumed = port
                        .capture_workspace_confirmation_key_for(expected.token, client_key(&item));
                    self.confirm_packet = true;
                    self.keys.capture_pending();
                }
            }
            (NativeCaptureOwner::AgentBoard, Some(NativeCaptureContext::AgentBoard(expected))) => {
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
                                    let _accepted = port.agent_board_select(&target);
                                }
                                (
                                    InteractionTarget::AgentBoardControl(target),
                                    PrimaryContract::ActivateOnRelease,
                                ) => {
                                    let _accepted = port.click_agent_board_control(&target);
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
                    let _consumed = port.capture_agent_board_key_for(expected.episode(), event);
                }
                if Self::projected_capture(view, port)
                    != Some(CaptureOwner::Native(NativeCaptureOwner::AgentBoard))
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

    /// Routes one decoded item through the ownership order: an open
    /// confirmation packet consumes the whole rest of the chunk, an armed
    /// prefix resolves the item against the binding registry, and anything
    /// else joins `self.pending` so its source bytes forward in order.
    /// the configured prefix arms its layer; configured direct bindings run
    /// after capture and history ownership.
    fn handle_item<Port: TerminalInputPort + ?Sized, Error>(
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
            self.host_reply(reply, now);
            self.synchronize_pointer_admission(view, port, mode)?;
            return Ok(DispatchOutcome::Continue);
        }
        if let DecodedKind::Focus(focused) = item.kind {
            #[cfg(windows)]
            // Preserve the active-drag edge before focus teardown installs its
            // cleanup tombstone; that edge owns the rest of this host packet.
            let tab_drag_cancelled = !focused
                && (self.pointer.cancel_tab_drag() | self.pointer.cancel_sidebar_reorder());
            self.pointer.set_host_focus(focused);
            if !focused {
                self.pointer.cancel_list_scroll();
                self.pointer.cancel_modal_press();
                self.pointer.cancel_scrollbar();
            }
            #[cfg(windows)]
            if !focused {
                self.menu_release = None;
                self.navigator.release = None;
                if tab_drag_cancelled {
                    self.menu_packet = true;
                    self.keys.capture_pending();
                }
            }
            self.flush_pending(view, port);
            port.observe_focus(focused);
            self.settle_capture_edge(view, port);
            return Ok(DispatchOutcome::Continue);
        }
        if let DecodedKind::Mouse(event) = item.kind {
            match self.pointer.cancelled_modal_event(event) {
                CancelledModalEvent::Ended => {
                    // The painted owner and the packet-level button debt are
                    // one gesture. Settle both before a newer surface can
                    // reinterpret the same physical release.
                    self.modal_release = None;
                    #[cfg(windows)]
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
                    #[cfg(windows)]
                    {
                        self.menu_release = None;
                    }
                }
                CancelledModalEvent::Ignored => {}
            }
            match self.pointer.cancelled_ui_event(event) {
                PointerOutcome::InteractionEnded => {
                    self.modal_release = None;
                    #[cfg(windows)]
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
                #[cfg(windows)]
                PointerOutcome::ChromeGestureEnded => {
                    self.pointer_packet = true;
                    self.keys.capture_pending();
                    return Ok(DispatchOutcome::Continue);
                }
                #[cfg(windows)]
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
            self.pointer.flush_list_scroll(view, port);
            self.pointer.cancel_list_scroll();
            self.pointer.cancel_modal_press();
            self.pointer.cancel_scrollbar();
        }
        #[cfg(windows)]
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
        #[cfg(windows)]
        if let DecodedKind::Mouse(event) = item.kind
            && event.action == super::MouseAction::Release
            && Self::projected_capture(view, port)
                != Some(CaptureOwner::Native(NativeCaptureOwner::ActionMenu))
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
                    NativeCaptureOwner::PanelClose
                    | NativeCaptureOwner::TabClose
                    | NativeCaptureOwner::WorkspaceClose,
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
        // Help is noncapturing, but an accepted generic Help press owns its
        // exact release even when intervening packet bytes were fenced. Route
        // that release before `item.owned` can consume it as ordinary tail.
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
        #[cfg(windows)]
        if self.capture_navigator_release(&item) {
            return Ok(DispatchOutcome::Continue);
        }
        if item.owned {
            // The item completes a frame that began inside a captured
            // packet: consumed by that capture, never forwarded or decided.
            self.flush_pending(view, port);
            return Ok(DispatchOutcome::Continue);
        }
        let exclusive_packet = self.confirm_packet || self.settings_packet;
        #[cfg(windows)]
        let exclusive_packet = exclusive_packet
            || self.machine_packet
            || self.popup_packet
            || self.copy_packet
            || self.menu_packet;
        if exclusive_packet {
            self.flush_pending(view, port);
            return Ok(DispatchOutcome::Continue);
        }
        if let Some(owner) = Self::projected_capture(view, port) {
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
        #[cfg(windows)]
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
        #[cfg(windows)]
        if match &item.kind {
            DecodedKind::Key(event) => event.kind != KeyKind::Release,
            DecodedKind::Paste { .. } | DecodedKind::RejectedPaste => true,
            DecodedKind::Mouse(event) => {
                event.action != super::mouse::MouseAction::Release
                    && event.button != super::mouse::MouseButton::None
            }
            _ => false,
        } {
            port.observe_input_intent();
        }
        if let DecodedKind::Mouse(event) = &item.kind
            && self.capture_help_mouse(view, port, *event)
        {
            self.flush_pending(view, port);
            return Ok(DispatchOutcome::Continue);
        }
        #[cfg(windows)]
        if self.navigator.packet || view.navigator_active() {
            self.flush_pending(view, port);
            if !self.navigator.packet {
                self.navigator_item(view, port, &item);
                self.reconcile(view, port, now);
                after_layout(view, port)?;
            }
            return Ok(DispatchOutcome::Continue);
        }
        #[cfg(windows)]
        if (self.pointer.tab_drag_captured() || self.pointer.sidebar_reorder_captured())
            && !matches!(item.kind, DecodedKind::Mouse(_))
        {
            self.flush_pending(view, port);
            if matches!(item.kind, DecodedKind::Key(event) if event.kind != KeyKind::Release
                && event.code == crate::client_view::KeyCode::Esc && event.mods == crate::client_view::Modifiers::NONE)
            {
                self.pointer.cancel_tab_drag();
                self.pointer.cancel_sidebar_reorder();
                self.menu_packet = true;
            }
            self.keys.capture_pending();
            return Ok(DispatchOutcome::Continue);
        }
        #[cfg(windows)]
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
        #[cfg(windows)]
        if self.sidebar_packet
            || (view.machines.sidebar_navigation_active()
                && !matches!(item.kind, DecodedKind::Mouse(_)))
        {
            self.flush_pending(view, port);
            if !self.sidebar_packet {
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
                    crate::machine_picker::SidebarIntent::Search => {
                        view.machines.leave_sidebar_navigation();
                        view.dismiss_overlay();
                        view.machines.show_agent_search();
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
                    crate::machine_picker::SidebarIntent::None => {
                        // Focus/window changes invalidate the row geometry until repaint.
                        self.pointer.invalidate_map_with_port(port);
                    }
                    crate::machine_picker::SidebarIntent::Help => {
                        let transition = view.decide_action(ClientAction::OpenShortcutHelp);
                        view.commit(transition, SessionActionDisposition::KeepNavigator);
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
                            port.commit_sidebar_reorder(&source, operation);
                        }
                    }
                    crate::machine_picker::SidebarIntent::OpenMenu(target) => {
                        if let Some(target) = self.pointer.sidebar_menu_target(view, &target) {
                            mode(ModeRequest::BracketedPaste(true))?;
                            if port.open_action_menu(target, None, view.pointer_released()) {
                                self.menu_packet = true;
                                self.keys.capture_pending();
                                self.pointer.invalidate_map_with_port(port);
                            } else {
                                mode(ModeRequest::BracketedPaste(
                                    view.machines.sidebar_navigation_active(),
                                ))?;
                            }
                        }
                    }
                }
            }
            self.reconcile(view, port, now);
            return Ok(DispatchOutcome::Continue);
        }
        if view.prefix_active() {
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
                    // the pane owns it in original order and the prefix
                    // stays armed.
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
                                    let disposition = port.activate_drawn_presentation(&target);
                                    view.commit_pointer(disposition);
                                    self.reconcile(view, port, now);
                                    if disposition == SessionActionDisposition::LayoutChanged {
                                        after_layout(view, port)?;
                                    }
                                }
                                #[cfg(windows)]
                                InteractionTarget::GlobalChrome(action) => {
                                    if Self::projected_capture(view, port).is_none() {
                                        let action = match action {
                                            crate::tui_pointer::GlobalChromeAction::OpenMenu => {
                                                Some(ClientAction::OpenActionMenu)
                                            }
                                            crate::tui_pointer::GlobalChromeAction::RestoreSidebar
                                                if view.machines.available
                                                    && view.sidebar_surface()
                                                        == crate::client_view::SidebarSurface::Hidden =>
                                            {
                                                Some(ClientAction::ToggleBusinessSidebar)
                                            }
                                            crate::tui_pointer::GlobalChromeAction::RestoreSidebar => {
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
                                #[cfg(not(windows))]
                                InteractionTarget::Tab(tab) => {
                                    let probe = port.pointer_probe();
                                    if let Some(index) = probe.tab_index(&tab)
                                        && let Ok(position) =
                                            u8::try_from(probe.tabs[index].position)
                                        && let Some(disposition) = port
                                            .apply_client_action(ClientAction::SelectTab(position))
                                    {
                                        view.commit_pointer(disposition);
                                        if disposition == SessionActionDisposition::LayoutChanged {
                                            after_layout(view, port)?;
                                        }
                                    }
                                }
                                #[cfg(not(windows))]
                                InteractionTarget::Context(location) => {
                                    let probe = port.pointer_probe();
                                    if let Some(index) = probe.context_index(&location)
                                        && let Some(disposition) = port.apply_client_action(
                                            ClientAction::SelectContextAt(index),
                                        )
                                    {
                                        view.commit_pointer(disposition);
                                        if disposition == SessionActionDisposition::LayoutChanged {
                                            after_layout(view, port)?;
                                        }
                                    }
                                }
                                #[cfg(windows)]
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
                                #[cfg(windows)]
                                InteractionTarget::Sidebar(drawn) => {
                                    use crate::machine_picker::SidebarActivation;

                                    let changed = match view.machines.sidebar_activation(&drawn) {
                                        Some(SidebarActivation::ToggleProject(target)) => {
                                            view.machines.toggle_sidebar_project(&target)
                                        }
                                        Some(SidebarActivation::ToggleContext(target)) => {
                                            view.machines.toggle_sidebar_context_agents(&target)
                                        }
                                        Some(SidebarActivation::Activate(selection)) => {
                                            let changed = view
                                                .machines
                                                .activate_sidebar_selection(&selection);
                                            if changed {
                                                view.machines.leave_sidebar_navigation();
                                                view.dismiss_overlay();
                                                self.machine_packet = true;
                                            }
                                            changed
                                        }
                                        None => false,
                                    };
                                    if changed {
                                        self.reconcile(view, port, now);
                                        after_layout(view, port)?;
                                    }
                                }
                                #[cfg(windows)]
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
                                        Some(SidebarFixedActivation::Search) => {
                                            view.machines.leave_sidebar_navigation();
                                            view.dismiss_overlay();
                                            view.machines.show_agent_search();
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
                                        Some(SidebarFixedActivation::Browse) => {
                                            view.machines.show_workspaces();
                                            self.machine_packet = true;
                                            self.reconcile(view, port, now);
                                            after_layout(view, port)?;
                                        }
                                        None => {}
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
                        #[cfg(windows)]
                        if matches!(
                            outcome,
                            PointerOutcome::TabGestureEnded | PointerOutcome::SidebarReorderEnded
                        ) {
                            self.menu_packet = true;
                            self.keys.capture_pending();
                            self.pointer.invalidate_map_with_port(port);
                        }
                        #[cfg(windows)]
                        if outcome == PointerOutcome::ChromeGestureEnded {
                            self.pointer_packet = true;
                            self.keys.capture_pending();
                        }
                        #[cfg(windows)]
                        if let PointerOutcome::MenuRequested {
                            target,
                            anchor,
                            button,
                        } = &outcome
                        {
                            mode(ModeRequest::BracketedPaste(true))?;
                            if port.open_action_menu(
                                target.clone(),
                                Some(*anchor),
                                view.pointer_released(),
                            ) {
                                self.menu_packet = true;
                                self.menu_release = Some(*button);
                                self.keys.capture_pending();
                                self.pointer.invalidate_map_with_port(port);
                            } else {
                                mode(ModeRequest::BracketedPaste(
                                    view.machines.sidebar_navigation_active(),
                                ))?;
                            }
                        }
                        if outcome == PointerOutcome::RegionsChanged {
                            self.pointer.stale_map();
                        }
                        #[cfg(windows)]
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
                if event.kind != KeyKind::Release && port.history_key(&event) =>
            {
                self.flush_pending(view, port);
            }
            DecodedKind::Key(event) if view.bindings().find(event, false).is_some() => {
                self.flush_pending(view, port);
                return self.decide(view, port, now, item, after_layout, mode);
            }
            _ => self.pending.extend_from_slice(&item.raw),
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
        transition: crate::client_view::ClientTransition,
        item: Option<&DecodedItem>,
        after_layout: &mut impl FnMut(&mut ClientView, &mut Port) -> Result<bool, Error>,
        mode: &mut impl FnMut(ModeRequest) -> Result<(), Error>,
    ) -> Result<DispatchOutcome, Error> {
        let action = transition.action();
        #[cfg(windows)]
        if action != Some(ClientAction::OpenActionMenu) && !transition.prefix_active() {
            // A prefix has not chosen an action yet. Preserve the active drag
            // until the decision, so menu admission can observe its owner.
            self.pointer.cancel_divider();
            self.pointer.cancel_sidebar_width();
        }
        #[cfg(windows)]
        let requests_copy = action == Some(ClientAction::EnterCopyMode);
        #[cfg(not(windows))]
        let requests_copy = false;
        #[cfg(windows)]
        let requests_navigator = action == Some(ClientAction::OpenWorkbenchNavigator);
        #[cfg(not(windows))]
        let requests_navigator = false;
        #[cfg(windows)]
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
                    | ClientAction::OpenSettings
                    | ClientAction::ToggleAgentBoard
                    | ClientAction::RequestWorkspaceCreate
                    | ClientAction::RequestQuickWorkspace
                    | ClientAction::RequestProjectQuickWorkspace(_)
                    | ClientAction::RequestWorkspaceDirectory
                    | ClientAction::RequestWorkspaceRename
                    | ClientAction::RequestProjectRename
                    | ClientAction::RequestTargetedProjectRename(_)
                    | ClientAction::RequestWorkspaceMove
                    | ClientAction::RequestProjectDissolve
                    | ClientAction::RequestWorkspaceOrganization(_)
                    | ClientAction::RequestProjectCloseAll(_)
                    | ClientAction::RequestWorkspaceCloseAt(_)
            )
        );
        #[cfg(windows)]
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
            #[cfg(windows)]
            if action == ClientAction::FocusBusinessSidebar {
                return Some(SessionActionDisposition::KeepNavigator);
            }
            #[cfg(windows)]
            if action == ClientAction::OpenActionMenu {
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
                return Some(SessionActionDisposition::KeepNavigator);
            }
            port.apply_client_action(action)
        });
        view.commit(
            transition,
            applied.unwrap_or(SessionActionDisposition::KeepNavigator),
        );
        #[cfg(windows)]
        if action == Some(ClientAction::FocusBusinessSidebar) {
            self.sidebar_packet = true;
            self.keys.capture_pending();
            self.cancel_pointer(port);
            self.pointer.invalidate_map_with_port(port);
        }
        #[cfg(windows)]
        if view.machines.available {
            match action {
                Some(ClientAction::OpenMachines) => view.machines.show(),
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
        #[cfg(windows)]
        let capture_admitted = capture_admitted
            || view.navigator_active()
            || view.machines.sidebar_navigation_active();
        if (requests_modal || requests_copy || requests_navigator || item.is_none())
            && !capture_admitted
        {
            mode(ModeRequest::BracketedPaste(false))?;
        }
        if layout_changed || applied == Some(SessionActionDisposition::LayoutChanged) {
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

    #[cfg(windows)]
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
            #[cfg(windows)]
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
        use crate::terminal_session::NativeCaptureContext;

        let capture_changed = self.settle_capture_edge(view, port);
        let projected = Self::projected_capture(view, port);
        let native_owner = match projected {
            Some(CaptureOwner::Native(owner)) => Some(owner),
            #[cfg(windows)]
            Some(CaptureOwner::Machine | CaptureOwner::Copy | CaptureOwner::Resize) => None,
            None => None,
        };
        #[cfg(windows)]
        let higher_exclusive = matches!(
            projected,
            Some(CaptureOwner::Machine | CaptureOwner::Native(_))
        );
        let context = native_owner.and_then(|owner| port.native_capture_context(owner));
        let setup = match context.as_ref() {
            Some(NativeCaptureContext::Setup(context)) => Some(*context),
            _ => None,
        };
        #[cfg(windows)]
        let worktree = match context.as_ref() {
            Some(NativeCaptureContext::Worktree(context)) => Some(*context),
            _ => None,
        };
        let current = match context.as_ref() {
            Some(NativeCaptureContext::Setup(context)) => Some(ModalEpoch::Setup(context.token)),
            Some(NativeCaptureContext::Rename(context)) => Some(ModalEpoch::Rename(context.token)),
            #[cfg(windows)]
            Some(NativeCaptureContext::Worktree(context)) => {
                Some(ModalEpoch::Worktree(context.token))
            }
            _ => None,
        };
        #[cfg(windows)]
        if higher_exclusive {
            view.exit_resize_mode();
            port.cancel_copy_mode();
        }
        #[cfg(windows)]
        let capture_retains_sidebar =
            projected == Some(CaptureOwner::Native(NativeCaptureOwner::ActionMenu));
        #[cfg(windows)]
        let capture_blocks_sidebar = projected.is_some() && !capture_retains_sidebar;
        #[cfg(windows)]
        if capture_blocks_sidebar {
            view.machines.leave_sidebar_navigation();
        }
        #[cfg(windows)]
        let navigator_changed = self.reconcile_navigator(view, port, projected.is_some());
        #[cfg(not(windows))]
        let navigator_changed = false;
        #[cfg(windows)]
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
        #[cfg(not(windows))]
        let sidebar_changed = false;
        let choice_keys = setup.is_some_and(|context| {
            matches!(
                context.stage,
                NativeSetupStage::QuickProject | NativeSetupStage::ChooseWorkspaceProject
            )
        });
        #[cfg(windows)]
        let choice_keys = choice_keys
            || worktree.is_some_and(|context| {
                context.stage == crate::terminal_session::WorktreeStage::Checkout
            });
        self.decoder.choice_keys(choice_keys);
        #[cfg(windows)]
        {
            if higher_exclusive {
                port.cancel_copy_mode();
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
                NativeSetupStage::DirectoryPath | NativeSetupStage::QuickDirectory
            )
        }) {
            ModalCapacity::Bytes4096
        } else {
            ModalCapacity::Scalars512
        };
        #[cfg(windows)]
        let capacity = if matches!(current, Some(ModalEpoch::Worktree(_))) {
            ModalCapacity::Bytes1024
        } else {
            capacity
        };
        if let Some(token) = current
            && self.decoder.start_with_capacity(capacity)
        {
            #[cfg(windows)]
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
                self.host_reply(reply, now);
                continue;
            }
            if let ModalInputEvent::Focus(focused) = event {
                self.pointer.set_host_focus(focused);
                if !focused {
                    self.pointer.cancel_list_scroll();
                    self.pointer.cancel_modal_press();
                    self.pointer.cancel_scrollbar();
                }
                port.observe_focus(focused);
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
                            match port.native_capture_context(NativeCaptureOwner::Setup) {
                                Some(crate::terminal_session::NativeCaptureContext::Setup(
                                    context,
                                )) => Some(ModalEpoch::Setup(context.token)),
                                _ => None,
                            }
                        }
                        #[cfg(windows)]
                        Some(ModalEpoch::Worktree(_)) => {
                            match port.native_capture_context(NativeCaptureOwner::Worktree) {
                                Some(crate::terminal_session::NativeCaptureContext::Worktree(
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
                #[cfg(windows)]
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
                            match port.native_capture_context(NativeCaptureOwner::Setup) {
                                Some(crate::terminal_session::NativeCaptureContext::Setup(
                                    context,
                                )) => Some(ModalEpoch::Setup(context.token)),
                                _ => None,
                            }
                        }
                        #[cfg(windows)]
                        ModalEpoch::Worktree(_) => {
                            match port.native_capture_context(NativeCaptureOwner::Worktree) {
                                Some(crate::terminal_session::NativeCaptureContext::Worktree(
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
                #[cfg(windows)]
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
                ModalInputEvent::Append(ref text) => NativeRenameEdit::Append(text),
                ModalInputEvent::Paste(ref text) => NativeRenameEdit::Paste(text),
                ModalInputEvent::Backspace => NativeRenameEdit::Backspace,
                ModalInputEvent::Clear => NativeRenameEdit::Clear,
                ModalInputEvent::Left => NativeRenameEdit::Left,
                ModalInputEvent::Right => NativeRenameEdit::Right,
                ModalInputEvent::Home => NativeRenameEdit::Home,
                ModalInputEvent::End => NativeRenameEdit::End,
                ModalInputEvent::Delete => NativeRenameEdit::Delete,
                ModalInputEvent::WordBackspace => NativeRenameEdit::WordBackspace,
                ModalInputEvent::Fault => NativeRenameEdit::Fault,
                ModalInputEvent::Completeness(complete) => NativeRenameEdit::Completeness(complete),
                ModalInputEvent::Submit => {
                    let _disposition = port.submit_rename_draft(token);
                    continue;
                }
                ModalInputEvent::Cancel => {
                    port.cancel_rename_draft(token);
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
            let _accepted = port.edit_rename_draft(token, edit);
        }
    }

    #[cfg(windows)]
    fn apply_worktree_event(
        &mut self,
        port: &mut (impl TerminalInputPort + ?Sized),
        token: crate::terminal_session::WorktreeToken,
        event: ModalInputEvent,
    ) {
        use crate::terminal_session::{
            CaptureReadiness, NativeCaptureContext, WorktreeEdit as E, WorktreeStage as S,
        };
        let Some(NativeCaptureContext::Worktree(context)) =
            port.native_capture_context(NativeCaptureOwner::Worktree)
        else {
            return;
        };
        if context.token != token {
            return;
        }
        if context.readiness == CaptureReadiness::Recovery {
            if event == ModalInputEvent::Cancel {
                port.dismiss_worktree(token);
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
            ModalInputEvent::Clear => E::Clear,
            ModalInputEvent::Left => E::Left,
            ModalInputEvent::Right => E::Right,
            ModalInputEvent::Home => E::Home,
            ModalInputEvent::End => E::End,
            ModalInputEvent::Delete => E::Delete,
            ModalInputEvent::WordBackspace => E::WordBackspace,
            ModalInputEvent::Fault => E::Fault,
            ModalInputEvent::Completeness(complete) => E::Completeness(complete),
            ModalInputEvent::Submit => {
                port.submit_worktree(token);
                return;
            }
            ModalInputEvent::Cancel => {
                port.dismiss_worktree(token);
                return;
            }
            ModalInputEvent::InputFailure
            | ModalInputEvent::Mouse(_)
            | ModalInputEvent::Focus(_)
            | ModalInputEvent::HostReply(_) => return,
        };
        if port.edit_worktree(token, edit)
            && let Some(NativeCaptureContext::Worktree(context)) =
                port.native_capture_context(NativeCaptureOwner::Worktree)
        {
            self.epoch = Some(ModalEpoch::Worktree(context.token));
        }
    }

    fn apply_setup_event(
        &mut self,
        port: &mut (impl TerminalInputPort + ?Sized),
        token: NativeSetupToken,
        event: ModalInputEvent,
    ) {
        use crate::terminal_session::{CaptureReadiness, NativeCaptureContext};

        let Some(NativeCaptureContext::Setup(context)) =
            port.native_capture_context(NativeCaptureOwner::Setup)
        else {
            return;
        };
        if context.token != token {
            return;
        }
        if context.readiness == CaptureReadiness::Recovery {
            if event == ModalInputEvent::Cancel {
                port.cancel_setup_draft(token);
            }
            return;
        }
        let edit = match event {
            ModalInputEvent::NextField => NativeSetupEdit::NextField,
            ModalInputEvent::PreviousChoice => NativeSetupEdit::SelectPreviousProject,
            ModalInputEvent::NextChoice => NativeSetupEdit::SelectNextProject,
            ModalInputEvent::Append(ref text)
                if matches!(
                    context.stage,
                    NativeSetupStage::QuickProject | NativeSetupStage::ChooseWorkspaceProject
                ) =>
            {
                match text.as_str() {
                    "[" => NativeSetupEdit::SelectPreviousProject,
                    "]" => NativeSetupEdit::SelectNextProject,
                    _ => return,
                }
            }
            ModalInputEvent::Append(ref text) => NativeSetupEdit::Append(text),
            ModalInputEvent::Paste(ref text) => NativeSetupEdit::Paste(text),
            ModalInputEvent::Backspace => NativeSetupEdit::Backspace,
            ModalInputEvent::Clear => NativeSetupEdit::Clear,
            ModalInputEvent::Left => NativeSetupEdit::Left,
            ModalInputEvent::Right => NativeSetupEdit::Right,
            ModalInputEvent::Home => NativeSetupEdit::Home,
            ModalInputEvent::End => NativeSetupEdit::End,
            ModalInputEvent::Delete => NativeSetupEdit::Delete,
            ModalInputEvent::WordBackspace => NativeSetupEdit::WordBackspace,
            ModalInputEvent::Fault => NativeSetupEdit::Fault,
            ModalInputEvent::Completeness(complete) => NativeSetupEdit::Completeness(complete),
            ModalInputEvent::Submit => {
                let _disposition = port.submit_setup_draft(token);
                return;
            }
            ModalInputEvent::Cancel => {
                port.cancel_setup_draft(token);
                return;
            }
            ModalInputEvent::InputFailure
            | ModalInputEvent::Mouse(_)
            | ModalInputEvent::Focus(_)
            | ModalInputEvent::HostReply(_) => return,
        };
        let selecting = matches!(
            edit,
            NativeSetupEdit::SelectPreviousProject
                | NativeSetupEdit::SelectNextProject
                | NativeSetupEdit::NextField
        );
        if port.edit_setup_draft(token, edit)
            && selecting
            && let Some(NativeCaptureContext::Setup(context)) =
                port.native_capture_context(NativeCaptureOwner::Setup)
            && matches!(
                context.stage,
                NativeSetupStage::ChooseWorkspaceProject
                    | NativeSetupStage::QuickProject
                    | NativeSetupStage::QuickName
                    | NativeSetupStage::QuickDirectory
            )
        {
            // Selection changes draw authority, while this decoder still owns the same picker.
            self.epoch = Some(ModalEpoch::Setup(context.token));
            self.decoder
                .set_capacity(if context.stage == NativeSetupStage::QuickDirectory {
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
