//! The Start Agent picker: one nested task inside the Agent board's modal owner.
//!
//! The picker lists every startable provider in groups — recent, installed, then one folded
//! row for the providers this computer did not find — filters them as the query changes, and
//! hands the chosen provider and its place to the composition. It never starts anything itself
//! and keeps no record: recency comes from [`super::session_agents`], installation from the
//! start state's catalog.

use orchbee_workbench_protocol::WorkbenchPanelId;

use super::session_agents::{RECENT_LIMIT, SessionAgents, recent};
use super::start::StartPlace;
use super::{AgentBoard, AgentBoardControl, AgentBoardControlTarget};
use crate::client_view::{Binding, KeyCode, KeyEvent, KeyKind, Modifiers};
use crate::terminal_text::line_editor::{LengthLimit, LineEditor, LineKey};
use crate::workbench_state::AgentStartNotice;

mod place;
pub(crate) use place::Places;

#[cfg(test)]
mod tests;

const MAXIMUM_QUERY_BYTES: usize = 64;

/// One listed group, in display order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Group {
    Recent,
    Installed,
    NotFound,
    /// A remote Session: this computer cannot say what the target terminal has.
    NotChecked,
}

/// What a provider row shows on its right.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Meta {
    /// The line a start types, such as `openclaw chat`.
    Command,
    /// This computer, or an earlier start, found no command.
    Missing,
    /// The command is generic, so finding it would not prove the provider is installed.
    Generic,
}

/// One list row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Item {
    Heading(Group),
    Provider {
        id: &'static str,
        group: Group,
        meta: Meta,
    },
    /// The folded Not found group and how many providers it holds.
    Fold(usize),
}

impl Item {
    const fn selectable(self) -> bool {
        !matches!(self, Self::Heading(_))
    }

    const fn target(self) -> Option<Target> {
        match self {
            Self::Heading(_) => None,
            Self::Provider { id, .. } => Some(Target::Provider(id)),
            Self::Fold(_) => Some(Target::Fold),
        }
    }
}

/// The cursor's identity, so it follows its row while the list changes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Target {
    Provider(&'static str),
    Fold,
}

/// What the picker lists from.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Sources<'a> {
    /// Recent providers, most recent first.
    pub(crate) recent: &'a [&'static str],
    /// The providers this computer found, or `None` when it cannot check the target terminal.
    pub(crate) installed: Option<&'a [&'static str]>,
    /// Providers whose start found no command.
    pub(crate) missing: &'a [&'static str],
}

/// Whether `id` matches the query by its product name, its command line or its id.
fn matches(id: &'static str, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let query = query.to_lowercase();
    let product = crate::agent_provider::product_name(id).to_lowercase();
    let line = crate::agent_provider::launch_line(id)
        .unwrap_or_default()
        .to_lowercase();
    product.contains(&query) || line.contains(&query) || id.contains(&query)
}

