//! Client-local navigator focus and exact painted-row identity. No I/O or selection effects.
use orchbee_workbench_protocol::{SiteId, WorkbenchRevision, WorkbenchTabId};

use super::{Binding, KeyCode, KeyEvent, KeyKind, Modifiers};
use crate::terminal_session::NativeContextLocation;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Source {
    pub(crate) site: SiteId,
    pub(crate) revision: WorkbenchRevision,
    pub(crate) location: NativeContextLocation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Target {
    Location(NativeContextLocation),
    Tab(WorkbenchTabId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Catalog {
    pub(crate) source: Source,
    pub(crate) locations: Vec<NativeContextLocation>,
    pub(crate) tabs: Vec<WorkbenchTabId>,
    pub(crate) current_tab: Option<WorkbenchTabId>,
}

impl Catalog {
    pub(crate) fn contains(&self, target: &Target) -> bool {
        match target {
            Target::Location(location) => self.locations.contains(location),
            Target::Tab(tab) => self.tabs.contains(tab),
        }
    }

    fn initial(&self, tabs: bool) -> Option<Target> {
        if tabs {
            self.current_tab
                .as_ref()
                .filter(|id| self.tabs.contains(id))
                .cloned()
                .map(Target::Tab)
        } else {
            self.locations
                .iter()
                .find(|location| *location == &self.source.location)
                .cloned()
                .map(Target::Location)
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Row {
    pub(crate) episode: u64,
    pub(crate) source: Source,
    pub(crate) target: Target,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Paint {
    pub(crate) rows: Vec<Row>,
    pub(crate) focus: Option<Target>,
    pub(crate) page_rows: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Admission {
    ExactCurrent,
    Admitted(u64),
    Refused,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Reconcile {
    Unchanged,
    Changed,
    Cancel,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Command {
    Previous,
    Next,
    First,
    Last,
    PagePrevious,
    PageNext,
    Locations,
    Tabs,
    Column,
    Open,
    Exit,
}

pub(crate) const BINDINGS: &[(Binding, Command)] = &[
    (
        Binding::new(KeyCode::Up, Modifiers::NONE),
        Command::Previous,
    ),
    (Binding::new(KeyCode::Down, Modifiers::NONE), Command::Next),
    (Binding::new(KeyCode::Home, Modifiers::NONE), Command::First),
    (Binding::new(KeyCode::End, Modifiers::NONE), Command::Last),
    (
        Binding::new(KeyCode::PageUp, Modifiers::NONE),
        Command::PagePrevious,
    ),
    (
        Binding::new(KeyCode::PageDown, Modifiers::NONE),
        Command::PageNext,
    ),
    (
        Binding::new(KeyCode::Left, Modifiers::NONE),
        Command::Locations,
    ),
    (Binding::new(KeyCode::Right, Modifiers::NONE), Command::Tabs),
    (Binding::new(KeyCode::Tab, Modifiers::NONE), Command::Column),
    (
        Binding::new(KeyCode::Tab, Modifiers::NONE.with_shift()),
        Command::Column,
    ),
    (Binding::new(KeyCode::Enter, Modifiers::NONE), Command::Open),
    (Binding::new(KeyCode::Esc, Modifiers::NONE), Command::Exit),
];

pub(crate) fn command(key: KeyEvent) -> Option<Command> {
    (key.kind != KeyKind::Release)
        .then(|| {
            BINDINGS
                .iter()
                .find(|(binding, _)| binding.matches(key))
                .map(|(_, command)| *command)
        })
        .flatten()
}

pub(crate) fn label(command: Command) -> String {
    super::format_binding(
        &BINDINGS
            .iter()
            .find(|(_, value)| *value == command)
            .expect("navigator command has a binding")
            .0,
        super::LabelStyle::for_host(),
    )
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Navigator {
    episode: u64,
    active: bool,
    pub(crate) catalog: Option<Catalog>,
    pub(crate) focus: Option<Target>,
    pub(crate) prefix: bool,
}

impl Navigator {
    pub(crate) fn episode(&self) -> u64 {
        self.episode
    }

    pub(crate) fn scroll_row(&self, tabs: bool) -> Option<Row> {
        let focus = self
            .focus
            .as_ref()
            .filter(|target| matches!(target, Target::Tab(_)) == tabs)
            .cloned();
        self.row(focus.or_else(|| self.catalog.as_ref()?.initial(tabs))?)
    }

    pub(crate) fn scroll(&mut self, row: &Row, up: bool, lines: usize) -> Option<Row> {
        if !self.accepts(row, false) {
            return None;
        }
        let tabs = matches!(row.target, Target::Tab(_));
        if self
            .focus
            .as_ref()
            .is_none_or(|focus| matches!(focus, Target::Tab(_)) != tabs)
        {
            self.move_focus(
                if tabs {
                    Command::Tabs
                } else {
                    Command::Locations
                },
                1,
            );
        }
        self.move_focus(
            if up {
                Command::PagePrevious
            } else {
                Command::PageNext
            },
            lines.saturating_add(1),
        );
        self.scroll_row(tabs)
    }
    pub(crate) fn open(&mut self) -> bool {
        let Some(episode) = self.episode.checked_add(1) else {
            return false;
        };
        *self = Self {
            episode,
            active: true,
            ..Self::default()
        };
        true
    }

    pub(crate) fn close(&mut self) {
        *self = Self {
            episode: self.episode,
            ..Self::default()
        };
    }

    pub(crate) fn reconcile(&mut self, catalog: Option<Catalog>) -> Reconcile {
        if !self.active || self.catalog == catalog {
            return Reconcile::Unchanged;
        }
        if let Some(old) = &self.catalog {
            let Some(new) = &catalog else {
                return Reconcile::Cancel;
            };
            if old.source.site != new.source.site
                || old.source.location != new.source.location
                || self
                    .focus
                    .as_ref()
                    .is_some_and(|target| !new.contains(target))
            {
                return Reconcile::Cancel;
            }
        }
        if self.focus.is_none() {
            self.focus = catalog
                .as_ref()
                .and_then(|rows| rows.initial(true).or_else(|| rows.initial(false)));
        }
        self.catalog = catalog;
        Reconcile::Changed
    }

    pub(crate) fn row(&self, target: Target) -> Option<Row> {
        let catalog = self.catalog.as_ref()?;
        (self.active && catalog.contains(&target)).then(|| Row {
            episode: self.episode,
            source: catalog.source.clone(),
            target,
        })
    }

    pub(crate) fn accepts(&self, row: &Row, keyboard: bool) -> bool {
        self.active
            && self.episode == row.episode
            && self.catalog.as_ref().is_some_and(|catalog| {
                catalog.source == row.source && catalog.contains(&row.target)
            })
            && (!keyboard || self.focus.as_ref() == Some(&row.target))
    }

    pub(crate) fn move_focus(&mut self, command: Command, page_rows: usize) -> bool {
        let Some(catalog) = &self.catalog else {
            return false;
        };
        let tabs = matches!(self.focus, Some(Target::Tab(_)));
        let column = match command {
            Command::Locations => Some(false),
            Command::Tabs => Some(true),
            Command::Column => Some(!tabs),
            _ => None,
        };
        let next = if let Some(tabs) = column {
            catalog.initial(tabs)
        } else {
            let rows: Vec<_> = if tabs {
                catalog.tabs.iter().cloned().map(Target::Tab).collect()
            } else {
                catalog
                    .locations
                    .iter()
                    .cloned()
                    .map(Target::Location)
                    .collect()
            };
            let Some(index) = self
                .focus
                .as_ref()
                .and_then(|focus| rows.iter().position(|row| row == focus))
            else {
                return false;
            };
            let page = page_rows.saturating_sub(1).max(1);
            let next = match command {
                Command::Previous => index.saturating_sub(1),
                Command::Next => index.saturating_add(1),
                Command::First => 0,
                Command::Last => rows.len().saturating_sub(1),
                Command::PagePrevious => index.saturating_sub(page),
                Command::PageNext => index.saturating_add(page),
                _ => return false,
            }
            .min(rows.len().saturating_sub(1));
            rows.get(next).cloned()
        };
        if next.is_none() || next == self.focus {
            return false;
        }
        self.focus = next;
        true
    }
}

#[cfg(test)]
mod tests;
