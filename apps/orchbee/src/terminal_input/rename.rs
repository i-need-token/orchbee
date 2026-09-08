use std::time::{Duration, Instant};

mod paste;
mod utf8;

use paste::{Paste, PasteStep};
use utf8::Scalar;

const WAIT: Duration = Duration::from_millis(50);
const MAX_SEQUENCE: usize = 32;

#[derive(Debug, Eq, PartialEq)]
pub(super) enum RenameInputEvent {
    Append(String),
    Backspace,
    Clear,
    Fault,
    Completeness(bool),
    Submit,
    Cancel,
    InputFailure,
}

enum Phase {
    Idle,
    Text,
    Utf8 { scalar: Scalar, discard: bool },
    Escape(Instant),
    Sequence(Sequence),
    Paste(Paste),
    Quiet,
    Suppressed,
}

struct Sequence {
    bytes: [u8; MAX_SEQUENCE],
    length: usize,
    intermediate: bool,
    drain: bool,
}

impl Sequence {
    fn new(introducer: u8) -> Self {
        let mut bytes = [0; MAX_SEQUENCE];
        bytes[0] = 0x1b;
        bytes[1] = introducer;
        Self {
            bytes,
            length: 2,
            intermediate: false,
            drain: false,
        }
    }
}

pub(super) struct RenameDecoder {
    phase: Phase,
    editing: bool,
    handoff_deadline: Option<Instant>,
    swallow_lf: bool,
}

impl Default for RenameDecoder {
    fn default() -> Self {
        Self {
            phase: Phase::Idle,
            editing: false,
            handoff_deadline: None,
            swallow_lf: false,
        }
    }
}

impl RenameDecoder {
    pub(super) fn owns_input(&self) -> bool {
        !matches!(self.phase, Phase::Idle)
    }

    pub(super) fn start(&mut self) -> bool {
        if self.owns_input() {
            return false;
        }
        self.editing = true;
        self.phase = Phase::Text;
        true
    }

    pub(super) fn cancel(&mut self, now: Instant) -> Vec<RenameInputEvent> {
        let mut events = Vec::new();
        if !self.editing {
            return events;
        }
        self.editing = false;
        if !self.reset_quiet_deadline(now, &mut events) {
            return events;
        }
        match &mut self.phase {
            Phase::Paste(paste) => paste.discard(),
            Phase::Sequence(_) => {}
            Phase::Utf8 { discard, .. } => *discard = true,
            Phase::Escape(_) => {}
            _ => self.phase = Phase::Quiet,
        }
        events
    }

    pub(super) fn tick(&mut self, now: Instant) -> Vec<RenameInputEvent> {
        let mut events = Vec::new();
        if matches!(self.phase, Phase::Escape(deadline) if now >= deadline) {
            self.phase = Phase::Quiet;
            if self.editing {
                self.editing = false;
                events.push(RenameInputEvent::Cancel);
            }
            self.reset_quiet_deadline(now, &mut events);
        }
        if matches!(self.phase, Phase::Quiet)
            && self
                .handoff_deadline
                .is_some_and(|deadline| now >= deadline)
        {
            self.phase = Phase::Idle;
            self.handoff_deadline = None;
        }
        events
    }

    pub(super) fn push(
        &mut self,
        byte: u8,
        now: Instant,
        remaining: usize,
    ) -> (bool, Vec<RenameInputEvent>) {
        let mut events = self.tick(now);
        if self.swallow_lf {
            self.swallow_lf = false;
            if byte == b'\n' {
                return (true, events);
            }
        }
        if !self.owns_input() {
            return (false, events);
        }
        if !self.editing
            && !matches!(self.phase, Phase::Suppressed)
            && !self.reset_quiet_deadline(now, &mut events)
        {
            return (true, events);
        }
        match std::mem::replace(&mut self.phase, Phase::Suppressed) {
            Phase::Text => self.text_byte(byte, now, &mut events),
            Phase::Quiet => {
                self.phase = Phase::Quiet;
                if byte == 0x1b {
                    self.escape(now, &mut events);
                }
            }
            Phase::Escape(_) => match byte {
                b'[' | b'O' => self.phase = Phase::Sequence(Sequence::new(byte)),
                0x80..=0xff => self.scalar_byte(Scalar::default(), true, byte, now, &mut events),
                _ => self.finish_frame(now, &mut events),
            },
            Phase::Utf8 { scalar, discard } => {
                self.scalar_byte(scalar, discard, byte, now, &mut events)
            }
            Phase::Sequence(sequence) => {
                self.sequence_byte(sequence, byte, now, remaining, &mut events)
            }
            Phase::Paste(mut paste) => match paste.push(byte) {
                PasteStep::Pending => self.phase = Phase::Paste(paste),
                PasteStep::Fault => {
                    if self.editing {
                        events.push(RenameInputEvent::Fault);
                    }
                    self.phase = Phase::Paste(paste);
                }
                PasteStep::Complete(text) => {
                    self.finish_frame(now, &mut events);
                    if self.editing {
                        events.push(text.map_or(RenameInputEvent::Fault, RenameInputEvent::Append));
                    }
                }
            },
            Phase::Suppressed | Phase::Idle => self.phase = Phase::Suppressed,
        }
        (true, events)
    }

