//! Client-local window and input decisions for the shared Agent projection.
use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SidebarWindow {
    pub(crate) top: usize,
    pub(crate) capacity: usize,
    pub(crate) total: usize,
}
impl SidebarWindow {
    pub(crate) fn end(self) -> usize {
        self.top.saturating_add(self.capacity).min(self.total)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Sidebar {
    visible: bool,
    top: usize,
    follow_focus: bool,
    scroll: Option<Scroll>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Scroll {
    up: bool,
    pending: usize,
    capacity: usize,
}
impl Default for Sidebar {
    fn default() -> Self {
        Self {
            visible: false,
            top: 0,
            follow_focus: true,
            scroll: None,
        }
    }
}
impl Sidebar {
    pub(super) fn reset(&mut self) {
        self.top = 0;
        self.follow_focus = true;
        self.scroll = None;
    }
    pub(super) fn visible(&self) -> bool {
        self.visible
    }
}

impl Row {
    pub(crate) fn sidebar_agent(&self) -> Option<&crate::machine_agents::Row> {
        self.agent.as_ref()
    }
    pub(crate) fn sidebar_status(&self) -> Option<&'static str> {
        self.agent_status
    }
}
impl Picker {
    /// Demand follows the actual height/width tier, not the sidebar intent alone.
    pub(crate) fn observe_agent_sidebar(&mut self, visible: bool) {
        if self.sidebar.visible != visible {
            self.sidebar.visible = visible;
            self.sidebar.reset();
        }
    }
    pub(crate) fn sidebar_rows(&self) -> &[Row] {
        &self.agent_rows
    }
    pub(crate) fn sidebar_window(&self, capacity: usize) -> SidebarWindow {
        let total = self.agent_rows.len();
        let mut top = self.sidebar.top.min(total.saturating_sub(capacity));
        if self.sidebar.follow_focus
            && capacity > 0
            && let Some(current) = self.agent_rows.iter().position(|row| {
                row.agent
                    .as_ref()
                    .is_some_and(|agent| self.current_agent(&row.choice, agent))
            })
        {
            if current < top {
                top = current;
            } else if current >= top.saturating_add(capacity) {
                top = current + 1 - capacity;
            }
        }
        SidebarWindow {
            top,
            capacity,
            total,
        }
    }
    pub(crate) fn queue_sidebar_scroll(&mut self, window: SidebarWindow, up: bool) {
        if !self.available || !self.sidebar.visible || self.open || window.capacity == 0 {
            return;
        }
        self.sidebar.top = window.top;
        self.sidebar.follow_focus = false;
        let pending = self
            .sidebar
            .scroll
            .filter(|scroll| scroll.up == up)
            .map_or(0, |scroll| scroll.pending);
        self.sidebar.scroll = Some(Scroll {
            up,
            pending: pending.saturating_add(3).min(window.total),
            capacity: window.capacity,
        });
    }
    pub(crate) fn cancel_sidebar_scroll(&mut self) {
        self.sidebar.scroll = None;
    }
    pub(crate) fn settle_sidebar_scroll(&mut self) -> bool {
        if !self.sidebar.visible || self.open {
            self.cancel_sidebar_scroll();
            return false;
        }
        let Some(mut scroll) = self.sidebar.scroll.take() else {
            return false;
        };
        let count = scroll.pending.min(scroll.capacity);
        let maximum = self.agent_rows.len().saturating_sub(scroll.capacity);
        let before = self.sidebar.top.min(maximum);
        let next = if scroll.up {
            before.saturating_sub(count)
        } else {
            before.saturating_add(count).min(maximum)
        };
        self.sidebar.top = next;
        scroll.pending -= count;
        if scroll.pending > 0 && next != before {
            self.sidebar.scroll = Some(scroll);
        }
        next != before
    }
    pub(crate) fn show_agents(&mut self) {
        self.cancel_sidebar_scroll();
        self.show();
        self.set_mode(Mode::Agents);
    }
    pub(crate) fn activate_sidebar_agent(&mut self, selected: &Selection) -> bool {
        if !self.available || !self.sidebar.visible || self.open {
            return false;
        }
        let Some(index) = self.agent_rows.iter().position(|row| {
            row.enabled && row.choice == selected.machine && row.agent.as_ref().is_some_and(|agent| {
                matches!(&selected.context, Some(NativeStartupTarget::Agent(target)) if target.as_ref() == &agent.target)
            })
        }) else { return false; };
        let row = &self.agent_rows[index];
        let current = row
            .agent
            .as_ref()
            .is_some_and(|a| self.current_agent(&row.choice, a));
        self.show_agents();
        self.selected = index;
        if current {
            self.open = false;
            self.request = None;
            self.invalidate();
        } else {
            self.request = Some(selected.clone());
        }
        true
    }
    pub(crate) fn sidebar_view_label(&self, locale: Locale) -> String {
        if let Some(spec) = self.source_spec() {
            return format!(
                "{}: {}",
                locale.machine_picker_text("agent-source"),
                display_label(spec.label.as_deref().unwrap_or(&spec.source))
            );
        }
        format!(
            "{} · {} · {}{}",
            locale.machine_agent_control(Control::State, &self.agent_view),
            locale.machine_agent_control(Control::Order, &self.agent_view),
            locale.machine_agent_control(Control::Scope, &self.agent_view),
            if self.agent_view.query.is_empty() {
                String::new()
            } else {
                format!(" · / {}", self.agent_view.query)
            }
        )
    }
}