fn by_product(ids: &mut [&'static str]) {
    ids.sort_by_key(|id| crate::agent_provider::product_name(id).to_lowercase());
}

/// The picker's rows: Recent (at most three, still available, most recent first), then the
/// rest. Locally the rest is Installed A–Z and a Not found group that stays folded into one row
/// until it is opened or a query searches it; for a remote Session it is one Not checked group.
/// A provider whose start found no command is listed as not found in both.
pub(crate) fn items(sources: Sources<'_>, query: &str, expanded: bool) -> Vec<Item> {
    let query = query.trim();
    let all: Vec<_> = crate::agent_provider::choices().collect();
    let missing = |id: &str| sources.missing.contains(&id);
    let available = |id: &str| {
        !missing(id)
            && sources
                .installed
                .is_none_or(|installed| installed.contains(&id))
    };
    let recent: Vec<&'static str> = sources
        .recent
        .iter()
        .copied()
        .filter(|id| all.iter().any(|choice| choice.id == *id) && available(id))
        .take(RECENT_LIMIT)
        .collect();
    let mut rest: Vec<&'static str> = all
        .iter()
        .map(|choice| choice.id)
        .filter(|id| available(id) && !recent.contains(id))
        .collect();
    by_product(&mut rest);
    let mut unavailable: Vec<&'static str> = all
        .iter()
        .map(|choice| choice.id)
        .filter(|id| !available(id))
        .collect();
    by_product(&mut unavailable);

    let mut rows = Vec::new();
    let group = |rows: &mut Vec<Item>, group: Group, ids: &[&'static str]| {
        let mut heading = false;
        for id in ids.iter().copied().filter(|id| matches(id, query)) {
            if !heading {
                rows.push(Item::Heading(group));
                heading = true;
            }
            let meta = if missing(id) || group == Group::NotFound && sources.installed.is_some() {
                let generic = all
                    .iter()
                    .any(|choice| choice.id == id && !choice.checkable);
                if generic && !missing(id) {
                    Meta::Generic
                } else {
                    Meta::Missing
                }
            } else {
                Meta::Command
            };
            rows.push(Item::Provider { id, group, meta });
        }
    };
    group(&mut rows, Group::Recent, &recent);
    if sources.installed.is_some() {
        group(&mut rows, Group::Installed, &rest);
        if query.is_empty() && !expanded {
            if !unavailable.is_empty() {
                rows.push(Item::Fold(unavailable.len()));
            }
        } else {
            group(&mut rows, Group::NotFound, &unavailable);
        }
    } else {
        group(&mut rows, Group::NotChecked, &rest);
        group(&mut rows, Group::NotFound, &unavailable);
    }
    rows
}

/// What Esc does in the picker: it clears a query first, then returns to the board the
/// picker was opened from, otherwise closes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Escape {
    ClearQuery,
    BackToBoard,
    Close,
}

pub(crate) const fn escape(query_empty: bool, from_board: bool) -> Escape {
    if !query_empty {
        Escape::ClearQuery
    } else if from_board {
        Escape::BackToBoard
    } else {
        Escape::Close
    }
}

/// A start the picker accepted; the composition runs it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PickedStart {
    pub(crate) provider: &'static str,
    pub(crate) place: StartPlace,
}

pub(crate) struct Picker {
    query: LineEditor,
    /// `None` rests on the first selectable row: the most recent provider, else the first
    /// installed one.
    cursor: Option<Target>,
    expanded: bool,
    scroll: usize,
    viewport: usize,
    from_board: bool,
    places: Places,
    /// Holds while the picker stays open; a query change never resets it.
    place: StartPlace,
    revision: u64,
}

impl Picker {
    fn new(from_board: bool, places: Places) -> Self {
        Self {
            query: LineEditor::new(String::new(), LengthLimit::Bytes(MAXIMUM_QUERY_BYTES)),
            cursor: None,
            expanded: false,
            scroll: 0,
            viewport: 0,
            from_board,
            place: places.automatic(),
            places,
            revision: 0,
        }
    }

    fn cycle_place(&mut self, forward: bool) {
        self.place = self.places.next(&self.place, forward);
        self.touch();
    }

