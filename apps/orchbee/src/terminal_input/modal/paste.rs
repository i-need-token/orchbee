use std::time::{Duration, Instant};

use super::{ModalCapacity, utf8::Scalar};
use crate::terminal_input::utf8::Continuation;

const END_ESC: &[u8; 6] = b"\x1b[201~";
const END_C1: &[u8; 5] = b"\x9b201~";
/// The bounded wait for the next payload byte. On expiry the collected
/// text is dropped and the run keeps draining to the real terminator —
/// the only honest release boundary for a paste.
const WAIT: Duration = Duration::from_millis(50);

pub(super) struct Paste {
    text: String,
    scalar: Scalar,
    utf8: Continuation,
    remaining: usize,
    capacity: ModalCapacity,
    /// The terminator matching the opener's notation: an `ESC[200~` run ends
    /// only at `ESC[201~`, a `9b200~` run only at `9b201~`. A `\x9b` UTF-8
    /// continuation byte inside the payload is data, never a C1 terminator.
    end: &'static [u8],
    prefix: usize,
    state: State,
    /// `None` once the run drains: a drained tail is consumed on arrival
    /// with bounded memory and needs no further clock.
    deadline: Option<Instant>,
}

enum State {
    Collecting,
    /// Completion still owes a refusal if the modal owner is editing.
    Rejected,
    /// Established only by the same push that returns the first Fault.
    FaultReported,
}

pub(super) enum PasteStep {
    Pending,
    Fault,
    Complete(Option<String>),
    /// A previously reported refusal has reached its real terminator.
    Drained,
}

impl Paste {
    pub(super) fn new(capacity: ModalCapacity, remaining: usize, c1: bool, now: Instant) -> Self {
        Self {
            text: String::new(),
            scalar: Scalar::default(),
            utf8: Continuation::default(),
            remaining: capacity.bound(remaining),
            capacity,
            end: if c1 { END_C1 } else { END_ESC },
            prefix: 0,
            state: State::Collecting,
            deadline: now.checked_add(WAIT),
        }
    }

    pub(super) fn discard(&mut self) {
        if matches!(self.state, State::Collecting) {
            self.state = State::Rejected;
        }
        self.text.clear();
        self.deadline = None;
    }

    /// The bounded payload wait expired: the collected text is dropped and
    /// the run drains until the real terminator.
    pub(super) fn tick(&mut self, now: Instant) {
        if let Some(deadline) = self.deadline
            && now >= deadline
        {
            self.discard();
        }
    }

    pub(super) fn push(&mut self, byte: u8, now: Instant) -> PasteStep {
        self.deadline = self.deadline.and_then(|_| now.checked_add(WAIT));
        let was_collecting = matches!(self.state, State::Collecting);
        // This must advance even after timeout or cancellation, when scalar
        // admission has stopped. Otherwise a stale scalar could own all C1 ends.
        let continuation = self.utf8.push(byte);
        let starts_end = byte == self.end[0] && !(continuation && self.end == END_C1);
        if (self.prefix > 0 && byte == self.end[self.prefix]) || (self.prefix == 0 && starts_end) {
            self.prefix += 1;
            if self.prefix == self.end.len() {
                return match self.state {
                    State::Collecting if self.scalar.complete() => {
                        PasteStep::Complete(Some(std::mem::take(&mut self.text)))
                    }
                    State::FaultReported => PasteStep::Drained,
                    State::Collecting | State::Rejected => PasteStep::Complete(None),
                };
            }
        } else {
            for literal in &self.end[..self.prefix] {
                self.text_byte(*literal);
            }
            self.prefix = 0;
            if starts_end {
                self.prefix = 1;
            } else {
                self.text_byte(byte);
            }
        }
        if was_collecting && matches!(self.state, State::Rejected) {
            self.state = State::FaultReported;
            PasteStep::Fault
        } else {
            PasteStep::Pending
        }
    }

    fn text_byte(&mut self, byte: u8) {
        if !matches!(self.state, State::Collecting) {
            return;
        }
        match self.scalar.push(byte) {
            Ok(Some(character)) if self.remaining >= self.capacity.cost(character) => {
                self.text.push(character);
                self.remaining -= self.capacity.cost(character);
            }
            Ok(None) => {}
            Ok(Some(_)) | Err(()) => self.discard(),
        }
    }
}
