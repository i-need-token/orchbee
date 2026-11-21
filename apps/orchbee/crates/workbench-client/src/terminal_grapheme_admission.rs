//! One-shot admission of outer-terminal grapheme clustering (DEC mode 2027).
//!
//! The Client asks once at startup, inside the reply window the pointer query uses.
//! Only a Reset reply inside that window admits the enable. Set, permanently set,
//! permanently reset, unsupported, a reply after the window, and no reply all leave the
//! terminal as it is. The first report inside the window settles the probe, so a later
//! report cannot reopen admission.

#[cfg(windows)]
use std::io;
use std::{fmt, time::Instant};

use crate::{terminal_events::ModeReportState, terminal_pointer_admission::QUERY_DEADLINE};

/// The endpoint's one DECRQM query for DEC mode 2027.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Query;

impl Query {
    pub(crate) const fn sequence(self) -> &'static str {
        "\x1b[?2027$p"
    }
}

impl crossterm::Command for Query {
    #[cfg(windows)]
    fn execute_winapi(&self) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        true
    }

    fn write_ansi(&self, output: &mut impl fmt::Write) -> fmt::Result {
        output.write_str(self.sequence())
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum State {
    #[default]
    Unissued,
    Pending {
        deadline: Instant,
    },
    Settled,
}

/// Admission of the endpoint's one 2027 query. It is deliberately not
/// resettable: a new probe requires a newly admitted endpoint owner.
#[derive(Debug, Default)]
pub struct Probe {
    state: State,
    enable_pending: bool,
    /// The deadline passed without a report since the owner last took this.
    unanswered: bool,
}

impl Probe {
    /// Begins the endpoint's sole query. Later calls are permanent no-ops.
    pub(crate) fn begin(&mut self, now: Instant) -> Option<Query> {
        if self.state != State::Unissued {
            return None;
        }
        self.state = State::Pending {
            deadline: now + QUERY_DEADLINE,
        };
        Some(Query)
    }

    /// Applies the first 2027 report inside the window. Only Reset admits the enable. The
    /// deadline wins at its exact boundary, so a report that arrives after it is ignored.
    pub(crate) fn observe(&mut self, reply: ModeReportState, now: Instant) {
        self.tick(now);
        if !matches!(self.state, State::Pending { .. }) {
            return;
        }
        self.state = State::Settled;
        self.enable_pending = reply == ModeReportState::Reset;
    }

    /// Settles a missing reply once. A timeout never permits another probe.
    pub(crate) fn tick(&mut self, now: Instant) {
        let State::Pending { deadline } = self.state else {
            return;
        };
        if now >= deadline {
            self.state = State::Settled;
            self.unanswered = true;
        }
    }

    pub(crate) fn pending(&self) -> bool {
        matches!(self.state, State::Pending { .. })
    }

    pub(crate) fn take_unanswered(&mut self) -> bool {
        std::mem::take(&mut self.unanswered)
    }

    /// Takes an admitted enable once, so the caller emits it at most once.
    pub(crate) fn take_enable(&mut self) -> bool {
        std::mem::take(&mut self.enable_pending)
    }
}

#[cfg(test)]
mod tests;
