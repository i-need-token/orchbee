//! A Workspace's Change machine: the list it opens and what a move says. The list shows This
//! computer, then the saved machines in bands, usable ones with room first, then usable ones low on
//! something, ones that need a login, and ones that cannot take a Workspace now; inside a band by
//! name in natural order. Its order is taken once, when it opens. Drawing-free: the action menu
//! lays the rows out, the machine manager runs the move, and `localization` words both.
//! `apps/orchbee/docs/machines.md#change-machine` owns the rules.

// module-layer: values

use crate::machine_settings::{Machine, State, natural_order};
use orchbee_machine_protocol::capacity::Capacity;

/// What the list is built from, as the machine manager hands it over.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Facts {
    /// The saved machine this Client's Session runs on; `None` for this computer.
    pub here: Option<String>,
    /// What this computer has free; `None` until read.
    pub local: Option<Capacity>,
    /// Every saved machine.
    pub machines: Vec<Machine>,
    /// A move is under way, so no other starts until it ends.
    pub moving: bool,
}

/// How a machine can take a Workspace now, in the order the list shows the bands.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Band {
    Room,
    /// Connected, with an amber number.
    Low,
    /// The move asks for the login where Orchbee asks for one.
    Login,
    /// Not connected, connecting, of another version, or stopping its terminals.
    Unusable,
}

impl Band {
    fn of(machine: &Machine) -> Self {
        if machine.stopping {
            return Self::Unusable;
        }
        match machine.state {
            State::Online
                if machine.capacity.as_ref().is_some_and(|capacity| {
                    crate::machine_capacity::parts(capacity)
                        .iter()
                        .any(|part| part.low)
                }) =>
            {
                Self::Low
            }
            State::Online => Self::Room,
            State::NeedsLogin => Self::Login,
            State::Connecting
            | State::Reconnecting
            | State::Offline
            | State::Disabled
            | State::NeedsUpdate
            | State::LocalNeedsUpdate
            | State::Other(_) => Self::Unusable,
        }
    }
}

/// One row of the list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Row {
    /// The saved machine; `None` for this computer.
    pub machine: Option<Machine>,
    /// What this computer has free, on its own row.
    pub local: Option<Capacity>,
    /// The Workspace runs there now.
    pub current: bool,
    pub band: Band,
}

impl Row {
    /// The saved machine's identity; `None` for this computer.
    #[must_use]
    pub fn id(&self) -> Option<&str> {
        self.machine.as_ref().map(|machine| machine.id.as_str())
    }

    /// What it has free, for a row that shows numbers rather than its state word.
    #[must_use]
    pub fn capacity(&self) -> Option<&Capacity> {
        match &self.machine {
            None => self.local.as_ref(),
            Some(machine) => machine.capacity.as_ref(),
        }
    }

    /// Whether `query` names it: any part of its name, or of a saved machine's address, without
    /// regard to case. `here` is this computer's name in the interface language.
    #[must_use]
    pub fn matches(&self, query: &str, here: &str) -> bool {
        match &self.machine {
            Some(machine) => machine.matches(query),
            None => {
                let query = query.trim().to_lowercase();
                query.is_empty() || here.to_lowercase().contains(&query)
            }
        }
    }

    /// Takes the facts of `newer` that change while the list is open, keeping its place.
    pub fn refresh(&mut self, facts: &Facts) {
        match &mut self.machine {
            None => self.local.clone_from(&facts.local),
            Some(machine) => {
                if let Some(newer) = facts.machines.iter().find(|newer| newer.id == machine.id) {
                    machine.clone_from(newer);
                    self.band = Band::of(machine);
                } else {
                    // A machine removed while the list is open cannot take a Workspace.
                    self.band = Band::Unusable;
                }
            }
        }
    }
}

/// The list: This computer, then every saved machine by band and name.
#[must_use]
pub fn rows(facts: &Facts) -> Vec<Row> {
    let mut machines: Vec<Row> = facts
        .machines
        .iter()
        .map(|machine| Row {
            machine: Some(machine.clone()),
            local: None,
            current: facts.here.as_deref() == Some(machine.id.as_str()),
            band: Band::of(machine),
        })
        .collect();
    machines.sort_by(|left, right| {
        let (Some(a), Some(b)) = (&left.machine, &right.machine) else {
            return std::cmp::Ordering::Equal;
        };
        left.band
            .cmp(&right.band)
            .then_with(|| natural_order(&a.label, &b.label))
            .then_with(|| a.id.cmp(&b.id))
    });
    let mut rows = vec![Row {
        machine: None,
        local: facts.local.clone(),
        current: facts.here.is_none(),
        band: Band::Room,
    }];
    rows.extend(machines);
    rows
}

/// The move a choice in the list asks for.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Request {
    /// The Workspace, by its identity in the Session it runs in.
    pub workspace: String,
    pub label: String,
    /// Where it goes: a saved machine's identity, `None` for this computer.
    pub to: Option<String>,
    /// That machine's name; empty for this computer.
    pub to_label: String,
}

/// What a move asks before it stops something.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Question {
    /// The Workspace's name, as its sidebar row shows it.
    pub workspace: String,
    /// Where it goes: that machine's name, empty for this computer.
    pub to: String,
    pub stops: Stops,
}

/// What a move would stop, as the machine that holds the Workspace read it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Stops {
    /// `count` terminals may run a program; `names` names those it can, in order.
    Busy { count: usize, names: Vec<Name> },
    /// What runs in them could not be read.
    Unread,
}

/// How a terminal that may run a program is named, as `workspace set-machine` names it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Name {
    /// The Agent it runs.
    Agent(String),
    /// Its title, which names it only when it names a program Orchbee knows.
    Title(String),
}

/// A move that did not just land where it was asked to, for the status line.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Notice {
    /// It landed in the home folder of `machine` (empty for this computer), which has no `folder`.
    FolderMissing { machine: String, folder: String },
    /// `machine` is not connected; nothing changed.
    NotConnected { machine: String },
    /// A Workspace in a linked worktree stays with its repository; nothing changed.
    Worktree,
    /// It runs on `machine` now too, and the old one could not be closed.
    CloseFailed { machine: String },
    /// Nothing changed.
    Failed,
    /// No answer said how it ended.
    Unknown,
}
