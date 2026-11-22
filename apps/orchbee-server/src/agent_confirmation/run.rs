//! One Agent run's confirmation decisions. The state lives in Server memory and ends with the
//! Agent process, so "this Agent only" ends there and a Server restart forgets it, which fails
//! closed. `formal/tla/AgentConfirmationPress.tla` checks the order of these steps against the
//! Terminal Host's gate.
use std::num::NonZeroU64;
use std::time::{Duration, Instant};

use super::{Dialog, Kind, Wait};

/// A prompt must stay the same this long, across this many observations, before it is answered.
const SETTLE: Duration = Duration::from_secs(1);
const SETTLE_OBSERVATIONS: u32 = 3;
/// The Host's quiet time after a person's input; a key refused for it is decided again after it.
const QUIET: Duration = Duration::from_secs(3);
/// Presses of one prompt in a row before pressing stops for the run.
const PRESS_LIMIT: u8 = 3;
/// Offers one run may show.
const OFFER_LIMIT: u8 = 2;
/// Refusals for a changed screen before the row says that the screen keeps changing.
const CHANGING_REFUSALS: u8 = 8;
const PRESSED_CHARACTERS: usize = 96;

/// The frame a dialog was read from, as the Terminal Host numbers it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Frame {
    pub(crate) generation: NonZeroU64,
    pub(crate) revision: u64,
}

/// What the Host must still see when it writes the key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Gate {
    pub(crate) frame: Frame,
    /// The revision of the frame in which this prompt was first seen.
    pub(crate) first_revision: u64,
}

/// What one sampler turn saw for the run.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Seen<'a> {
    pub(crate) frame: Frame,
    /// The Agent waits on something, by its detected state.
    pub(crate) blocked: bool,
    /// The dialog read from the frame and what it asks for, judged.
    pub(crate) dialog: Option<(&'a Dialog, Result<(), Wait>)>,
}

/// The saved settings a decision reads, as they are now.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Settings {
    /// Automatic confirmation is on for the run's Agent type.
    pub(crate) agent_type: bool,
    /// Offers may still be shown; false once a person answered "no".
    pub(crate) offer: bool,
}

/// What the run's row shows as its choice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Choice {
    Off,
    Run,
    AgentType,
}

/// What the caller does next.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Step {
    Nothing,
    /// Send the layout's accept key, gated on the frame.
    Press(Gate),
    /// Send a hold, so no key decided earlier can still be written.
    Hold,
}

/// Why the Host refused a key; mirrors the Host's refusal codes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Refusal {
    AgentGone,
    Changed,
    AlreadyTyped,
    InputPending,
    PersonInput,
    PersonRecent,
}

/// How a gated send ended.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Outcome {
    Written,
    Refused(Refusal),
    /// The Host, Worker or Server on the way does not offer the gate.
    Unsupported,
    /// No answer, or one that does not say whether the key was written.
    Unknown,
    /// The key provably never left for the Host: the terminal, its Worker or the choice changed
    /// before it was sent, or the way there refused it.
    NotSent,
}

/// The last confirmation Orchbee answered, for the status notice.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Pressed {
    /// Counts presses of the run, so a Client can tell a new press from one it has shown.
    pub(crate) serial: u32,
    pub(crate) kind: Kind,
    pub(crate) text: String,
}

/// What Clients show for the run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Facts {
    pub(crate) choice: Choice,
    pub(crate) offer: bool,
    pub(crate) wait: Option<Wait>,
    pub(crate) pressed: Option<Pressed>,
    /// Pressing stopped because one confirmation kept coming back.
    pub(crate) stopped: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Sent {
    No,
    /// On its way, or ended without saying whether it was written; counts as written.
    Unknown,
    Written,
}

struct Current {
    dialog: Dialog,
    verdict: Result<(), Wait>,
    first_revision: u64,
    first_seen: Instant,
    observations: u32,
    sent: Sent,
    person: bool,
    retry_at: Option<Instant>,
    refusals: u8,
    /// The run already offered on this prompt.
    offered: bool,
}

