//! Client-local window and input decisions for the shared Agent projection.
use super::{Mode, NativeStartupTarget, Picker, Row, Selection, SidebarTarget};

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
pub(super) struct Sidebar<S> {
    pub(super) visible: bool,
    top: usize,
    follow_focus: bool,
    pub(super) scroll: Option<Scroll>,
    pub(super) settings: std::sync::Arc<S>,
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct AgentSidebar {
    pub(super) settings: std::sync::Arc<orchbee_local_site_runtime::WindowsAgentSidebarSettings>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Scroll {
    up: bool,
    pending: usize,
    capacity: usize,
    maximum: usize,
}
impl Scroll {
    pub(super) fn queue(pending: &mut Option<Self>, window: SidebarWindow, up: bool) {
        if window.capacity == 0 {
            return;
        }
        let count = pending
            .filter(|scroll| scroll.up == up)
            .map_or(0, |scroll| scroll.pending);
        *pending = Some(Self {
            up,
            pending: count.saturating_add(3).min(window.total),
            capacity: window.capacity,
            maximum: window.maximum,
        });
    }

    pub(super) fn settle(pending: &mut Option<Self>, top: &mut usize, total: usize) -> bool {
        let Some(mut scroll) = pending.take() else {
            return false;
        };
        let count = scroll.pending.min(scroll.capacity);
        let maximum = scroll.maximum.min(total.saturating_sub(1));
        let before = (*top).min(maximum);
        let next = if scroll.up {
            before.saturating_sub(count)
        } else {
            before.saturating_add(count).min(maximum)
        };
        *top = next;
        scroll.pending -= count;
        if scroll.pending > 0 && next != before {
            *pending = Some(scroll);
        }
        next != before
    }
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
    }
    pub(crate) fn configure_agent_sidebar(
        &mut self,
        settings: orchbee_local_site_runtime::WindowsAgentSidebarSettings,
    ) {
        if self.agent_sidebar.settings.as_ref() != &settings {
            self.agent_sidebar.settings = std::sync::Arc::new(settings);
            self.spaces.reset();
            self.invalidate();
        }
    }
    pub(crate) fn sidebar_settings(
        &self,
    ) -> &orchbee_local_site_runtime::WindowsAgentSidebarSettings {
        &self.agent_sidebar.settings
    }
    pub(crate) fn cancel_sidebar_scroll(&mut self) {
        self.spaces.scroll = None;
    }
    pub(crate) fn settle_sidebar_scroll(&mut self) -> bool {
        if self.open {
            self.cancel_sidebar_scroll();
            return false;
        }
        self.spaces.settle(self.sidebar_navigation_rows().len())
    }
    pub(crate) fn show_agents(&mut self) {
        self.cancel_sidebar_scroll();
        self.show();
        self.set_mode(Mode::Agents);
    }
    pub(crate) fn show_agent_search(&mut self) {
        self.show_agents();
        self.agent_control(super::Control::Search);
    }
    pub(crate) fn activate_sidebar_agent(&mut self, selected: &Selection) -> bool {
        if !self.available || !self.spaces.visible() || self.open {
            return false;
        }
        let target = match &selected.context {
            Some(NativeStartupTarget::Agent(target)) => SidebarTarget::Agent {
                machine: selected.machine.clone(),
                target: target.clone(),
            },
            _ => return false,
        };
        if !self
            .sidebar_navigation_rows()
            .iter()
            .any(|row| row.target() == target)
        {
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
    pub(crate) fn activate_sidebar_selection(&mut self, selected: &Selection) -> bool {
        match &selected.context {
            Some(NativeStartupTarget::Agent(_)) => self.activate_sidebar_agent(selected),
            _ => self.activate_sidebar_workspace(selected),
        }
    }
}

fn fit_end_with_gaps(heights: &[u16], gaps: &[u16], top: usize, cells: u16) -> usize {
    if cells == 0 {
        return top;
    }
    let mut used = 0usize;
    let mut end = top;
    for (index, &height) in heights.iter().enumerate().skip(top) {
        let cost = usize::from(height.max(1).min(cells))
            + if index == top {
                0
            } else {
                usize::from(gaps[index - 1])
            };
        if used + cost > usize::from(cells) {
            break;
        }
        used += cost;
        end += 1;
    }
    end
}

fn fit_backwards_with_gaps(heights: &[u16], gaps: &[u16], end: usize, cells: u16) -> usize {
    if cells == 0 {
        return 0;
    }
    let mut top = end;
    let mut used = 0usize;
    for (index, &height) in heights[..end].iter().enumerate().rev() {
        let cost = usize::from(height.max(1).min(cells))
            + if top == end {
                0
            } else {
                usize::from(gaps[index])
            };
        if used + cost > usize::from(cells) {
            break;
        }
        used += cost;
        top -= 1;
    }
    top
}

impl<S> Sidebar<S> {
    pub(super) fn window_with_gaps(
        &self,
        heights: &[u16],
        gaps: &[u16],
        cells: u16,
        current: Option<usize>,
    ) -> SidebarWindow {
        let total = heights.len().min(gaps.len());
        let heights = &heights[..total];
        let gaps = &gaps[..total];
        let maximum = if cells == 0 {
            0
        } else {
            fit_backwards_with_gaps(heights, gaps, total, cells)
        };
        let mut top = self.top.min(maximum);
        if self.follow_focus
            && cells > 0
            && let Some(current) = current.filter(|&index| index < total)
        {
            if current < top {
                top = current;
            } else if current >= fit_end_with_gaps(heights, gaps, top, cells) {
                top = fit_backwards_with_gaps(heights, gaps, current + 1, cells);
            }
        }
        SidebarWindow {
            top,
            capacity: fit_end_with_gaps(heights, gaps, top, cells).saturating_sub(top),
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
        Scroll::queue(&mut self.scroll, window, up);
    }
    fn settle(&mut self, total: usize) -> bool {
        if !self.visible {
            self.scroll = None;
            return false;
        }
        Scroll::settle(&mut self.scroll, &mut self.top, total)
    }
}
