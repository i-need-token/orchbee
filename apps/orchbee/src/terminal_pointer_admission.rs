//! One-shot admission of outer-terminal any-motion pointer reporting.
//!
//! DEC mode reports do not carry a query identifier. One admitted terminal
//! endpoint therefore gets at most one probe: after any settlement, a late
//! reply can be consumed by the input decoder but can never reopen admission.

use std::io;
use std::{
    fmt,
    time::{Duration, Instant},
};

const QUERY_DEADLINE: Duration = Duration::from_millis(300);

/// The one query issued after button-event tracking has established the host
/// baseline. DA1 is an ordered completion sentinel for hosts that ignore
/// DECRQM; it does not correlate or authorize a second query.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Query;

impl Query {
    pub(crate) const fn sequence(self) -> &'static str {
        "\x1b[?1003$p\x1b[c"
    }
}

impl crossterm::Command for Query {
    fn execute_winapi(&self) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }

    fn is_ansi_code_supported(&self) -> bool {
        true
    }

    fn write_ansi(&self, output: &mut impl fmt::Write) -> fmt::Result {
        output.write_str(self.sequence())
    }
}

/// The five values defined by DECRPM for one DEC private mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ModeReportState {
    Unsupported,
    Set,
    Reset,
    PermanentlySet,
    PermanentlyReset,
}

/// Pointer-admission replies recognized and removed by the host decoder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HostReply {
    Mode1003(ModeReportState),
    PrimaryDeviceAttributes,
}

/// Settled capability of this exact admitted outer-terminal endpoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Capability {
    Admitted,
    Unavailable,
    Unknown,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum State {
    #[default]
    Unissued,
    Pending {
        deadline: Instant,
    },
    Settled(Capability),
}

/// A probe is deliberately neither resettable nor generation-based. A new
/// probe requires a newly admitted endpoint owner.
#[derive(Debug, Default)]
pub(crate) struct Probe {
    state: State,
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

    /// Applies one typed host reply. The deadline wins at its exact boundary,
    /// and every reply after settlement is inert.
    pub(crate) fn observe(&mut self, reply: HostReply, now: Instant) -> Option<Capability> {
        if let Some(capability) = self.tick(now) {
            return Some(capability);
        }
        if !matches!(self.state, State::Pending { .. }) {
            return None;
        }
        let capability = match reply {
            // The query follows a successful `1003l -> 1002h` baseline.
            // Seeing Set contradicts that ordered baseline rather than
            // authorizing hover from an uncorrelated or stale observation.
            HostReply::Mode1003(ModeReportState::Reset) => Capability::Admitted,
            HostReply::Mode1003(ModeReportState::Set) | HostReply::PrimaryDeviceAttributes => {
                Capability::Unknown
            }
            HostReply::Mode1003(
                ModeReportState::Unsupported
                | ModeReportState::PermanentlySet
                | ModeReportState::PermanentlyReset,
            ) => Capability::Unavailable,
        };
        self.state = State::Settled(capability);
        Some(capability)
    }

    /// Settles a missing reply once. A timeout never permits another probe.
    pub(crate) fn tick(&mut self, now: Instant) -> Option<Capability> {
        let State::Pending { deadline } = self.state else {
            return None;
        };
        if now < deadline {
            return None;
        }
        self.state = State::Settled(Capability::Unknown);
        Some(Capability::Unknown)
    }

    #[cfg(test)]
    pub(crate) const fn capability(&self) -> Option<Capability> {
        match self.state {
            State::Settled(capability) => Some(capability),
            State::Unissued | State::Pending { .. } => None,
        }
    }

    pub(crate) const fn any_motion_admitted(&self) -> bool {
        matches!(self.state, State::Settled(Capability::Admitted))
    }
}

#[cfg(test)]
mod tests;
