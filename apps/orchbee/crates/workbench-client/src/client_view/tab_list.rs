//! The Mobile tier's Tab list: the Tab strip turned vertical over the whole window. Its rows are
//! the strip's own tokens in painted order; this state keeps only what the person moved, the
//! keyboard focus and the scroll window, and resolves keys against the rows the last frame painted.
//! Every effect a key names is an existing Tab strip action, applied by the dispatcher.

use super::{KeyEvent, KeyKind};
use crate::{
    machine_picker::{SIDEBAR_BINDINGS, SidebarCommand},
    tui_hit_map::chrome::TabTarget,
};

/// One painted row of the list: a Tab, or a group's header.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Row {
    pub(crate) target: TabTarget,
    /// A header's group is collapsed, so its members have no rows.
    pub(crate) collapsed: bool,
    /// A member Tab's header row.
    pub(crate) header: Option<usize>,
}

/// The rows one frame painted and the window it showed of them.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Paint {
    pub(crate) rows: Vec<Row>,
    /// The current Tab's row, where navigation starts before a key moved the focus.
    pub(crate) current: Option<usize>,
    pub(crate) first: usize,
    pub(crate) visible: usize,
}

impl Paint {
    fn index(&self, target: &TabTarget) -> Option<usize> {
        self.rows.iter().position(|row| row.target == *target)
    }

    /// The first row that keeps `index` in view, moving the window as little as possible.
    fn reveal(&self, index: usize) -> usize {
        let visible = self.visible.max(1);
        if index < self.first {
            index
        } else if index >= self.first + visible {
            index + 1 - visible
        } else {
            self.first
        }
    }

    fn last_first(&self) -> usize {
        self.rows.len().saturating_sub(self.visible.max(1))
    }
}

/// What a key asks of the list; the dispatcher applies it through the Tab strip's own actions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Intent {
    /// Consumed without an effect beyond the list's own focus or window.
    None,
    Close,
    /// Selects a Tab and closes the list, or toggles a header's group.
    Activate(TabTarget),
    OpenMenu(TabTarget),
    Move {
        tab: orchbee_workbench_protocol::WorkbenchTabId,
        previous: bool,
    },
    NewTab,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct State {
    /// The row a key moved to. None until one does, so the list opens showing only the current
    /// Tab's fill.
    focus: Option<TabTarget>,
    /// The first row the person scrolled or navigated to. None keeps the current Tab in view.
    first: Option<usize>,
    /// The rows the last painted frame showed, so every key of one burst resolves while the frame
    /// after the first is still pending.
    rows: Paint,
}

impl State {
    pub(crate) const fn focus(&self) -> Option<&TabTarget> {
        self.focus.as_ref()
    }

    pub(crate) const fn first(&self) -> Option<usize> {
        self.first
    }

    fn observe(&mut self, painted: Option<&Paint>) {
        if let Some(painted) = painted {
            self.rows.clone_from(painted);
        }
    }

    fn show_from(&mut self, first: usize) {
        self.first = Some(first);
        self.rows.first = first;
    }

    /// Moves the window by `lines` rows within the painted rows; false when it cannot move.
    pub(crate) fn scroll(&mut self, painted: &Paint, up: bool, lines: usize) -> bool {
        self.observe(Some(painted));
        let paint = &self.rows;
        let next = if up {
            paint.first.saturating_sub(lines)
        } else {
            paint.first.saturating_add(lines).min(paint.last_first())
        };
        if next == paint.first {
            return false;
        }
        self.show_from(next);
        true
    }

    fn focus_index(&mut self, index: usize) {
        if let Some(row) = self.rows.rows.get(index) {
            self.focus = Some(row.target.clone());
            let first = self.rows.reveal(index);
            self.show_from(first);
        }
    }

    /// Resolves one key against the rows the last frame painted, `painted` when that frame is
    /// still current. The list uses the sidebar list's navigation keys; keys it has no meaning
    /// for are consumed, since the list covers the terminal.
    pub(crate) fn key(&mut self, event: KeyEvent, painted: Option<&Paint>) -> Intent {
        if event.kind == KeyKind::Release {
            return Intent::None;
        }
        let Some(command) = SIDEBAR_BINDINGS
            .iter()
            .find(|(binding, _)| binding.matches(event))
            .map(|(_, command)| *command)
        else {
            return Intent::None;
        };
        self.observe(painted);
        let at = self
            .focus
            .as_ref()
            .and_then(|focus| self.rows.index(focus))
            .or(self.rows.current);
        let last = self.rows.rows.len().checked_sub(1);
        let row = at.and_then(|index| self.rows.rows.get(index)).cloned();
        match command {
            SidebarCommand::Exit => Intent::Close,
            SidebarCommand::NewWorkspace => Intent::NewTab,
            SidebarCommand::Previous => {
                if let Some(index) = at.map_or(last, |index| index.checked_sub(1)) {
                    self.focus_index(index);
                }
                Intent::None
            }
            SidebarCommand::Next => {
                let next = at.map_or(0, |index| index + 1);
                if last.is_some_and(|last| next <= last) {
                    self.focus_index(next);
                }
                Intent::None
            }
            SidebarCommand::First => {
                if last.is_some() {
                    self.focus_index(0);
                }
                Intent::None
            }
            SidebarCommand::Last => {
                if let Some(last) = last {
                    self.focus_index(last);
                }
                Intent::None
            }
            SidebarCommand::Activate => {
                row.map_or(Intent::None, |row| Intent::Activate(row.target))
            }
            SidebarCommand::Expand => match row {
                Some(Row {
                    target: target @ TabTarget::Group { .. },
                    collapsed: true,
                    ..
                }) => Intent::Activate(target),
                _ => Intent::None,
            },
            SidebarCommand::Collapse => match row {
                Some(Row {
                    target: target @ TabTarget::Group { .. },
                    collapsed: false,
                    ..
                }) => Intent::Activate(target),
                // A member steps out to its header, as an Agent row does to its Workspace.
                Some(Row {
                    header: Some(header),
                    ..
                }) => {
                    self.focus_index(header);
                    Intent::None
                }
                _ => Intent::None,
            },
            SidebarCommand::OpenMenu => {
                row.map_or(Intent::None, |row| Intent::OpenMenu(row.target))
            }
            SidebarCommand::MoveUp | SidebarCommand::MoveDown => match row {
                Some(Row {
                    target: TabTarget::Tab(tab),
                    ..
                }) => Intent::Move {
                    tab,
                    previous: command == SidebarCommand::MoveUp,
                },
                _ => Intent::None,
            },
            SidebarCommand::Search | SidebarCommand::Help => Intent::None,
        }
    }
}