    fn touch(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    pub(crate) fn query(&self) -> &str {
        self.query.text()
    }

    pub(crate) fn query_cursor(&self) -> usize {
        self.query.cursor()
    }

    pub(crate) const fn place(&self) -> &StartPlace {
        &self.place
    }

    pub(crate) const fn scroll(&self) -> usize {
        self.scroll
    }

    /// The cursor's row index in `items`.
    pub(crate) fn cursor_index(&self, items: &[Item]) -> Option<usize> {
        self.cursor
            .and_then(|target| items.iter().position(|item| item.target() == Some(target)))
            .or_else(|| items.iter().position(|item| item.selectable()))
    }

    fn reveal(&mut self, items: &[Item]) {
        let visible = self.viewport.max(1);
        if let Some(index) = self.cursor_index(items) {
            if index < self.scroll {
                self.scroll = index;
            } else if index >= self.scroll + visible {
                self.scroll = index + 1 - visible;
            }
        }
        // A heading directly above the cursor scrolls in with it.
        if let Some(index) = self.cursor_index(items)
            && index > 0
            && index - 1 < self.scroll
            && matches!(items.get(index - 1), Some(Item::Heading(_)))
            && index < self.scroll + visible
        {
            self.scroll = index - 1;
        }
        self.clamp(items.len());
    }

    fn clamp(&mut self, len: usize) {
        self.scroll = self.scroll.min(len.saturating_sub(self.viewport.max(1)));
    }

    fn move_cursor(&mut self, items: &[Item], delta: isize) {
        let selectable: Vec<usize> = (0..items.len())
            .filter(|index| items[*index].selectable())
            .collect();
        let Some(last) = selectable.len().checked_sub(1) else {
            return;
        };
        let current = self
            .cursor_index(items)
            .and_then(|index| selectable.iter().position(|candidate| *candidate == index))
            .unwrap_or(0);
        let next = current.saturating_add_signed(delta).min(last);
        self.cursor = items[selectable[next]].target();
        self.touch();
        self.reveal(items);
    }

    fn edit(&mut self, event: KeyEvent) -> bool {
        let changed = if let Some(key) = LineKey::for_key(event) {
            self.query.apply(key) == Ok(true)
        } else {
            match (event.code, event.mods) {
                (KeyCode::Char(character), modifiers)
                    if !modifiers.ctrl
                        && !modifiers.alt
                        && !modifiers.super_
                        && !character.is_control() =>
                {
                    self.query.insert(&character.to_string()).is_ok()
                }
                (KeyCode::Space, Modifiers::NONE) => self.query.insert(" ").is_ok(),
                (KeyCode::Backspace, Modifiers::NONE) => self.query.backspace(),
                (KeyCode::Delete, Modifiers::NONE) => self.query.delete(),
                (KeyCode::Left, Modifiers::NONE) => self.query.left(),
                (KeyCode::Right, Modifiers::NONE) => self.query.right(),
                (KeyCode::Home, Modifiers::NONE) => self.query.home(),
                (KeyCode::End, Modifiers::NONE) => self.query.end(),
                _ => false,
            }
        };
        if changed {
            // A changed query rests on its first match.
            self.cursor = None;
            self.scroll = 0;
            self.touch();
        }
        changed
    }
}

/// The picker's part of a Board draw: any change to what it lists or where its cursor rests.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DrawSource {
    revision: u64,
    start: u64,
    agents: u64,
}

/// What a key or click inside the picker did.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PickerOutcome {
    Handled,
    Start(PickedStart),
}

impl AgentBoard {
    pub(crate) const fn picker(&self) -> Option<&Picker> {
        self.picker.as_ref()
    }

    pub(super) fn picker_draw_source(&self) -> Option<DrawSource> {
        self.picker.as_ref().map(|picker| DrawSource {
            revision: picker.revision,
            start: self.start.revision(),
            agents: self.agents_revision,
        })
    }

    /// The Session's Agents as last observed; a change redraws an open picker.
    pub(crate) fn observe_session_agents(&mut self, agents: SessionAgents) {
        if self.session_agents != agents {
            self.session_agents = agents;
            self.agents_revision = self.agents_revision.wrapping_add(1);
        }
    }

    pub(crate) fn picker_sources(&self) -> (Vec<&'static str>, Option<Vec<&'static str>>) {
        (
            recent(self.start.started(), &self.session_agents),
            self.start.installed().map(<[_]>::to_vec),
        )
    }

    /// The picker's current rows.
    pub(crate) fn picker_items(&self) -> Vec<Item> {
        let Some(picker) = self.picker.as_ref() else {
            return Vec::new();
        };
        let (recent, installed) = self.picker_sources();
        items(
            Sources {
                recent: &recent,
                installed: installed.as_deref(),
                missing: self.start.missing(),
            },
            picker.query(),
            picker.expanded,
        )
    }

