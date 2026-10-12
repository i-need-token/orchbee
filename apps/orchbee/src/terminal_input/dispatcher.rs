use std::time::Instant;

use crate::client_view::{
    ClientAction, ClientKey, ClientView, KeyCode, KeyKind, Modifiers, PrefixInput,
    SessionActionDisposition, ShellCommand,
};
use crate::tui_pointer::{FrameHitMap, PointerOutcome, PointerState};

use super::{
    DispatchOutcome, ModeRequest, NativeRenameDraftToken, NativeRenameEdit, NativeSetupEdit,
    NativeSetupStage, NativeSetupToken, TerminalInputPort,
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
    #[cfg(windows)]
    menu_owned: bool,
    #[cfg(windows)]
    menu_hover: bool,
    settings_owned: bool,
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
    /// The board's input ownership as last observed — an open or close
    /// edge rewrites what the painted regions mean, so the recorded hit
    /// map drops until the next frame publishes.
    board_owned: bool,
    /// Pointer ownership: capture flag, last-drawn hit map, gesture
    /// lifecycle, and the bounded wheel stream.
    pointer: PointerState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ModalEpoch {
    Rename(NativeRenameDraftToken),
    Setup(NativeSetupToken),
    #[cfg(windows)]
    Worktree(crate::terminal_session::WorktreeToken),
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
            if mods == Modifiers::NONE {
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
    fn settle_settings_closed(&mut self, port: &mut (impl TerminalInputPort + ?Sized)) {
        self.settings_owned = false;
        self.settings_packet = true;
        self.settings_cancel_capture = true;
        self.keys.capture_pending();
        self.pointer.invalidate_map_with_port(port);
    }
    #[cfg(windows)]
    fn settle_action_menu_closed(&mut self, port: &mut (impl TerminalInputPort + ?Sized)) {
        self.menu_owned = false;
        self.menu_packet = true;
        self.menu_cancel_capture = true;
        self.keys.capture_pending();
        self.pointer.invalidate_map_with_port(port);
    }
    #[cfg(windows)]
    fn synchronize_menu_hover<Error>(
        &mut self,
        port: &(impl TerminalInputPort + ?Sized),
        mode: &mut impl FnMut(ModeRequest) -> Result<(), Error>,
    ) -> Result<(), Error> {
        let enabled = self.pointer.capture_enabled() && port.action_menu_active();
        if enabled != self.menu_hover {
            mode(ModeRequest::PointerHover(enabled))?;
            self.menu_hover = enabled;
        }
        Ok(())
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
        self.synchronize_menu_hover(port, mode)?;
        let transition = view.decide_action(action);
        self.apply_transition(view, port, now, transition, None, after_layout, mode)
            .map(Some)
    }
    fn host_reply(&mut self, reply: crate::terminal_graphics::Reply, now: Instant) {
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
    #[cfg(windows)]
    pub(crate) fn machine_surface_changed(&mut self) {
        self.keys.capture_pending();
        self.pointer.invalidate_map();
        self.board_owned = false;
    }

    /// The shell settles the armed flag after the terminal mode write
    /// succeeded, so client state never claims a capture the host did not
    /// actually take.
    pub(crate) fn set_pointer_capture(
        &mut self,
        view: &mut ClientView,
        port: &mut (impl TerminalInputPort + ?Sized),
        enabled: bool,
    ) {
        #[cfg(windows)]
        if !enabled {
            self.menu_hover = false;
            let closing = self.menu_owned || port.action_menu_active();
            port.cancel_action_menu();
            self.menu_release = None;
            if closing {
                self.settle_action_menu_closed(port);
            }
        }
        self.pointer.set_capture_with_port(port, enabled);
        view.settle_pointer_released(!enabled);
    }

    /// The renderer's finished frame map becomes the resolution authority.
    pub(crate) fn publish_hit_map(&mut self, map: FrameHitMap) {
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
        }
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

    /// The board's open or close edge changes what the recorded regions
    /// mean the same way a resize does: the surface they describe is no
    /// longer what is on screen. Dropping the map makes pointer events
    /// fall out until the next frame publishes fresh regions — a stale
    /// map could otherwise route a click into a board that is no longer
    /// there, or past one that just appeared.
    fn settle_board_edge<Port: TerminalInputPort + ?Sized>(&mut self, port: &mut Port) {
        #[cfg(windows)]
        {
            let popup = port.popup_identity();
            if popup != self.popup_owned.as_ref() {
                self.popup_owned = popup.cloned();
                self.keys.capture_pending();
                self.pointer.invalidate_map_with_port(port);
            }
        }
        let owned = port.agent_board_input();
        if owned != self.board_owned {
            self.board_owned = owned;
            self.pointer.invalidate_map_with_port(port);
        }
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
        let owned_input = self.decoder.owns_input() || port.settings_active();
        #[cfg(windows)]
        let owned_input = owned_input
            || view.navigator_active()
            || port.copy_mode_active()
            || port.action_menu_active()
            || port.settings_active()
            || view.machines.sidebar_navigation_active();
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
        changed |= self.pointer.tick(view, port, now);
        changed |= self.reconcile(view, port, now);
        let capturing = self.decoder.owns_input() || port.settings_active();
        #[cfg(windows)]
        let capturing = capturing
            || view.navigator_active()
            || port.copy_mode_active()
            || port.action_menu_active()
            || port.settings_active()
            || view.machines.sidebar_navigation_active();
        if !capturing {
            mode(ModeRequest::BracketedPaste(false))?;
        } else if !owned_input {
            // A later wizard step can arrive after the previous decoder released its lease.
            mode(ModeRequest::BracketedPaste(true))?;
        }
        #[cfg(windows)]
        self.synchronize_menu_hover(port, mode)?;
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
        // Ready bytes resolve pending frames before idle timeouts, and a newly
        // available reversal precedes draining the previous list burst.
        self.synchronize_input(view, port, now, mode, input.is_empty())?;
        #[cfg(windows)]
        if self.complete_action_menu(view, port, now, &mut after_layout, mode)?
            == Some(DispatchOutcome::Exit)
        {
            return Ok(DispatchOutcome::Exit);
        }
        // A new read packet begins: the modal's packet-tail guard ends at
        // this boundary, while an in-flight frame or an open ESC
        // disambiguation keeps ownership under its own rules.
        self.decoder.begin_packet(now);
        for byte in input.iter().copied() {
            self.reconcile(view, port, now);
            let remaining = match self.epoch {
                Some(ModalEpoch::Rename(_)) => port
                    .rename_input_context()
                    .map_or(0, |context| context.remaining_scalar_capacity()),
                Some(ModalEpoch::Setup(_)) => port.setup_input_context().map_or(0, |context| {
                    if matches!(
                        context.stage(),
                        NativeSetupStage::DirectoryPath | NativeSetupStage::QuickDirectory
                    ) {
                        context.remaining_bytes()
                    } else {
                        context.remaining_scalars()
                    }
                }),
                #[cfg(windows)]
                Some(ModalEpoch::Worktree(_)) => port
                    .worktree_input_context()
                    .map_or(0, |c| c.remaining_bytes),
                None if port.settings_active() => port.settings_text_remaining(),
                None => 0,
            };
            let (captured, events) = self.decoder.push(byte, now, remaining);
            if captured || !events.is_empty() {
                self.flush_pending(view, port);
                self.apply_events(view, port, events, now);
                self.reconcile(view, port, now);
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
                || self.menu_packet
                || self.settings_packet;
            for item in self.keys.push(byte, now, owned) {
                if let DispatchOutcome::Exit =
                    self.handle_item(view, port, now, item, &mut after_layout, mode)?
                {
                    self.flush_pending(view, port);
                    return Ok(DispatchOutcome::Exit);
                }
            }
        }
        self.flush_pending(view, port);
        // Queued pointer gestures and the wheel stream advance at the same
        // packet boundary the modal leases do.
        let _list_changed = self.pointer.flush_list_scroll(view, port);
        let _changed = self.pointer.tick(view, port, now);
        // The final pointer report may retire an input owner without a
        // following byte. Settle that edge before a frame can be published.
        self.reconcile(view, port, now);
        #[cfg(windows)]
        if self.complete_action_menu(view, port, now, &mut after_layout, mode)?
            == Some(DispatchOutcome::Exit)
        {
            return Ok(DispatchOutcome::Exit);
        }
        #[cfg(windows)]
        self.synchronize_menu_hover(port, mode)?;
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
            return Ok(DispatchOutcome::Continue);
        }
        if let DecodedKind::Focus(focused) = item.kind {
            if !focused {
                self.pointer.cancel_list_scroll();
                self.pointer.cancel_modal_press();
                self.pointer.cancel_scrollbar();
            }
            #[cfg(windows)]
            if !focused {
                self.menu_release = None;
                self.navigator.release = None;
                if self.pointer.cancel_tab_drag() {
                    self.menu_packet = true;
                    self.keys.capture_pending();
                }
            }
            self.flush_pending(view, port);
            port.observe_focus(focused);
            #[cfg(windows)]
            if self.menu_owned && !port.action_menu_active() {
                self.settle_action_menu_closed(port);
            }
            return Ok(DispatchOutcome::Continue);
        }
        #[cfg(windows)]
        let resize_controls = (view.resize_mode() || port.copy_mode_active())
            && view.overlay() != Some(crate::client_view::Overlay::ShortcutHelp);
        #[cfg(not(windows))]
        let resize_controls = false;
        if let DecodedKind::Mouse(event) = item.kind
            && !port.settings_active()
            && !port.agent_board_input()
            && !resize_controls
            && self.capture_modal_release(event)
        {
            return Ok(DispatchOutcome::Continue);
        }
        if matches!(&item.kind, DecodedKind::Key(_) | DecodedKind::Paste { .. }) {
            self.pointer.flush_list_scroll(view, port);
            self.pointer.cancel_list_scroll();
            self.pointer.cancel_modal_press();
            self.pointer.cancel_scrollbar();
        }
        #[cfg(windows)]
        if let DecodedKind::Mouse(event) = item.kind
            && event.action == super::MouseAction::Press
            && !port.action_menu_active()
            && !self.menu_packet
            && !item.owned
            && self.menu_release == Some(event.button)
        {
            // A new press proves that an unreported old release can no longer arrive.
            self.menu_release = None;
        }
        #[cfg(windows)]
        if let DecodedKind::Mouse(event) = item.kind
            && event.action == super::MouseAction::Release
            && !port.action_menu_active()
            && self.menu_release.is_some_and(|button| {
                event.button == button || event.button == super::MouseButton::None
            })
        {
            self.menu_release = None;
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
        if self.settings_packet || port.settings_active() {
            self.flush_pending(view, port);
            if !self.settings_packet {
                match item.kind {
                    DecodedKind::Paste { text } => port.capture_settings_paste(text.as_deref()),
                    DecodedKind::Key(event) => {
                        port.capture_settings_key(event);
                        if crate::client_view::Binding::new(KeyCode::Enter, Modifiers::NONE)
                            .matches(event)
                            || crate::client_view::Binding::new(KeyCode::Esc, Modifiers::NONE)
                                .matches(event)
                        {
                            self.settings_packet = true;
                            self.keys.capture_pending();
                        }
                    }
                    DecodedKind::Mouse(event) if self.pointer.capture_enabled() => {
                        let decision = self.pointer.settings_event(port, event);
                        self.capture_modal_release(event);
                        if event.action == super::MouseAction::Press
                            && matches!(
                                event.button,
                                super::MouseButton::Left | super::MouseButton::Right
                            )
                        {
                            self.modal_release = Some(event.button);
                        }
                        if decision {
                            self.settings_packet = true;
                            self.keys.capture_pending();
                        }
                    }
                    _ => {}
                }
                if !port.settings_active() {
                    self.settle_settings_closed(port);
                    mode(ModeRequest::BracketedPaste(false))?;
                }
            }
            return Ok(DispatchOutcome::Continue);
        }
        #[cfg(windows)]
        if self.menu_packet || port.action_menu_active() {
            self.flush_pending(view, port);
            if !self.menu_packet {
                match item.kind {
                    DecodedKind::Key(event) => {
                        port.capture_action_menu_key(event);
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
                        let decision = self.pointer.action_menu_event(port, event);
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
                        if decision {
                            self.menu_packet = true;
                            self.keys.capture_pending();
                        }
                    }
                    _ => {}
                }
                if !port.action_menu_active() {
                    self.settle_action_menu_closed(port);
                    mode(ModeRequest::BracketedPaste(
                        view.machines.sidebar_navigation_active(),
                    ))?;
                }
            }
            return Ok(DispatchOutcome::Continue);
        }
        #[cfg(windows)]
        if self.popup_packet || port.popup_captured() {
            self.flush_pending(view, port);
            if !self.popup_packet {
                if let DecodedKind::Mouse(event) = item.kind {
                    self.pointer.external_capture_event(event);
                    self.pointer.popup_event(port, event);
                } else {
                    port.popup_input(&item);
                }
                if !port.popup_captured() {
                    self.popup_packet = true;
                    self.pointer.invalidate_map_with_port(port);
                }
            }
            return Ok(DispatchOutcome::Continue);
        }
        #[cfg(windows)]
        if self.machine_packet || view.machines.open {
            self.flush_pending(view, port);
            if !self.machine_packet {
                match item.kind {
                    DecodedKind::Key(key) => view.machines.key(key),
                    DecodedKind::Mouse(event) if self.pointer.capture_enabled() => {
                        self.pointer.external_capture_event(event);
                        view.machines.pointer(event);
                    }
                    _ => {}
                }
            }
            if !view.machines.open || view.machines.request.is_some() {
                self.machine_packet = true;
            }
            self.pointer.invalidate_map_with_port(port);
            return Ok(DispatchOutcome::Continue);
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
        if let DecodedKind::Mouse(event) = item.kind
            && self.pointer.capture_enabled()
            && view.machines.pointer(event)
        {
            self.flush_pending(view, port);
            self.pointer.external_capture_event(event);
            self.machine_packet = true;
            self.pointer.invalidate_map_with_port(port);
            return Ok(DispatchOutcome::Continue);
        }
        #[cfg(windows)]
        if match &item.kind {
            DecodedKind::Key(event) => event.kind != KeyKind::Release,
            DecodedKind::Paste { .. } => true,
            DecodedKind::Mouse(event) => {
                event.action != super::mouse::MouseAction::Release
                    && event.button != super::mouse::MouseButton::None
            }
            _ => false,
        } {
            port.observe_input_intent();
        }
        if !self.confirm_packet
            && let DecodedKind::Mouse(event) = item.kind
            && let Some(decision) = self.pointer.confirmation_event(port, event)
        {
            self.flush_pending(view, port);
            self.confirm_packet = decision;
            if decision {
                self.keys.capture_pending();
            }
            return Ok(DispatchOutcome::Continue);
        }
        if self.confirm_packet || port.capture_confirmation_key(client_key(&item)) {
            #[cfg(windows)]
            {
                view.exit_resize_mode();
                view.machines.leave_sidebar_navigation();
                port.cancel_copy_mode();
            }
            self.flush_pending(view, port);
            self.confirm_packet = true;
            return Ok(DispatchOutcome::Continue);
        }
        if let DecodedKind::Mouse(event) = item.kind
            && port.agent_board_input()
        {
            self.flush_pending(view, port);
            let decision = self.pointer.board_event(port, event);
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
            self.settle_board_edge(port);
            return Ok(DispatchOutcome::Continue);
        }
        if let DecodedKind::Mouse(event) = item.kind
            && self.pointer.help_event(view, event)
        {
            self.flush_pending(view, port);
            if event.action == super::MouseAction::Press && event.button == super::MouseButton::Left
            {
                self.modal_release = Some(event.button);
                self.pointer_packet = true;
                self.keys.capture_pending();
            }
            return Ok(DispatchOutcome::Continue);
        }
        #[cfg(windows)]
        if self.copy_packet || port.copy_mode_active() {
            self.flush_pending(view, port);
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
                        port.capture_copy_mode_paste(text.as_deref());
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
        if self.pointer.tab_drag_captured() && !matches!(item.kind, DecodedKind::Mouse(_)) {
            self.flush_pending(view, port);
            if matches!(item.kind, DecodedKind::Key(event) if event.kind != KeyKind::Release
                && event.code == crate::client_view::KeyCode::Esc && event.mods == crate::client_view::Modifiers::NONE)
            {
                self.pointer.cancel_tab_drag();
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
            if sidebar_ended || self.pointer.cancel_divider() {
                self.flush_pending(view, port);
                if sidebar_ended {
                    self.capture_sidebar_redraw(port);
                }
                return Ok(DispatchOutcome::Continue);
            }
        }
        #[cfg(windows)]
        if view.resize_mode() {
            self.flush_pending(view, port);
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
                    crate::machine_picker::SidebarIntent::OpenMenu(target) => {
                        if let Some(target) = self.pointer.sidebar_menu_target(view, &target) {
                            mode(ModeRequest::BracketedPaste(true))?;
                            if port.open_action_menu(target, None, view.pointer_released()) {
                                self.menu_owned = true;
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
                    // stays armed — unless the open board owns input, in
                    // which case the whole run is swallowed like every
                    // other item under its capture.
                    if !port.agent_board_input() {
                        forward_or_report(view, &item.raw, port);
                    }
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
                // also stales the map — the gesture that owns the pointer
                // survives either way.
                self.flush_pending(view, port);
                self.settle_board_edge(port);
                match self.pointer.event(view, port, now, event) {
                    PointerOutcome::Ignored => {}
                    #[cfg(windows)]
                    PointerOutcome::TabCreateRequested(target) => {
                        // Creation may replace the input target. Fence this packet's tail,
                        // including a refused stale click, like other chrome action handoffs.
                        self.menu_packet = true;
                        self.keys.capture_pending();
                        self.pointer.invalidate_map_with_port(port);
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
                    PointerOutcome::SettingsRequested => {
                        let transition = view.decide_action(ClientAction::OpenSettings);
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
                    #[cfg(windows)]
                    PointerOutcome::NewWorkspaceRequested => {
                        self.menu_packet = true;
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
                    #[cfg(windows)]
                    PointerOutcome::SidebarFocusRequested => {
                        let transition = view.decide_action(ClientAction::FocusBusinessSidebar);
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
                    #[cfg(windows)]
                    PointerOutcome::SidebarDisclosureRequested(target) => {
                        if !matches!(target, crate::machine_picker::SidebarTarget::Agent { .. }) {
                            // A disclosure is chrome owned even if its painted identity became
                            // stale before this read. Fence the whole packet and retire that map;
                            // only a still-current target may mutate presentation state.
                            let project = matches!(
                                target,
                                crate::machine_picker::SidebarTarget::Project { .. }
                            );
                            let current = view.machines.sidebar_target_exists(&target);
                            // Project disclosure also enters sidebar keyboard navigation and must
                            // acquire its terminal-input lease. Context disclosure is display-only.
                            if project && current {
                                mode(ModeRequest::BracketedPaste(true))?;
                            }
                            self.sidebar_packet = true;
                            self.keys.capture_pending();
                            self.pointer.invalidate_map_with_port(port);
                            let changed = if current {
                                match &target {
                                    crate::machine_picker::SidebarTarget::Project { .. } => {
                                        if view.machines.focus_sidebar_target(&target) {
                                            view.machines.toggle_sidebar_project(&target);
                                            true
                                        } else {
                                            false
                                        }
                                    }
                                    crate::machine_picker::SidebarTarget::Context { .. } => {
                                        // Disclosure changes only Client-local presentation. It
                                        // never activates or focuses the Workspace/terminal it
                                        // annotates.
                                        view.machines.toggle_sidebar_context_agents(&target)
                                    }
                                    crate::machine_picker::SidebarTarget::Agent { .. } => false,
                                }
                            } else {
                                false
                            };
                            if changed {
                                self.reconcile(view, port, now);
                                after_layout(view, port)?;
                            } else if project {
                                mode(ModeRequest::BracketedPaste(
                                    view.machines.sidebar_navigation_active(),
                                ))?;
                            }
                        }
                    }
                    outcome => {
                        if after_layout(view, port)? {
                            // A host sample can change dimensions inside this read,
                            // even when the Client viewport class stays unchanged.
                            self.pointer.invalidate_map_with_port(port);
                        }
                        #[cfg(windows)]
                        if outcome == PointerOutcome::MachineNavigation {
                            self.machine_packet = true;
                            self.pointer.invalidate_map_with_port(port);
                        }
                        #[cfg(windows)]
                        if outcome == PointerOutcome::TabGestureEnded {
                            self.menu_packet = true;
                            self.keys.capture_pending();
                            self.pointer.invalidate_map_with_port(port);
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
                                self.menu_owned = true;
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
            DecodedKind::Ignored => {
                // Mouse-shaped bytes that failed validation or never
                // completed: pointer protocol consumed whole — never keys,
                // never pending, never forwarded raw to the pane.
            }
            _ if port.agent_board_input() => {
                // The painted Agent board owns keyboard input between the
                // armed prefix and the pane: keys feed its capture
                // handler, paste and non-decision items are swallowed —
                // nothing typed here ever reaches the pane.
                self.flush_pending(view, port);
                if let DecodedKind::Key(event) = item.kind {
                    let _captured = port.capture_agent_board_key(event);
                }
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
                        | ClientAction::OpenSettings
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
                    && !view.machines.open
                    && !port.popup_captured()
                    && !port.copy_mode_active()
                    && !port.agent_board_input()
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
            let next = !self.pointer.capture_enabled();
            mode(ModeRequest::PointerCapture(next))?;
            self.set_pointer_capture(view, port, next);
        }
        if applied.is_none()
            && let Some(action) = action.as_ref().and_then(ClientAction::backend_action)
        {
            view.report_backend_unavailable(action);
        }
        self.reconcile(view, port, now);
        let capture_admitted = self.epoch.is_some();
        #[cfg(windows)]
        let capture_admitted = capture_admitted
            || view.navigator_active()
            || port.copy_mode_active()
            || port.action_menu_active()
            || port.settings_active()
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

    fn reconcile(
        &mut self,
        view: &mut ClientView,
        port: &mut (impl TerminalInputPort + ?Sized),
        now: Instant,
    ) -> bool {
        self.settle_board_edge(port);

        #[cfg(windows)]
        if port.popup_captured()
            || view.machines.open
            || port.agent_board_input()
            || port.settings_active()
        {
            view.exit_resize_mode();
            view.machines.leave_sidebar_navigation();
            port.cancel_copy_mode();
        }
        let setup = port.setup_input_context();
        let current = setup
            .as_ref()
            .map(|context| ModalEpoch::Setup(context.token()))
            .or_else(|| {
                port.rename_input_context()
                    .map(|context| ModalEpoch::Rename(context.token()))
            });
        #[cfg(windows)]
        let current = port
            .worktree_input_context()
            .map(|c| ModalEpoch::Worktree(c.token))
            .or(current);
        #[cfg(windows)]
        let navigator_changed = self.reconcile_navigator(view, port, current.is_some());
        #[cfg(not(windows))]
        let navigator_changed = false;
        #[cfg(windows)]
        if current.is_some() || port.copy_mode_active() || view.resize_mode() {
            view.machines.leave_sidebar_navigation();
        }
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
        #[cfg(windows)]
        if current.is_some()
            || port.popup_captured()
            || view.machines.open
            || port.agent_board_input()
            || port.settings_active()
        {
            port.cancel_action_menu();
        }
        let settings_changed = {
            let owned = port.settings_active();
            let changed = owned != self.settings_owned;
            if changed {
                if owned {
                    self.settings_owned = true;
                    self.keys.capture_pending();
                    self.pointer.invalidate_map_with_port(port);
                } else {
                    self.settle_settings_closed(port);
                }
            }
            changed
        };
        #[cfg(windows)]
        let menu_changed = {
            let owned = port.action_menu_active();
            let changed = owned != self.menu_owned;
            if changed {
                if owned {
                    self.menu_owned = true;
                    self.keys.capture_pending();
                    self.pointer.invalidate_map_with_port(port);
                } else {
                    self.settle_action_menu_closed(port);
                }
            }
            changed
        };
        #[cfg(not(windows))]
        let menu_changed = false;
        let choice_keys = setup.as_ref().is_some_and(|context| {
            matches!(
                context.stage(),
                NativeSetupStage::QuickProject | NativeSetupStage::ChooseWorkspaceProject
            )
        });
        #[cfg(windows)]
        let choice_keys = choice_keys
            || port.worktree_input_context().is_some_and(|context| {
                context.stage == crate::terminal_session::WorktreeStage::Checkout
            });
        self.decoder.choice_keys(choice_keys);
        #[cfg(windows)]
        {
            if current.is_some() {
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
            return menu_changed || settings_changed || sidebar_changed || navigator_changed;
        }
        if self.epoch.take().is_some() {
            let events = self.decoder.cancel(now);
            self.apply_events(view, port, events, now);
        }
        let capacity = if setup.is_some_and(|context| {
            matches!(
                context.stage(),
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
                if !focused {
                    self.pointer.cancel_list_scroll();
                    self.pointer.cancel_modal_press();
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
                        Some(ModalEpoch::Setup(_)) => port
                            .setup_input_context()
                            .map(|context| ModalEpoch::Setup(context.token())),
                        #[cfg(windows)]
                        Some(ModalEpoch::Worktree(_)) => port
                            .worktree_input_context()
                            .map(|context| ModalEpoch::Worktree(context.token)),
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
                        ModalEpoch::Setup(_) => port
                            .setup_input_context()
                            .map(|context| ModalEpoch::Setup(context.token())),
                        #[cfg(windows)]
                        ModalEpoch::Worktree(_) => port
                            .worktree_input_context()
                            .map(|context| ModalEpoch::Worktree(context.token)),
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
        use crate::terminal_session::{WorktreeEdit as E, WorktreeStage as S};
        let Some(context) = port.worktree_input_context().filter(|c| c.token == token) else {
            return;
        };
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
            && let Some(context) = port.worktree_input_context()
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
        let Some(context) = port
            .setup_input_context()
            .filter(|context| context.token() == token)
        else {
            return;
        };
        let edit = match event {
            ModalInputEvent::NextField => NativeSetupEdit::NextField,
            ModalInputEvent::PreviousChoice => NativeSetupEdit::SelectPreviousProject,
            ModalInputEvent::NextChoice => NativeSetupEdit::SelectNextProject,
            ModalInputEvent::Append(ref text)
                if matches!(
                    context.stage(),
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
            && let Some(context) = port.setup_input_context()
            && matches!(
                context.stage(),
                NativeSetupStage::ChooseWorkspaceProject
                    | NativeSetupStage::QuickProject
                    | NativeSetupStage::QuickName
                    | NativeSetupStage::QuickDirectory
            )
        {
            // Selection changes draw authority, while this decoder still owns the same picker.
            self.epoch = Some(ModalEpoch::Setup(context.token()));
            self.decoder
                .set_capacity(if context.stage() == NativeSetupStage::QuickDirectory {
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
            self.modal_release = None;
            self.pointer.cancel_modal_press();
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
}
