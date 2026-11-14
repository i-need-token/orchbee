//! Navigator capture uses the shared decoder and successful frame map.
use super::{ClientView, DecodedItem, DecodedKind, InputDispatcher, KeyKind, TerminalInputPort};
use crate::client_view::{
    ClientTransition, Notice, PrefixAction, PrefixInput,
    navigator::{Admission, Command, Reconcile, Row},
};
use crate::terminal_input::{MouseAction, MouseButton};
use crate::tui_hit_map::{InteractionTarget, PrimaryContract};
use crate::tui_pointer::PointerOutcome;

#[derive(Default)]
pub(super) struct Capture {
    pub(super) owned: bool,
    pub(super) packet: bool,
    pub(super) cancel_capture: bool,
    pub(super) unpresented: bool,
    pub(super) release: Option<MouseButton>,
}

impl InputDispatcher {
    pub(super) fn capture_navigator_release(&mut self, item: &DecodedItem) -> bool {
        let DecodedKind::Mouse(event) = item.kind else {
            return false;
        };
        if event.action == MouseAction::Press
            && !event.mods.shift
            && matches!(
                event.button,
                MouseButton::Left | MouseButton::Right | MouseButton::Middle
            )
        {
            self.navigator.release = None;
        }
        if event.action == MouseAction::Release
            && self
                .navigator
                .release
                .is_some_and(|button| event.button == button || event.button == MouseButton::None)
        {
            self.navigator.release = None;
            return true;
        }
        false
    }

    pub(super) fn reconcile_navigator(
        &mut self,
        view: &mut ClientView,
        port: &mut (impl TerminalInputPort + ?Sized),
        exclusive_capture: bool,
    ) -> bool {
        let mut changed = false;
        if view.navigator_active() {
            if exclusive_capture || port.copy_mode_active() || port.pointer_probe().input_captured {
                view.close_navigator(None);
            } else if self.navigator.unpresented {
                view.close_navigator(Some(Notice::NavigatorTooSmall));
            } else {
                match view.navigator.reconcile(port.navigator_catalog()) {
                    Reconcile::Unchanged => {}
                    Reconcile::Changed => {
                        self.pointer.invalidate_map_with_port(port);
                        changed = true;
                    }
                    Reconcile::Cancel => view.close_navigator(Some(Notice::NavigatorChanged)),
                }
            }
        }
        let owned = view.navigator_active();
        if owned != self.navigator.owned {
            self.navigator.owned = owned;
            self.navigator.unpresented = false;
            self.navigator.packet = true;
            if !owned {
                self.navigator.cancel_capture = true;
                view.navigator.close();
            }
            self.keys.capture_pending();
            self.pointer.invalidate_map_with_port(port);
            changed = true;
        }
        changed
    }

    fn activate_navigator_row(
        &mut self,
        view: &mut ClientView,
        port: &mut (impl TerminalInputPort + ?Sized),
        row: Row,
        keyboard: bool,
    ) {
        if view.navigator.accepts(&row, keyboard)
            && matches!(
                port.activate_navigator(&row),
                Admission::ExactCurrent | Admission::Admitted(_)
            )
        {
            view.close_navigator(None);
        }
        self.navigator.packet = true;
        self.keys.capture_pending();
    }

    pub(super) fn navigator_item(
        &mut self,
        view: &mut ClientView,
        port: &mut (impl TerminalInputPort + ?Sized),
        item: &DecodedItem,
    ) -> Option<ClientTransition> {
        match item.kind {
            DecodedKind::Key(key) if key.kind != KeyKind::Release => {
                if view.navigator.prefix {
                    return Some(view.decide_navigator_prefix(PrefixInput::Key(key)));
                }
                let command = crate::client_view::navigator::command(key);
                if command == Some(Command::Exit) {
                    view.close_navigator(None);
                    return None;
                }
                if view.bindings().prefix.matches(key) {
                    view.navigator.prefix = true;
                    return None;
                }
                if view
                    .bindings()
                    .find(key, false)
                    .is_some_and(|member| member.action == PrefixAction::NavigatorToggle)
                {
                    view.close_navigator(None);
                    return None;
                }
                if command == Some(Command::Open) {
                    let row = self.pointer.navigator_paint().and_then(|paint| {
                        paint
                            .rows
                            .iter()
                            .find(|row| Some(&row.target) == paint.focus.as_ref())
                            .cloned()
                    });
                    if let Some(row) = row {
                        self.activate_navigator_row(view, port, row, true);
                    }
                } else if let Some(command) = command {
                    let page = self
                        .pointer
                        .navigator_paint()
                        .map_or(1, |paint| paint.page_rows);
                    if view.navigator.move_focus(command, page) {
                        self.pointer.invalidate_map_with_port(port);
                    }
                }
            }
            DecodedKind::Mouse(event) => match self.pointer.navigator_event(event) {
                PointerOutcome::Interaction {
                    target: InteractionTarget::NavigatorRow(row),
                    contract: PrimaryContract::ActivateOnRelease,
                } => self.activate_navigator_row(view, port, row, false),
                PointerOutcome::Interaction {
                    target: InteractionTarget::NavigatorClose(source),
                    contract: PrimaryContract::ActivateOnRelease,
                } => {
                    let _closed = view.close_navigator_for_source(&source);
                    self.navigator.packet = true;
                    self.keys.capture_pending();
                }
                PointerOutcome::Interaction { .. } | PointerOutcome::InteractionEnded => {
                    // The exact painted target owns its physical release even
                    // if its source has become stale. Never re-hit a replacement.
                    self.navigator.packet = true;
                    self.keys.capture_pending();
                }
                PointerOutcome::Ignored
                    if self.pointer.capture_enabled()
                        && event.action == MouseAction::Press
                        && !event.mods.shift
                        && !matches!(
                            event.button,
                            MouseButton::WheelUp | MouseButton::WheelDown | MouseButton::None
                        )
                        && self.pointer.navigator_backdrop_at(event.column, event.row) =>
                {
                    // Backdrop dismissal intentionally remains press-time and
                    // keeps its legacy release debt after the Navigator closes.
                    self.navigator.release = Some(event.button);
                    view.close_navigator(None);
                }
                PointerOutcome::Ignored
                | PointerOutcome::Handled
                | PointerOutcome::IdleMotion { .. }
                | PointerOutcome::RegionsChanged => {}
                PointerOutcome::TabGestureEnded
                | PointerOutcome::SidebarReorderEnded
                | PointerOutcome::SidebarWidthEnded
                | PointerOutcome::MenuRequested { .. }
                | PointerOutcome::ChromeGestureEnded => {
                    unreachable!("Navigator routing cannot produce a chrome outcome")
                }
            },
            _ => {}
        }
        None
    }
}