    pub(crate) const fn can_start_agent(&self) -> bool {
        self.start.can_start()
    }

    fn places(&self, focus: Option<&WorkbenchPanelId>) -> Places {
        Places::new(focus, |panel| {
            self.session_agents.holds(panel) || self.start.guards(panel)
        })
    }

    /// `Ctrl+B G`: opens the Board's owner straight into the picker, without reading the
    /// Board's rows.
    pub(crate) fn open_picker(
        &mut self,
        path: Option<std::ffi::OsString>,
        focus: Option<&WorkbenchPanelId>,
    ) {
        self.open();
        self.intent = None;
        self.start_picker(path, focus, false);
    }

    /// The Board's Start Agent row or `s`: the picker replaces the Board's content until Esc.
    pub(crate) fn enter_picker(
        &mut self,
        path: Option<std::ffi::OsString>,
        focus: Option<&WorkbenchPanelId>,
    ) -> bool {
        if !self.open || self.picker.is_some() || !self.offers_start() {
            return false;
        }
        self.start_picker(path, focus, true);
        true
    }

    fn start_picker(
        &mut self,
        path: Option<std::ffi::OsString>,
        focus: Option<&WorkbenchPanelId>,
        from_board: bool,
    ) {
        let places = self.places(focus);
        self.picker = Some(Picker::new(from_board, places));
        self.start_catalog_path = path;
        self.start.refresh_catalog();
        self.shown = None;
    }

    fn leave_picker(&mut self) {
        self.picker = None;
        self.start_catalog_path = None;
        self.shown = None;
    }

    /// Esc inside the picker. Returns whether the whole owner closed.
    pub(crate) fn picker_escape(&mut self) -> bool {
        let Some(picker) = self.picker.as_mut() else {
            return false;
        };
        match escape(picker.query().is_empty(), picker.from_board) {
            Escape::ClearQuery => {
                picker.query.clear();
                picker.cursor = None;
                picker.scroll = 0;
                picker.touch();
                self.shown = None;
                false
            }
            Escape::BackToBoard => {
                self.leave_picker();
                false
            }
            Escape::Close => {
                self.close();
                true
            }
        }
    }

    /// One key the picker owns. Esc is the caller's, so its order stays in one place.
    pub(crate) fn picker_key(&mut self, event: KeyEvent) -> PickerOutcome {
        if event.kind == KeyKind::Release || self.picker.is_none() {
            return PickerOutcome::Handled;
        }
        let items = self.picker_items();
        let Some(picker) = self.picker.as_mut() else {
            return PickerOutcome::Handled;
        };
        let page = isize::try_from(picker.viewport.max(1)).unwrap_or(1);
        match (event.code, event.mods) {
            // A terminal sends Enter as a bare CR, which decodes to its canonical `Ctrl+M`.
            _ if Binding::new(KeyCode::Enter, Modifiers::NONE).matches(event) => {
                let target = picker
                    .cursor_index(&items)
                    .and_then(|index| items.get(index).copied());
                return self.activate(target);
            }
            // A terminal sends Tab as a bare HT and Shift+Tab as `CSI Z`.
            _ if Binding::new(KeyCode::Tab, Modifiers::NONE).matches(event) => {
                picker.cycle_place(true);
            }
            _ if Binding::new(KeyCode::Tab, Modifiers::NONE.with_shift()).matches(event) => {
                picker.cycle_place(false);
            }
            (KeyCode::Up, Modifiers::NONE) => picker.move_cursor(&items, -1),
            (KeyCode::Down, Modifiers::NONE) => picker.move_cursor(&items, 1),
            (KeyCode::PageUp, Modifiers::NONE) => picker.move_cursor(&items, -page),
            (KeyCode::PageDown, Modifiers::NONE) => picker.move_cursor(&items, page),
            _ => {
                picker.edit(event);
            }
        }
        self.shown = None;
        PickerOutcome::Handled
    }

