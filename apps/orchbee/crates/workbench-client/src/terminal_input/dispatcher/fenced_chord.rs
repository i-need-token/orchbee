//! Prefix chords inside the fenced tail of a pointer decision resolve after that decision.
//!
//! A decision consumes the rest of its input packet so type-ahead never reaches the new owner
//! before it is visible. Under load the host batches a click with keys typed after it, so a chord
//! typed after the click can share its packet. That chord is Client input rather than type-ahead
//! for the clicked surface: consuming only its introducer would hand the continuation to the
//! terminal. The fence therefore keeps chord keys and resolves them once the keyboard returns to
//! the binding owner; a capturing owner consumes them with the rest of the tail. A key decision
//! consumes its tail whole, chords included, as type-ahead of the keyboard that made it.

use std::time::Instant;

use super::{
    ClientView, DecodedItem, DecodedKind, DispatchOutcome, InputDispatcher, KeyKind, ModeRequest,
    TerminalInputPort,
};
use crate::terminal_input::{MouseAction, MouseButton};

/// Completed chords kept from fenced tails; the bound only limits pathological bursts.
const MAX_CHORDS: usize = 4;

#[derive(Default)]
pub(super) struct FencedChords {
    chords: Vec<[DecodedItem; 2]>,
    introducer: Option<DecodedItem>,
    /// Whether a pointer decision raised the fence now held, so that its tail keeps chords.
    pointer_fence: bool,
}

impl FencedChords {
    /// An item raised the packet fence; only a pointer decision's tail keeps chords.
    pub(super) fn fence_raised(&mut self, pointer: bool) {
        self.pointer_fence = pointer;
    }
}

impl InputDispatcher {
    /// Keeps the chord keys among items a pointer decision's packet fence consumes, in arrival
    /// order; any other fence consumes them with its tail.
    pub(super) fn fence_chord(&mut self, view: &ClientView, item: &DecodedItem) {
        let fenced = &mut self.fenced_chords;
        if !fenced.pointer_fence {
            fenced.introducer = None;
            return;
        }
        match &item.kind {
            DecodedKind::Key(key) if key.kind == KeyKind::Release => {}
            DecodedKind::Key(key) => {
                let prefix = view.bindings().prefix.matches(*key);
                match fenced.introducer.take() {
                    // The literal double prefix is terminal input, so it stays type-ahead.
                    Some(_) if prefix => {}
                    Some(introducer) if fenced.chords.len() < MAX_CHORDS => {
                        fenced.chords.push([introducer, replayable(item)]);
                    }
                    Some(_) => {}
                    None if prefix => fenced.introducer = Some(replayable(item)),
                    None => {}
                }
            }
            DecodedKind::Mouse(event) if button_free_motion(event.action, event.button) => {}
            // Paste or pointer input interrupts an unfinished chord as it would an armed prefix.
            DecodedKind::Paste { .. } | DecodedKind::RejectedPaste | DecodedKind::Mouse(_) => {
                fenced.introducer = None;
            }
            DecodedKind::Ignored
            | DecodedKind::Focus(_)
            | DecodedKind::HostReply(_)
            | DecodedKind::Nondecision => {}
        }
    }

    /// Host focus loss dismisses an unfinished fenced chord exactly like an armed prefix.
    pub(super) fn dismiss_fenced_introducer(&mut self) {
        self.fenced_chords.introducer = None;
    }

    /// Resolves kept chords at a packet boundary that no fence holds. A capturing owner, or an
    /// armed prefix nothing in the tail could have armed, consumes them with the tail instead.
    pub(super) fn replay_fenced_chords<Port: TerminalInputPort + ?Sized, Error>(
        &mut self,
        view: &mut ClientView,
        port: &mut Port,
        now: Instant,
        after_layout: &mut impl FnMut(&mut ClientView, &mut Port) -> Result<bool, Error>,
        mode: &mut impl FnMut(ModeRequest) -> Result<(), Error>,
    ) -> Result<Option<DispatchOutcome>, Error> {
        if self.packet_fenced() {
            return Ok(None);
        }
        let FencedChords {
            chords,
            introducer,
            pointer_fence: _,
        } = std::mem::take(&mut self.fenced_chords);
        if chords.is_empty() && introducer.is_none() {
            return Ok(None);
        }
        let capturing = self.decoder.owns_input() || Self::projected_capture(view, port).is_some();
        if capturing || view.prefix_active() || view.surface_owns_input() {
            return Ok(None);
        }
        for item in chords.into_iter().flatten().chain(introducer) {
            if self.handle_item(view, port, now, item, after_layout, mode)? == DispatchOutcome::Exit
            {
                return Ok(Some(DispatchOutcome::Exit));
            }
        }
        Ok(Some(DispatchOutcome::Continue))
    }
}

/// Hover is not a decision: it never interrupts a chord or dismisses an armed prefix.
pub(super) fn button_free_motion(action: MouseAction, button: MouseButton) -> bool {
    action == MouseAction::Motion && button == MouseButton::None
}

fn replayable(item: &DecodedItem) -> DecodedItem {
    DecodedItem {
        owned: false,
        ..item.clone()
    }
}
