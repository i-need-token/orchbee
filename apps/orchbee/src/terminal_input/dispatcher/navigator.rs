//! Navigator capture uses the shared decoder and successful frame map.
use super::*;
use crate::client_view::{
    Notice, PrefixAction,
    navigator::{Admission, Command, Reconcile, Row},
};
use crate::terminal_input::{MouseAction, MouseButton};
use crate::tui_pointer::NavigatorHit;

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
        modal: bool,
    ) -> bool {
        let mut changed = false;
        if view.navigator_active() {
            if modal
                || port.popup_captured()
                || view.machines.open
                || port.agent_board_input()
                || port.settings_active()
                || port.action_menu_active()
                || port.copy_mode_active()
                || view.resize_mode()
                || port.pointer_probe().input_captured
            {
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
    ) {
        match item.kind {
            DecodedKind::Key(key) if key.kind != KeyKind::Release => {
                let command = crate::client_view::navigator::command(key);
                if command == Some(Command::Exit) {
                    view.close_navigator(None);
                    return;
                }
                if view.navigator.prefix {
                    view.navigator.prefix = false;
                    if view
                        .bindings()
                        .find(key, true)
                        .is_some_and(|member| member.action == PrefixAction::NavigatorToggle)
                    {
                        view.close_navigator(None);
                    }
                    return;
                }
                if view.bindings().prefix.matches(key) {
                    view.navigator.prefix = true;
                    return;
                }
                if view
                    .bindings()
                    .find(key, false)
                    .is_some_and(|member| member.action == PrefixAction::NavigatorToggle)
                {
                    view.close_navigator(None);
                    return;
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
            DecodedKind::Mouse(event)
                if self.pointer.capture_enabled()
                    && event.action == MouseAction::Press
                    && !event.mods.shift
                    && !matches!(
                        event.button,
                        MouseButton::WheelUp | MouseButton::WheelDown | MouseButton::None
                    ) =>
            {
                match self.pointer.navigator_hit(event.column, event.row) {
                    Some(NavigatorHit::Row(row))
                        if event.button == MouseButton::Left && event.mods == Modifiers::NONE =>
                    {
                        let row = row.clone();
                        self.navigator.release = Some(event.button);
                        self.activate_navigator_row(view, port, row, false);
                    }
                    Some(NavigatorHit::Backdrop) => {
                        self.navigator.release = Some(event.button);
                        view.close_navigator(None);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}