    /// Whether `target` was painted by the current interactive draw.
    pub(crate) fn drawn(&self, target: &AgentBoardControlTarget) -> bool {
        self.interactive()
            && self
                .current_draw()
                .is_some_and(|draw| draw.token() == target.token())
    }

    /// A press released on one painted picker control, against the exact draw that painted it.
    pub(crate) fn picker_control(&mut self, target: &AgentBoardControlTarget) -> PickerOutcome {
        if !self.drawn(target) {
            return PickerOutcome::Handled;
        }
        match target.control() {
            AgentBoardControl::Pick(index) => {
                let item = self.picker_items().get(index).copied();
                self.activate(item)
            }
            AgentBoardControl::Place => {
                if let Some(picker) = self.picker.as_mut() {
                    picker.cycle_place(true);
                }
                self.shown = None;
                PickerOutcome::Handled
            }
            AgentBoardControl::Close => {
                // The close control leaves the picker the way Esc does with an empty query.
                if self.picker.as_ref().is_some_and(|picker| picker.from_board) {
                    self.leave_picker();
                } else {
                    self.close();
                }
                PickerOutcome::Handled
            }
            _ => PickerOutcome::Handled,
        }
    }

    fn activate(&mut self, item: Option<Item>) -> PickerOutcome {
        match item {
            Some(Item::Provider { id, .. }) if self.start.can_start() => {
                let Some(picker) = self.picker.as_ref() else {
                    return PickerOutcome::Handled;
                };
                // The place shown is the place used, unless one of this Client's own starts
                // claimed the Panel meanwhile.
                let place = match &picker.place {
                    StartPlace::ThisPanel(panel) if self.start.guards(panel) => StartPlace::NewTab,
                    place => place.clone(),
                };
                self.close();
                PickerOutcome::Start(PickedStart {
                    provider: id,
                    place,
                })
            }
            Some(Item::Provider { .. }) => {
                // An unsupported host says why in the status row; any other refusal says it here.
                if self.start.launch_supported() {
                    self.start.set_notice(AgentStartNotice::Failed);
                }
                PickerOutcome::Handled
            }
            Some(Item::Fold(_)) => {
                if let Some(picker) = self.picker.as_mut() {
                    picker.expanded = true;
                    picker.cursor = None;
                    picker.touch();
                    let items = self.picker_items();
                    if let Some(picker) = self.picker.as_mut() {
                        // The cursor moves onto the first provider the fold held.
                        picker.cursor = items
                            .iter()
                            .find(|item| {
                                matches!(
                                    item,
                                    Item::Provider {
                                        group: Group::NotFound,
                                        ..
                                    }
                                )
                            })
                            .and_then(|item| item.target());
                        // The group the fold held closes the list: show all of it that fits,
                        // then keep the cursor in view.
                        picker.scroll = items.len().saturating_sub(picker.viewport.max(1));
                        picker.reveal(&items);
                    }
                }
                self.shown = None;
                PickerOutcome::Handled
            }
            _ => PickerOutcome::Handled,
        }
    }

    pub(super) fn picker_scroll_by(&mut self, delta: isize) {
        let len = self.picker_items().len();
        if let Some(picker) = self.picker.as_mut() {
            picker.scroll = picker.scroll.saturating_add_signed(delta);
            picker.clamp(len);
            picker.touch();
        }
        self.shown = None;
    }

    /// The first row a picker draw of `visible` rows shows.
    pub(super) fn picker_first_row(&self, visible: usize) -> usize {
        let len = self.picker_items().len();
        self.picker
            .as_ref()
            .map_or(0, |picker| picker.scroll.min(len.saturating_sub(visible)))
    }

    pub(super) fn settle_picker_viewport(&mut self, visible: usize, first: usize) {
        if let Some(picker) = self.picker.as_mut() {
            picker.viewport = visible;
            picker.scroll = first;
        }
    }
}