/// One Agent run's state.
pub(crate) struct Run {
    run: bool,
    stopped: bool,
    /// The choice was on at the last observation, so turning it off owes the Host a hold.
    was_on: bool,
    hold_pending: bool,
    unsupported: bool,
    blocked: bool,
    offers: u8,
    offer: bool,
    current: Option<Current>,
    /// The prompt pressed last and how many times in a row.
    repeated: Option<(Dialog, u8)>,
    pressed: Option<Pressed>,
}

/// The first row of what was answered, marked as cut when it is longer, when more rows follow, or
/// when the Agent showed none.
fn shown(dialog: &Dialog) -> String {
    let mut text: String = dialog
        .text
        .first()
        .map(|row| row.chars().take(PRESSED_CHARACTERS).collect())
        .unwrap_or_default();
    let cut = dialog
        .text
        .first()
        .is_some_and(|row| row.chars().count() > PRESSED_CHARACTERS);
    if cut || dialog.text.len() != 1 {
        text.push('…');
    }
    text
}

impl Run {
    pub(crate) fn new() -> Self {
        Self {
            run: false,
            stopped: false,
            was_on: false,
            hold_pending: false,
            unsupported: false,
            blocked: false,
            offers: 0,
            offer: false,
            current: None,
            repeated: None,
            pressed: None,
        }
    }

    fn on(&self, settings: Settings) -> bool {
        !self.stopped && (self.run || settings.agent_type)
    }

    /// A person set the run's choice. On clears a repeat stop; the hold that Off owes is sent by
    /// the next observation.
    pub(crate) fn choose(&mut self, run: bool) {
        self.run = run;
        self.stopped = false;
        self.repeated = None;
        self.offer = false;
    }

    fn track(&mut self, now: Instant, seen: Seen<'_>) {
        self.blocked = seen.blocked;
        let Some((dialog, verdict)) = seen.dialog else {
            self.current = None;
            self.offer = false;
            return;
        };
        match &mut self.current {
            Some(current) if current.dialog == *dialog => {
                current.observations = current.observations.saturating_add(1);
                current.verdict = verdict;
            }
            _ => {
                self.offer = false;
                self.current = Some(Current {
                    dialog: dialog.clone(),
                    verdict,
                    first_revision: seen.frame.revision,
                    first_seen: now,
                    observations: 1,
                    sent: Sent::No,
                    person: false,
                    retry_at: None,
                    refusals: 0,
                    offered: false,
                });
            }
        }
    }

    /// Whether the Host is owed a hold: the choice went off since it was last seen on, or an
    /// earlier hold has no answer yet.
    pub(crate) fn owes_hold(&mut self, settings: Settings) -> bool {
        if self.was_on && !self.on(settings) {
            self.was_on = false;
            self.hold_pending = true;
        }
        self.hold_pending
    }

    /// Whether the press the last observation asked for may still leave: the choice is on, no
    /// hold is owed, and the press has no outcome yet. Read again when the key is published.
    pub(crate) fn sending(&self, settings: Settings) -> bool {
        self.on(settings)
            && !self.hold_pending
            && self
                .current
                .as_ref()
                .is_some_and(|current| current.sent == Sent::Unknown)
    }

