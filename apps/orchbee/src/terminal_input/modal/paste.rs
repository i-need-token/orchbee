use std::time::{Duration, Instant};

use super::{ModalCapacity, utf8::Scalar};

const END_ESC: &[u8; 6] = b"\x1b[201~";
const END_C1: &[u8; 5] = b"\x9b201~";
/// The bounded wait for the next payload byte. On expiry the collected
/// text is dropped and the run keeps draining to the real terminator —
/// the only honest release boundary for a paste.
const WAIT: Duration = Duration::from_millis(50);

pub(super) struct Paste {
    text: String,
    scalar: Scalar,
    remaining: usize,
    capacity: ModalCapacity,
    /// The terminator matching the opener's notation: an `ESC[200~` run ends
    /// only at `ESC[201~`, a `9b200~` run only at `9b201~`. A `\x9b` UTF-8
    /// continuation byte inside the payload is data, never a C1 terminator.
    end: &'static [u8],
    prefix: usize,
    rejected: bool,
    /// `None` once the run drains: a drained tail is consumed on arrival
    /// with bounded memory and needs no further clock.
    deadline: Option<Instant>,
}

pub(super) enum PasteStep {
    Pending,
    Fault,
    Complete(Option<String>),
}

impl Paste {
    pub(super) fn new(capacity: ModalCapacity, remaining: usize, c1: bool, now: Instant) -> Self {
        Self {
            text: String::new(),
            scalar: Scalar::default(),
            remaining: capacity.bound(remaining),
            capacity,
            end: if c1 { END_C1 } else { END_ESC },
            prefix: 0,
            rejected: false,
            deadline: now.checked_add(WAIT),
        }
    }

    pub(super) fn discard(&mut self) {
        self.rejected = true;
        self.text.clear();
    }

    /// The bounded payload wait expired: the collected text is dropped and
    /// the run drains until the real terminator.
    pub(super) fn tick(&mut self, now: Instant) {
        if let Some(deadline) = self.deadline
            && now >= deadline
        {
            self.discard();
            self.deadline = None;
        }
    }

    pub(super) fn push(&mut self, byte: u8, now: Instant) -> PasteStep {
        self.deadline = self.deadline.and_then(|_| now.checked_add(WAIT));
        let was_rejected = self.rejected;
        if byte == self.end[self.prefix] {
            self.prefix += 1;
            if self.prefix == self.end.len() {
                return PasteStep::Complete(
                    (!self.rejected && self.scalar.complete())
                        .then(|| std::mem::take(&mut self.text)),
                );
            }
        } else {
            for literal in &self.end[..self.prefix] {
                self.text_byte(*literal);
            }
            self.prefix = 0;
            if byte == self.end[0] {
                self.prefix = 1;
            } else {
                self.text_byte(byte);
            }
        }
        if !was_rejected && self.rejected {
            PasteStep::Fault
        } else {
            PasteStep::Pending
        }
    }

    fn text_byte(&mut self, byte: u8) {
        if self.rejected {
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