    fn text_byte(&mut self, byte: u8, now: Instant, events: &mut Vec<RenameInputEvent>) {
        self.phase = Phase::Text;
        match byte {
            0x1b => {
                events.push(RenameInputEvent::Completeness(false));
                self.escape(now, events);
            }
            8 | 127 => events.push(RenameInputEvent::Backspace),
            21 => events.push(RenameInputEvent::Clear),
            b'\r' | b'\n' => {
                self.swallow_lf = byte == b'\r';
                events.push(RenameInputEvent::Submit);
            }
            32..=126 => events.push(RenameInputEvent::Append(char::from(byte).to_string())),
            0x80..=0xff => {
                events.push(RenameInputEvent::Completeness(false));
                self.scalar_byte(Scalar::default(), false, byte, now, events);
            }
            _ => {}
        }
    }

    fn scalar_byte(
        &mut self,
        mut scalar: Scalar,
        discard: bool,
        byte: u8,
        now: Instant,
        events: &mut Vec<RenameInputEvent>,
    ) {
        match scalar.push(byte) {
            Ok(None) => self.phase = Phase::Utf8 { scalar, discard },
            Ok(Some(character)) => {
                self.finish_frame(now, events);
                if self.editing && !discard {
                    events.push(RenameInputEvent::Append(character.to_string()));
                }
            }
            Err(()) => {
                if self.editing {
                    events.push(RenameInputEvent::Fault);
                }
                self.finish_frame(now, events);
                if byte == 0x1b {
                    if self.editing {
                        events.push(RenameInputEvent::Completeness(false));
                    }
                    self.escape(now, events);
                }
            }
        }
    }

    fn sequence_byte(
        &mut self,
        mut sequence: Sequence,
        byte: u8,
        now: Instant,
        remaining: usize,
        events: &mut Vec<RenameInputEvent>,
    ) {
        let final_byte = (0x40..=0x7e).contains(&byte);
        if !sequence.drain {
            let valid = final_byte
                || (0x20..=0x2f).contains(&byte)
                || (!sequence.intermediate && (0x30..=0x3f).contains(&byte));
            if sequence.length == MAX_SEQUENCE || !valid {
                sequence.drain = true;
                if self.editing {
                    events.push(RenameInputEvent::Fault);
                }
            } else {
                sequence.bytes[sequence.length] = byte;
                sequence.length += 1;
                sequence.intermediate |= (0x20..=0x2f).contains(&byte);
            }
        }
        if final_byte {
            if !sequence.drain && &sequence.bytes[..sequence.length] == b"\x1b[200~" {
                let mut paste = Paste::new(remaining);
                if !self.editing {
                    paste.discard();
                }
                self.phase = Phase::Paste(paste);
            } else {
                self.finish_frame(now, events);
            }
        } else {
            self.phase = Phase::Sequence(sequence);
        }
    }

    fn finish_frame(&mut self, now: Instant, events: &mut Vec<RenameInputEvent>) {
        if self.editing {
            self.phase = Phase::Text;
            events.push(RenameInputEvent::Completeness(true));
        } else if self.reset_quiet_deadline(now, events) {
            self.phase = Phase::Quiet;
        }
    }

    fn escape(&mut self, now: Instant, events: &mut Vec<RenameInputEvent>) {
        if let Some(deadline) = now.checked_add(WAIT) {
            self.phase = Phase::Escape(deadline);
        } else {
            self.suppress(events);
        }
    }

    fn reset_quiet_deadline(&mut self, now: Instant, events: &mut Vec<RenameInputEvent>) -> bool {
        self.handoff_deadline = now.checked_add(WAIT);
        if self.handoff_deadline.is_none() {
            self.suppress(events);
            return false;
        }
        true
    }

    fn suppress(&mut self, events: &mut Vec<RenameInputEvent>) {
        if self.editing {
            events.push(RenameInputEvent::Cancel);
        }
        self.editing = false;
        self.phase = Phase::Suppressed;
        events.push(RenameInputEvent::InputFailure);
    }
}

#[cfg(test)]
mod tests;