    /// Takes one observation of the run's screen and says what to send, if anything. A hold is
    /// asked for again on every observation until it is answered.
    pub(crate) fn observe(&mut self, now: Instant, settings: Settings, seen: Seen<'_>) -> Step {
        self.track(now, seen);
        if self.owes_hold(settings) {
            return Step::Hold;
        }
        let on = self.on(settings);
        self.was_on = on;
        let Some(current) = &mut self.current else {
            return Step::Nothing;
        };
        let settled = current.observations >= SETTLE_OBSERVATIONS
            && now.saturating_duration_since(current.first_seen) >= SETTLE;
        if !on {
            if !settings.offer || self.stopped {
                self.offer = false;
            } else if !self.offer
                && self.offers < OFFER_LIMIT
                && settled
                && current.verdict.is_ok()
                && !current.offered
            {
                self.offer = true;
                self.offers += 1;
                current.offered = true;
            }
            return Step::Nothing;
        }
        self.offer = false;
        if current.verdict.is_err()
            || current.person
            || current.sent != Sent::No
            || self.unsupported
            || !settled
            || current.retry_at.is_some_and(|at| now < at)
        {
            return Step::Nothing;
        }
        if self
            .repeated
            .as_ref()
            .is_some_and(|(dialog, presses)| *dialog == current.dialog && *presses >= PRESS_LIMIT)
        {
            self.stopped = true;
            self.was_on = false;
            self.hold_pending = true;
            return Step::Hold;
        }
        current.sent = Sent::Unknown;
        Step::Press(Gate {
            frame: seen.frame,
            first_revision: current.first_revision,
        })
    }

    fn count(&mut self) {
        let Some(current) = &self.current else {
            return;
        };
        match &mut self.repeated {
            Some((dialog, presses)) if *dialog == current.dialog => {
                *presses = presses.saturating_add(1);
            }
            _ => self.repeated = Some((current.dialog.clone(), 1)),
        }
    }

    /// Records how the press that the last observation asked for ended.
    pub(crate) fn pressed(&mut self, now: Instant, outcome: Outcome) {
        match outcome {
            Outcome::Written => {
                self.count();
                if let Some(current) = &mut self.current {
                    current.sent = Sent::Written;
                    let serial = self.pressed.as_ref().map_or(0, |pressed| pressed.serial);
                    self.pressed = Some(Pressed {
                        serial: serial.wrapping_add(1),
                        kind: current.dialog.kind,
                        text: shown(&current.dialog),
                    });
                }
            }
            Outcome::Unknown => self.count(),
            Outcome::NotSent => {
                if let Some(current) = &mut self.current {
                    current.sent = Sent::No;
                    current.retry_at = Some(now + QUIET);
                }
            }
            Outcome::Unsupported => {
                self.unsupported = true;
                if let Some(current) = &mut self.current {
                    current.sent = Sent::No;
                }
            }
            Outcome::Refused(refusal) => {
                let Some(current) = &mut self.current else {
                    return;
                };
                current.sent = Sent::No;
                match refusal {
                    Refusal::PersonInput => current.person = true,
                    Refusal::PersonRecent => current.retry_at = Some(now + QUIET),
                    Refusal::Changed | Refusal::AlreadyTyped | Refusal::InputPending => {
                        current.refusals = current.refusals.saturating_add(1);
                    }
                    Refusal::AgentGone => {}
                }
            }
        }
    }

    /// The Host answered the hold, or cannot type a gated key at all.
    pub(crate) fn held(&mut self) {
        self.hold_pending = false;
    }

    /// What Clients show. The choice stays as it was until the hold that Off owes is answered.
    pub(crate) fn facts(&self, settings: Settings) -> Facts {
        let on = self.on(settings);
        let choice = if settings.agent_type && (on || self.hold_pending) {
            Choice::AgentType
        } else if on || self.hold_pending {
            Choice::Run
        } else {
            Choice::Off
        };
        let wait = if self.stopped {
            self.blocked.then_some(Wait::Repeating)
        } else if !on || !self.blocked {
            None
        } else if self.unsupported {
            Some(Wait::HostTooOld)
        } else {
            match &self.current {
                None => Some(Wait::NotAnswered),
                Some(current) if current.person => Some(Wait::Person),
                Some(current) => current.verdict.err().or_else(|| {
                    (current.sent == Sent::No && current.refusals >= CHANGING_REFUSALS)
                        .then_some(Wait::Changing)
                }),
            }
        };
        Facts {
            choice,
            offer: self.offer && settings.offer && !on,
            wait,
            pressed: self.pressed.clone(),
            stopped: self.stopped,
        }
    }
}
