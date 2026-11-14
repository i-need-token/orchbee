//! Machine picker pointer handling: releases and wheel movement on painted picker targets.
use super::{
    Control, DrawnPickerList, DrawnPickerTarget, Mode, Picker, PickerAction, SidebarWindow,
};

impl Picker {
    /// Applies a release only when its complete catalog still describes the
    /// live picker. Visual-only refreshes deliberately preserve that source.
    pub(crate) fn activate_drawn_picker(&mut self, target: &DrawnPickerTarget) -> bool {
        let Some(source) = self.picker_source() else {
            return false;
        };
        if &source != target.source() || !source.available(target.action()) {
            return false;
        }
        self.pending_navigation = None;
        match target.action() {
            PickerAction::ForgetPassword(index) => {
                self.forget_request = self.forget_target(index).cloned();
            }
            PickerAction::Close => self.close_picker(),
            PickerAction::Mode(mode) => self.set_mode(mode),
            PickerAction::Control(control) if self.mode == Mode::Workspaces => {
                if control == Control::Clear {
                    self.workspace_query.clear();
                    self.filter_workspaces(self.selection());
                }
            }
            PickerAction::Control(control) => self.agent_control(control),
            PickerAction::Row(index) => {
                let Some(catalog_row) = source.0.rows.get(index) else {
                    return false;
                };
                if self
                    .visible_rows()
                    .get(index)
                    .is_none_or(|row| row.selection() != catalog_row.selection)
                {
                    return false;
                }
                self.selected = index;
                self.choose(index);
                return self.request.is_some();
            }
        }
        true
    }

    /// Moves at most one painted viewport and returns its replacement witness.
    /// The selection never moves; the next key returns the viewport to it.
    /// Stale geometry and edge movement fail closed without retaining work.
    pub(crate) fn scroll_drawn_picker(
        &mut self,
        target: &DrawnPickerList,
        up: bool,
        lines: usize,
    ) -> Option<DrawnPickerList> {
        let source = self.picker_source()?;
        let window = target.window();
        if &source != target.source() || self.picker_window(window.capacity) != window {
            return None;
        }
        let amount = lines.min(window.capacity);
        let top = if up {
            window.top.saturating_sub(amount)
        } else {
            window.top.saturating_add(amount).min(window.maximum)
        };
        if top == window.top {
            return None;
        }
        self.top = top;
        self.wheel_capacity = Some(window.capacity);
        source.drawn_list(SidebarWindow { top, ..window })
    }
}
