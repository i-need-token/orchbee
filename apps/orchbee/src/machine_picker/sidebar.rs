//! Client-local window and input decisions for the shared Agent projection.
use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SidebarWindow {
    pub(crate) top: usize,
    pub(crate) capacity: usize,
    pub(crate) total: usize,
    pub(crate) maximum: usize,
}
impl SidebarWindow {
    pub(crate) fn end(self) -> usize {
        self.top.saturating_add(self.capacity).min(self.total)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Sidebar<S = orchbee_local_site_runtime::WindowsAgentSidebarSettings> {
    pub(super) visible: bool,
    top: usize,
    follow_focus: bool,
    pub(super) scroll: Option<Scroll>,
    pub(super) settings: std::sync::Arc<S>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Scroll {
    up: bool,
    pending: usize,
    capacity: usize,
    maximum: usize,
}
impl<S: Default> Default for Sidebar<S> {
    fn default() -> Self {
        Self {
            visible: false,
            top: 0,
            follow_focus: true,
            scroll: None,
            settings: Default::default(),
        }
    }
}
impl<S> Sidebar<S> {
    pub(super) fn reset(&mut self) {
        self.top = 0;
        self.follow_focus = true;
        self.scroll = None;
    }
    pub(super) fn visible(&self) -> bool {
        self.visible
    }
    pub(super) fn reveal(&mut self) {
        self.follow_focus = true;
        self.scroll = None;
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
    pub(crate) const fn sidebar_width(&self) -> crate::tui_layout::SidebarWidth {
        self.sidebar_width
    }
    /// The input owner first validates a live painted source; this compares its preference.
    pub(crate) fn set_sidebar_width(
        &mut self,
        expected: crate::tui_layout::SidebarWidth,
        next: crate::tui_layout::SidebarWidth,
    ) -> bool {
        if !self.available
            || !self.spaces.visible()
            || self.open
            || self.sidebar_width != expected
            || next == expected
        {
            return false;
        }
        self.sidebar_width = next;
        self.spaces.reveal();
        self.sidebar.reveal();
        self.invalidate();
        true
    }
    /// Demand follows the actual height/width tier, not the sidebar intent alone.
    pub(crate) fn observe_agent_sidebar(&mut self, visible: bool) {
        if !visible {
            self.leave_sidebar_navigation();
        }
        if self.spaces.visible != visible {
            self.spaces.visible = visible;
            self.spaces.reset();
        }
        if self.sidebar.visible != visible {
            self.sidebar.visible = visible;
            self.sidebar.reset();
        }
    }
    pub(crate) fn sidebar_rows(&self) -> &[Row] {
        &self.agent_rows
    }
    #[cfg(test)]
    pub(crate) fn sidebar_window(&self, capacity: usize) -> SidebarWindow {
        self.sidebar_window_for_heights(
            &vec![1; self.agent_rows.len()],
            capacity.min(usize::from(u16::MAX)) as u16,
            0,
        )
    }
    pub(crate) fn configure_agent_sidebar(
        &mut self,
        settings: orchbee_local_site_runtime::WindowsAgentSidebarSettings,
    ) {
        if self.sidebar.settings.as_ref() != &settings {
            self.sidebar.settings = std::sync::Arc::new(settings);
            self.sidebar.reset();
            self.invalidate();
        }
    }
    pub(crate) fn sidebar_settings(
        &self,
    ) -> &orchbee_local_site_runtime::WindowsAgentSidebarSettings {
        &self.sidebar.settings
    }
    pub(crate) fn sidebar_window_for_heights(
        &self,
        heights: &[u16],
        cells: u16,
        gap: u16,
    ) -> SidebarWindow {
        let total = heights.len().min(self.agent_rows.len());
        let heights = &heights[..total];
        let current = self.agent_rows.iter().position(|row| {
            row.agent
                .as_ref()
                .is_some_and(|agent| self.current_agent(&row.choice, agent))
        });
        self.sidebar.window(&heights[..total], cells, gap, current)
    }
    pub(crate) fn queue_sidebar_scroll(&mut self, window: SidebarWindow, up: bool) {
        if self.available && self.sidebar.visible && !self.open {
            self.sidebar.queue(window, up);
        }
    }
    pub(crate) fn cancel_sidebar_scroll(&mut self) {
        self.sidebar.scroll = None;
        self.spaces.scroll = None;
    }
    pub(crate) fn settle_sidebar_scroll(&mut self) -> bool {
        if self.open {
            self.cancel_sidebar_scroll();
            return false;
        }
        let agents = self.sidebar.settle(self.agent_rows.len());
        let spaces = self.spaces.settle(self.sidebar_navigation_rows().len());
        agents | spaces
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

fn fit_end(heights: &[u16], top: usize, cells: u16, gap: u16) -> usize {
    if cells == 0 {
        return top;
    }
    let mut used = 0usize;
    let mut end = top;
    for &height in heights.iter().skip(top) {
        let cost =
            usize::from(height.max(1).min(cells)) + if end == top { 0 } else { usize::from(gap) };
        if used + cost > usize::from(cells) {
            break;
        }
        used += cost;
        end += 1;
    }
    end
}
fn fit_backwards(heights: &[u16], end: usize, cells: u16, gap: u16) -> usize {
    if cells == 0 {
        return 0;
    }
    let mut top = end;
    let mut used = 0usize;
    for &height in heights[..end].iter().rev() {
        let cost =
            usize::from(height.max(1).min(cells)) + if top == end { 0 } else { usize::from(gap) };
        if used + cost > usize::from(cells) {
            break;
        }
        used += cost;
        top -= 1;
    }
    top
}

impl<S> Sidebar<S> {
    pub(super) fn window(
        &self,
        heights: &[u16],
        cells: u16,
        gap: u16,
        current: Option<usize>,
    ) -> SidebarWindow {
        let total = heights.len();
        let maximum = if cells == 0 {
            0
        } else {
            fit_backwards(heights, total, cells, gap)
        };
        let mut top = self.top.min(maximum);
        if self.follow_focus
            && cells > 0
            && let Some(current) = current.filter(|&i| i < total)
        {
            if current < top {
                top = current;
            } else if current >= fit_end(heights, top, cells, gap) {
                top = fit_backwards(heights, current + 1, cells, gap);
            }
        }
        SidebarWindow {
            top,
            capacity: fit_end(heights, top, cells, gap).saturating_sub(top),
            total,
            maximum,
        }
    }
    pub(super) fn queue(&mut self, window: SidebarWindow, up: bool) {
        if window.capacity == 0 {
            return;
        }
        self.top = window.top;
        self.follow_focus = false;
        let pending = self.scroll.filter(|s| s.up == up).map_or(0, |s| s.pending);
        self.scroll = Some(Scroll {
            up,
            pending: pending.saturating_add(3).min(window.total),
            capacity: window.capacity,
            maximum: window.maximum,
        });
    }
    fn settle(&mut self, total: usize) -> bool {
        if !self.visible {
            self.scroll = None;
            return false;
        }
        let Some(mut scroll) = self.scroll.take() else {
            return false;
        };
        let count = scroll.pending.min(scroll.capacity);
        let maximum = scroll.maximum.min(total.saturating_sub(1));
        let before = self.top.min(maximum);
        let next = if scroll.up {
            before.saturating_sub(count)
        } else {
            before.saturating_add(count).min(maximum)
        };
        self.top = next;
        scroll.pending -= count;
        if scroll.pending > 0 && next != before {
            self.scroll = Some(scroll);
        }
        next != before
    }
}
