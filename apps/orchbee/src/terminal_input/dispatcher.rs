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

#[derive(Default)]
pub(crate) struct InputDispatcher {
    graphics: crate::terminal_graphics::Probe,
    #[cfg(windows)]
    graphics_replies: Vec<crate::terminal_graphics::Reply>,
    #[cfg(windows)]
    graphics_reply_overflow: bool,
    decoder: ModalDecoder,
    keys: LegacyDecoder,
    epoch: Option<ModalEpoch>,
    /// Decoded bytes of the in-flight packet that still owe an ordered
    /// forward; always drained before `dispatch`/`synchronize` returns.
    pending: Vec<u8>,
    /// An open confirmation captured the current packet whole, so every
    /// later item of the same read stays swallowed.
    confirm_packet: bool,
    #[cfg(windows)]
    machine_packet: bool,
    #[cfg(windows)]
    popup_packet: bool,
    #[cfg(windows)]
    copy_packet: bool,
    #[cfg(windows)]
    copy_owned: bool,
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
        self.pointer.set_capture_with_port(port, enabled);
        view.settle_pointer_released(!enabled);
    }

    /// The renderer's finished frame map becomes the resolution authority.
    pub(crate) fn publish_hit_map(&mut self, map: FrameHitMap) {
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
        self.pointer.cancel_divider();
    }

    /// Immutable preview supplied by the gesture owner for one paint pass.
    #[cfg(windows)]
    pub(crate) fn divider_preview(&self) -> Option<&crate::tui_layout::DividerDrag> {
        self.pointer.divider_preview()
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
        self.graphics.tick(now);
        let owned_input = self.decoder.owns_input();
        #[cfg(windows)]
        let owned_input = owned_input || port.copy_mode_active();
        // An idle read is a packet boundary too: a `Released` guard left
        // over from an earlier packet ends here so the lease is returned
        // even when no further bytes ever arrive. A release produced by
        // this call's own reconcile still guards the current packet tail.
        self.decoder.end_released(now);
        let mut changed = self.reconcile(view, port, now);
        let events = self.decoder.tick(now);
        changed |= !events.is_empty();
        self.apply_events(view, port, events, now);
        self.confirm_packet = false;
        #[cfg(windows)]
        {
            self.machine_packet = false;
            self.popup_packet = false;
            self.copy_packet = false;
        }
        for item in self.keys.tick(now) {
            changed = true;
            self.handle_item(view, port, now, item, &mut |_, _| Ok(()), mode)?;
        }
        self.flush_pending(view, port);
        changed |= self.pointer.tick(view, port, now);
        changed |= self.reconcile(view, port, now);
        let capturing = self.decoder.owns_input();
        #[cfg(windows)]
        let capturing = capturing || port.copy_mode_active();
        if !capturing {
            mode(ModeRequest::BracketedPaste(false))?;
        } else if !owned_input {
            // A later wizard step can arrive after the previous decoder released its lease.
            mode(ModeRequest::BracketedPaste(true))?;
        }
        Ok(changed)
    }

    pub(crate) fn dispatch<Port: TerminalInputPort + ?Sized, Error>(
        &mut self,
        view: &mut ClientView,
        input: &[u8],
        port: &mut Port,
        now: Instant,
        mut after_layout: impl FnMut(&mut ClientView, &mut Port) -> Result<(), Error>,
        mode: &mut impl FnMut(ModeRequest) -> Result<(), Error>,
    ) -> Result<DispatchOutcome, Error> {
        self.synchronize(view, port, now, mode)?;
        // A new read packet begins: the modal's packet-tail guard ends at
        // this boundary, while an in-flight frame or an open ESC
        // disambiguation keeps ownership under its own rules.
        let events = self.decoder.begin_packet(now);
        self.apply_events(view, port, events, now);
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
            let owned = self.confirm_packet;
            #[cfg(windows)]
            let owned = owned || self.machine_packet || self.popup_packet || self.copy_packet;
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
        let _changed = self.pointer.tick(view, port, now);
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
        after_layout: &mut impl FnMut(&mut ClientView, &mut Port) -> Result<(), Error>,
        mode: &mut impl FnMut(ModeRequest) -> Result<(), Error>,
    ) -> Result<DispatchOutcome, Error> {
        if let DecodedKind::HostReply(reply) = item.kind {
            self.flush_pending(view, port);
            self.host_reply(reply, now);
            return Ok(DispatchOutcome::Continue);
        }
        if let DecodedKind::Focus(focused) = item.kind {
            self.flush_pending(view, port);
            port.observe_focus(focused);
            return Ok(DispatchOutcome::Continue);
        }
        if item.owned {
            // The item completes a frame that began inside a captured
            // packet: consumed by that capture, never forwarded or decided.
            self.flush_pending(view, port);
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
                    DecodedKind::Mouse(event) => {
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
        if self.confirm_packet || port.capture_confirmation_key(client_key(&item)) {
            #[cfg(windows)]
            {
                view.exit_resize_mode();
                port.cancel_copy_mode();
            }
            self.flush_pending(view, port);
            self.confirm_packet = true;
            return Ok(DispatchOutcome::Continue);
        }
        #[cfg(windows)]
        if self.copy_packet || port.copy_mode_active() {
            self.flush_pending(view, port);
            if !self.copy_packet {
                match &item.kind {
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
        if matches!(item.kind, DecodedKind::Key(event) if event.kind != KeyKind::Release
            && event.code == crate::client_view::KeyCode::Esc && event.mods == crate::client_view::Modifiers::NONE)
            && self.pointer.cancel_divider()
        {
            self.flush_pending(view, port);
            return Ok(DispatchOutcome::Continue);
        }
        #[cfg(windows)]
        if view.resize_mode() {
            self.flush_pending(view, port);
            if matches!(item.kind, DecodedKind::Key(event) if event.kind == KeyKind::Release) {
                return Ok(DispatchOutcome::Continue);
            }
            return self.decide(view, port, now, item, after_layout, mode);
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
                    outcome => {
                        after_layout(view, port)?;
                        #[cfg(windows)]
                        if outcome == PointerOutcome::MachineNavigation {
                            self.machine_packet = true;
                            self.pointer.invalidate_map_with_port(port);
                        }
                        if outcome == PointerOutcome::RegionsChanged {
                            self.pointer.stale_map();
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
        after_layout: &mut impl FnMut(&mut ClientView, &mut Port) -> Result<(), Error>,
        mode: &mut impl FnMut(ModeRequest) -> Result<(), Error>,
    ) -> Result<DispatchOutcome, Error> {
        #[cfg(windows)]
        self.pointer.cancel_divider();
        let input = match item.kind {
            DecodedKind::Key(event) if event.kind != KeyKind::Release => PrefixInput::Key(event),
            _ => PrefixInput::NonKey,
        };
        let transition = view.decide(input);
        let action = transition.action();
        #[cfg(windows)]
        let requests_copy = action == Some(ClientAction::EnterCopyMode);
        #[cfg(not(windows))]
        let requests_copy = false;
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
                    | ClientAction::RequestWorkspaceDirectory
                    | ClientAction::RequestWorkspaceRename
                    | ClientAction::RequestProjectRename
            )
        );
        if requests_modal || requests_copy {
            // The terminal lease must exist before the state can capture any draft authority.
            mode(ModeRequest::BracketedPaste(true))?;
        }
        let applied = action
            .clone()
            .and_then(|action| port.apply_client_action(action));
        view.commit(
            transition,
            applied.unwrap_or(SessionActionDisposition::KeepNavigator),
        );
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
            self.pointer.set_capture_with_port(port, next);
            view.settle_pointer_released(!next);
        }
        if applied.is_none()
            && let Some(action) = action.as_ref().and_then(ClientAction::backend_action)
        {
            view.report_backend_unavailable(action);
        }
        self.reconcile(view, port, now);
        let capture_admitted = self.epoch.is_some();
        #[cfg(windows)]
        let capture_admitted = capture_admitted || port.copy_mode_active();
        if (requests_modal || requests_copy) && !capture_admitted {
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
                if let Some(byte) = literal_chord_byte(&item) {
                    forward_or_report(view, std::slice::from_ref(&byte), port);
                } else {
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

    fn reconcile(
        &mut self,
        view: &mut ClientView,
        port: &mut (impl TerminalInputPort + ?Sized),
        now: Instant,
    ) -> bool {
        self.settle_board_edge(port);
        #[cfg(windows)]
        if port.popup_captured() || view.machines.open || port.agent_board_input() {
            view.exit_resize_mode();
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
            return false;
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
                port.observe_focus(focused);
                continue;
            }
            if event == ModalInputEvent::InputFailure {
                view.report_terminal_input_unavailable();
                continue;
            }
            let Some(epoch) = self.epoch else {
                continue;
            };
            let token = match epoch {
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
                | ModalInputEvent::InputFailure
                | ModalInputEvent::Focus(_)
                | ModalInputEvent::HostReply(_) => continue,
            };
            let _accepted = port.edit_rename_draft(token, edit);
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
            ModalInputEvent::Append(ref text)
                if matches!(
                    context.stage(),
                    NativeSetupStage::ChooseProject | NativeSetupStage::QuickProject
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
                NativeSetupStage::ChooseProject
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
}
