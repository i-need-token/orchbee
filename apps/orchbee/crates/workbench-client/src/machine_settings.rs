//! Settings › Machines as values: what the TUI knows of this computer and of each saved machine,
//! and the one request the person made on the page. The exchange of the TUI writes the facts and
//! carries out the requests; Settings shows the facts and hands back its request, so neither reads
//! the other's state. `apps/orchbee/docs/machines.md#machines-in-settings` owns the rules.

// module-layer: values

use crate::machine_status::{Explanation, Line};
use orchbee_machine_protocol::capacity::Capacity;
use std::cmp::Ordering;

/// More saved machines than this, and the list offers `Find a machine`.
pub const FILTER_THRESHOLD: usize = 8;

/// What Settings › Machines shows.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Facts {
    /// What this computer has free; `None` until the hub read it.
    pub local: Option<Capacity>,
    /// Every saved machine, by name in natural order.
    pub machines: Vec<Machine>,
    /// The system password store can be used, so a machine's page offers Forget.
    pub passwords: bool,
    /// How the person's last request ended, until the next one.
    pub outcome: Option<Outcome>,
    /// The machine the person just added from Settings, which the list focuses once.
    pub added: Option<String>,
}

/// One saved machine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Machine {
    pub id: String,
    pub label: String,
    /// What `ssh` connects to; `None` for a connection kind this build does not know.
    pub address: Option<String>,
    pub state: State,
    /// What it has free, while it is online and its Orchbee reported it.
    pub capacity: Option<Capacity>,
    /// What its Session holds while it is online: Workspaces and terminals.
    pub workspaces: Option<usize>,
    pub terminals: Option<usize>,
    /// Its working Agents, while their read is current.
    pub agents: Option<usize>,
    /// When the last connection attempt ended, in milliseconds since the Unix epoch.
    pub last_attempt: Option<u64>,
    /// Why it reads its state, as every interface explains it.
    pub explanation: Explanation,
    /// Its terminals are being stopped.
    pub stopping: bool,
}

impl Machine {
    /// Whether `query` names it: any part of its name or address, without regard to case.
    #[must_use]
    pub fn matches(&self, query: &str) -> bool {
        let query = query.trim().to_lowercase();
        query.is_empty()
            || self.label.to_lowercase().contains(&query)
            || self
                .address
                .as_ref()
                .is_some_and(|address| address.to_lowercase().contains(&query))
    }

    /// The one action its Status row offers now; none while a connection is being made or its
    /// terminals are being stopped.
    #[must_use]
    pub const fn action(&self) -> Option<Action> {
        if self.stopping {
            return None;
        }
        match self.state {
            State::Online => Some(Action::Disconnect),
            State::Disabled => Some(Action::Connect),
            State::Offline => Some(Action::Retry),
            State::Reconnecting => Some(Action::RetryNow),
            State::NeedsLogin => Some(Action::LogIn),
            State::Connecting => None,
            // It keeps trying in the background: the one thing to offer is to stop that.
            State::NeedsUpdate | State::LocalNeedsUpdate | State::Other(_) => {
                Some(Action::Disconnect)
            }
        }
    }

    /// Its Orchbee's version, as the hub last reported it.
    #[must_use]
    pub fn version(&self) -> Option<&str> {
        self.explanation.lines.iter().find_map(|line| match line {
            Line::Version(version) => Some(version.as_str()),
            _ => None,
        })
    }
}

/// A machine's state as the page words it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum State {
    Online,
    Connecting,
    Reconnecting,
    /// Orchbee connects to it, but it did not answer.
    Offline,
    /// Disconnected in Settings (the record's disabled state): nothing connects it until Connect.
    Disabled,
    NeedsLogin,
    /// Its Orchbee is too old for this computer's.
    NeedsUpdate,
    /// This computer's Orchbee is too old for its.
    LocalNeedsUpdate,
    /// Any other state, by the key of the machine list's word for it.
    Other(&'static str),
}

/// What a Status row's button does.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    /// Stops Orchbee from connecting to it, across restarts too: the record is disabled.
    Disconnect,
    /// Enables the record again, which connects it.
    Connect,
    Retry,
    RetryNow,
    LogIn,
}

/// What the person asked on the page.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Request {
    /// Opens Add Machine.
    Add,
    Rename {
        machine: String,
        label: String,
    },
    Act {
        machine: String,
        action: Action,
    },
    ForgetPassword(String),
    /// Stops the machine's terminals; its record stays.
    Stop(String),
    /// Removes the record without stopping anything.
    Remove(String),
}

/// How a request ended, for the page's status line.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Outcome {
    /// The record went; the terminals over there keep running.
    Removed(String),
    /// Its Session stopped.
    Stopped(String),
    /// The stop failed or no answer said how it ended.
    StopFailed(String),
    /// It was not connected, so nothing could be stopped.
    NotConnected(String),
    /// A new name the catalog refuses: empty, too long, a control character, or `local`.
    NameRefused,
    /// The machine list could not be saved.
    SaveFailed,
    PasswordForgotten,
    NoPasswordSaved,
    ForgetFailed,
}

/// Names in natural order: case aside, digit runs compare by value, so `build-2` comes before
/// `build-10`.
#[must_use]
pub fn natural_order(left: &str, right: &str) -> Ordering {
    let (mut left, mut right) = (left.chars().peekable(), right.chars().peekable());
    loop {
        match (left.peek().copied(), right.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(a), Some(b)) if a.is_ascii_digit() && b.is_ascii_digit() => {
                let a = digits(&mut left);
                let b = digits(&mut right);
                let (a, b) = (a.trim_start_matches('0'), b.trim_start_matches('0'));
                let order = a.len().cmp(&b.len()).then_with(|| a.cmp(b));
                if order != Ordering::Equal {
                    return order;
                }
            }
            (Some(a), Some(b)) => {
                let order = a.to_lowercase().cmp(b.to_lowercase());
                if order != Ordering::Equal {
                    return order;
                }
                left.next();
                right.next();
            }
        }
    }
}

fn digits(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut run = String::new();
    while let Some(digit) = chars.next_if(char::is_ascii_digit) {
        run.push(digit);
    }
    run
}

#[cfg(test)]
mod tests;
