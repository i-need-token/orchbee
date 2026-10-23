//! Client-local window and input decisions for the shared Agent projection.
use super::{NativeStartupTarget, Picker, Row, Selection, SelectionOrigin, SidebarTarget};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SidebarWindow {
    pub(crate) top: usize,
    pub(crate) capacity: usize,
    pub(crate) total: usize,
    pub(crate) maximum: usize,
    /// An inert provisional row participates in geometry, but owns no canonical identity.
    pub(crate) insertion: Option<usize>,
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
    order: Vec<SidebarTarget>,
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
            order: Vec::new(),
            follow_focus: true,
            scroll: None,
            settings: Default::default(),
        }
    }
}
impl<S> Sidebar<S> {
    /// Manual reading retains the top identity across topology changes. If removed,
    /// prefer the next surviving row in the old order, then the previous one.
    pub(super) fn reconcile(&mut self, order: &[SidebarTarget]) {
        if self.order == order {
            return;
        }
        if !self.follow_focus {
            self.top = self
                .order
                .iter()
                .skip(self.top)
                .chain(self.order.iter().take(self.top).rev())
                .find_map(|target| order.iter().position(|candidate| candidate == target))
                .unwrap_or(0);
        }
        self.order = order.to_vec();
        self.scroll = None;
    }

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
    #[cfg(test)]
    pub(crate) fn show_agents(&mut self) {
        self.cancel_sidebar_scroll();
        self.show();
        self.set_mode(super::Mode::Agents);
    }
    #[cfg(test)]
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
        if !self.agent_rows.iter().any(|row| {
            row.enabled && row.choice == selected.machine && row.agent.as_ref().is_some_and(|agent| {
                matches!(&selected.context, Some(NativeStartupTarget::Agent(target)) if target.as_ref() == &agent.target)
            })
        }) || !self.begin_selection(SelectionOrigin::Sidebar) {
            return false;
        }
        // The Session checks busy before exact-current. A click on A during A -> B
        // is a new return intent, even while the sidebar still marks A current.
        self.cancel_sidebar_scroll();
        self.request = Some(selected.clone());
        self.error = None;
        self.invalidate();
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
        insertion: Option<usize>,
    ) -> SidebarWindow {
        let total = heights.len().min(gaps.len());
        let heights = &heights[..total];
        let gaps = &gaps[..total];
        let maximum = if cells == 0 {
            0
        } else {
            fit_backwards_with_gaps(heights, gaps, total, cells)
        };
        let mut top = self
            .top
            .saturating_add(usize::from(insertion.is_some_and(|slot| slot <= self.top)))
            .min(maximum);
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
            insertion,
        }
    }
    pub(super) fn queue(&mut self, window: SidebarWindow, up: bool) {
        let window = if let Some(slot) = window.insertion {
            // Store the real row being read, not an index shifted by a temporary creation.
            // An inert slot at the top yields to the following real row on explicit scroll.
            let logical = |index: usize| index.saturating_sub(usize::from(slot < index));
            SidebarWindow {
                top: logical(window.top),
                maximum: logical(window.maximum),
                total: window.total.saturating_sub(1),
                capacity: window
                    .capacity
                    .saturating_sub(usize::from(slot >= window.top && slot < window.end())),
                insertion: None,
            }
        } else {
            window
        };
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

/// Working-glyph phase for the Tab strip and pane titles; the Sidebar shows Working statically.
/// Rendering only reads the phase; the event loop advances it, so every frame remains a pure
/// function of Client state.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct Motion {
    frame: usize,
    advanced: Option<std::time::Instant>,
}

/// Interval between Working glyph frames.
const WORKING_FRAME: std::time::Duration = std::time::Duration::from_millis(100);

impl Picker {
    pub(crate) const fn working_frame(&self) -> usize {
        self.working_motion.frame
    }

    /// Advances the Working glyph only while an observed Agent is Working, and reports whether
    /// the frame changed. Only the Sidebar and the Tab strip demand these observations.
    pub(crate) fn advance_working_motion(&mut self, now: std::time::Instant) -> bool {
        let working = self.agent_observations.iter().any(|(_, observation)| {
            matches!(observation, crate::machine_agents::Observation::Ready(checkpoint)
            if checkpoint.rows.iter().any(|row| {
                row.display_state() == orchbee_workbench_protocol::MetadataState::Working
            }))
        });
        if !working {
            self.working_motion.advanced = None;
            return false;
        }
        if self
            .working_motion
            .advanced
            .is_some_and(|at| now.saturating_duration_since(at) < WORKING_FRAME)
        {
            return false;
        }
        self.working_motion.frame = self.working_motion.frame.wrapping_add(1);
        self.working_motion.advanced = Some(now);
        true
    }
}
