use std::time::{Duration, Instant};

mod paste;
mod utf8;

use paste::{Paste, PasteStep};
use utf8::Scalar;

const WAIT: Duration = Duration::from_millis(50);
const MAX_SEQUENCE: usize = 32;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum ModalCapacity {
    #[default]
    Scalars512,
    Bytes4096,
}

impl ModalCapacity {
    fn bound(self, remaining: usize) -> usize {
        remaining.min(match self {
            Self::Scalars512 => 512,
            Self::Bytes4096 => 4096,
        })
    }

    fn cost(self, character: char) -> usize {
        match self {
            Self::Scalars512 => 1,
            Self::Bytes4096 => character.len_utf8(),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum ModalInputEvent {
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
    Utf8 {
        scalar: Scalar,
        discard: bool,
        deadline: Instant,
    },
    /// A scalar whose bounded wait expired keeps exactly its remaining
    /// continuation bytes owned; anything else ends the frame.
    Utf8Tail {
        remaining: usize,
        deadline: Instant,
    },
    Escape(Instant),
    Sequence(Sequence),
    Paste(Paste),
    /// The modal released its lease: every remaining byte of the packet
    /// that held the decision stays owned, and a frame that starts inside
    /// it (ESC/C1 lead, UTF-8 scalar) keeps frame ownership across reads.
    /// Ends only at `begin_packet` — never by clock.
    Released,
    Suppressed,
}

struct Sequence {
    bytes: [u8; MAX_SEQUENCE],
    length: usize,
    intermediate: bool,
    drain: bool,
    /// The bounded wait for the next frame byte; `None` once the sequence
    /// drains, since a drained tail is consumed on arrival with bounded
    /// memory and needs no further clock.
    deadline: Option<Instant>,
}

impl Sequence {
    fn new(introducer: u8, now: Instant) -> Self {
        let mut bytes = [0; MAX_SEQUENCE];
        bytes[0] = 0x1b;
        bytes[1] = introducer;
        Self {
            bytes,
            length: 2,
            intermediate: false,
            drain: false,
            deadline: now.checked_add(WAIT),
        }
    }

    fn c1(lead: u8, now: Instant) -> Self {
        let mut bytes = [0; MAX_SEQUENCE];
        bytes[0] = lead;
        Self {
            bytes,
            length: 1,
            intermediate: false,
            drain: false,
            deadline: now.checked_add(WAIT),
        }
    }

    fn is_paste_opener(&self) -> bool {
        matches!(&self.bytes[..self.length], b"\x1b[200~" | b"\x9b200~")
    }
}

pub(super) struct ModalDecoder {
    capacity: ModalCapacity,
    phase: Phase,
    editing: bool,
    swallow_lf: bool,
    /// `Some(boundary)` — armed by boundary reconciliation (`cancel`): the
    /// guard covers the packet processed at that boundary instant and ends
    /// at the first strictly later one. `None` — armed inside a packet or
    /// by an expired frame: the guard covers only the rest of that packet
    /// and ends at the next `end_released` regardless of its instant.
    released_at: Option<Instant>,
}

impl Default for ModalDecoder {
    fn default() -> Self {
        Self {
            capacity: ModalCapacity::Scalars512,
            phase: Phase::Idle,
            editing: false,
            swallow_lf: false,
            released_at: None,
        }
    }
}

impl ModalDecoder {
    pub(super) fn owns_input(&self) -> bool {
        !matches!(self.phase, Phase::Idle)
    }

    #[cfg(test)]
    pub(super) fn start(&mut self) -> bool {
        self.start_with_capacity(ModalCapacity::Scalars512)
    }

    pub(super) fn start_with_capacity(&mut self, capacity: ModalCapacity) -> bool {
        if self.owns_input() {
            return false;
        }
        self.editing = true;
        self.capacity = capacity;
        self.phase = Phase::Text;
        self.released_at = None;
        true
    }

    /// A new read packet begins. A `Released` guard armed at an earlier
    /// boundary ends here; an in-flight frame or an open ESC disambiguation
    /// keeps ownership under its own rules. Called once per `dispatch`
    /// before the first byte of that packet.
    pub(super) fn begin_packet(&mut self, now: Instant) -> Vec<ModalInputEvent> {
        let events = self.tick(now);
        self.end_released(now);
        events
    }

    /// A boundary without input bytes — an idle `synchronize` — ends a
    /// `Released` guard armed before this boundary, so a released lease is
    /// always returned even when no next packet ever arrives. A guard armed
    /// at this same boundary still covers the packet being processed.
    pub(super) fn end_released(&mut self, now: Instant) {
        if matches!(self.phase, Phase::Released) && self.released_at.is_none_or(|armed| armed < now)
        {
            self.phase = Phase::Idle;
            self.released_at = None;
        }
    }

    pub(super) fn cancel(&mut self, now: Instant) -> Vec<ModalInputEvent> {
        let events = Vec::new();
        if !self.editing {
            return events;
        }
        self.editing = false;
        match &mut self.phase {
            Phase::Paste(paste) => paste.discard(),
            Phase::Sequence(_) => {}
            Phase::Utf8 { discard, .. } => *discard = true,
            Phase::Escape(_) => {}
            _ => self.release_at(now),
        }
        events
    }

    /// Armed by boundary reconciliation: the guard covers the packet
    /// processed at this boundary and ends at the first later one.
    fn release_at(&mut self, now: Instant) {
        self.phase = Phase::Released;
        self.released_at = Some(now);
    }

    /// Armed inside a packet or by an expired frame: the guard covers only
    /// the rest of that packet and ends at the next boundary.
    fn release_tail(&mut self) {
        self.phase = Phase::Released;
        self.released_at = None;
    }

    pub(super) fn tick(&mut self, now: Instant) -> Vec<ModalInputEvent> {
        let mut events = Vec::new();
        match &mut self.phase {
            Phase::Escape(deadline) if now >= *deadline => {
                // An expired frame owns nothing beyond its own packet: the
                // next boundary frees this guard regardless of its instant.
                self.release_tail();
                if self.editing {
                    self.editing = false;
                    events.push(ModalInputEvent::Cancel);
                }
            }
            // A truncated sequence stops waiting at its deadline and drains:
            // its late tail is still consumed on arrival, never released as
            // fresh input.
            Phase::Sequence(sequence) => {
                if let Some(deadline) = sequence.deadline
                    && now >= deadline
                {
                    sequence.drain = true;
                    sequence.deadline = None;
                    if self.editing {
                        events.push(ModalInputEvent::Fault);
                    }
                }
            }
            // A truncated scalar ends at its deadline: while editing the
            // frame faults back to text; after release the tail keeps its
            // remaining continuation bytes owned rather than leaking them.
            Phase::Utf8 {
                scalar, deadline, ..
            } if now >= *deadline => {
                if self.editing {
                    events.push(ModalInputEvent::Fault);
                    self.phase = Phase::Text;
                } else if scalar.remaining() > 0 {
                    self.phase = Phase::Utf8Tail {
                        remaining: scalar.remaining(),
                        deadline: now + WAIT,
                    };
                } else {
                    self.release_tail();
                }
            }
            Phase::Utf8Tail { deadline, .. } if now >= *deadline => {
                if self.editing {
                    events.push(ModalInputEvent::Fault);
                    self.phase = Phase::Text;
                } else {
                    self.release_tail();
                }
            }
            Phase::Paste(paste) => {
                paste.tick(now);
            }
            _ => {}
        }
        events
    }

    pub(super) fn push(
        &mut self,
        byte: u8,
        now: Instant,
        remaining: usize,
    ) -> (bool, Vec<ModalInputEvent>) {
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
        let mut pending = Some(byte);
        while let Some(byte) = pending.take() {
            match std::mem::replace(&mut self.phase, Phase::Suppressed) {
                Phase::Text => self.text_byte(byte, now, &mut events),
                Phase::Released => {
                    self.phase = Phase::Released;
                    match byte {
                        0x1b => self.escape(now, &mut events),
                        0x9b | 0x8f => self.phase = Phase::Sequence(Sequence::c1(byte, now)),
                        0x80..=0xff => {
                            self.scalar_byte(Scalar::default(), true, byte, now, &mut events)
                        }
                        _ => {}
                    }
                }
                Phase::Escape(_) => match byte {
                    b'[' | b'O' => self.phase = Phase::Sequence(Sequence::new(byte, now)),
                    0x80..=0xff => {
                        self.scalar_byte(Scalar::default(), true, byte, now, &mut events)
                    }
                    _ => self.finish_frame(now, &mut events),
                },
                Phase::Utf8 {
                    scalar, discard, ..
                } => self.scalar_byte(scalar, discard, byte, now, &mut events),
                Phase::Utf8Tail { remaining, .. } => {
                    if (0x80..=0xbf).contains(&byte) {
                        // A late tail byte stays owned by the frame it
                        // belongs to; the wait re-bounds per byte.
                        let remaining = remaining - 1;
                        if remaining == 0 {
                            self.finish_frame(now, &mut events);
                        } else {
                            self.phase = Phase::Utf8Tail {
                                remaining,
                                deadline: now + WAIT,
                            };
                        }
                    } else {
                        // The tail never came: this byte belongs to whatever
                        // the frame resolved into, so it is reprocessed.
                        self.finish_frame(now, &mut events);
                        pending = Some(byte);
                    }
                }
                Phase::Sequence(sequence) => {
                    self.sequence_byte(sequence, byte, now, remaining, &mut events)
                }
                Phase::Paste(mut paste) => match paste.push(byte, now) {
                    PasteStep::Pending => self.phase = Phase::Paste(paste),
                    PasteStep::Fault => {
                        if self.editing {
                            events.push(ModalInputEvent::Fault);
                        }
                        self.phase = Phase::Paste(paste);
                    }
                    PasteStep::Complete(text) => {
                        self.finish_frame(now, &mut events);
                        if self.editing {
                            events
                                .push(text.map_or(ModalInputEvent::Fault, ModalInputEvent::Append));
                        }
                    }
                },
                Phase::Suppressed | Phase::Idle => self.phase = Phase::Suppressed,
            }
        }
        (true, events)
    }

    fn text_byte(&mut self, byte: u8, now: Instant, events: &mut Vec<ModalInputEvent>) {
        self.phase = Phase::Text;
        match byte {
            0 if self.capacity == ModalCapacity::Bytes4096 => {
                events.push(ModalInputEvent::Append("\0".to_owned()));
            }
            0x1b => {
                events.push(ModalInputEvent::Completeness(false));
                self.escape(now, events);
            }
            8 | 127 => events.push(ModalInputEvent::Backspace),
            21 => events.push(ModalInputEvent::Clear),
            b'\r' | b'\n' => {
                self.swallow_lf = byte == b'\r';
                events.push(ModalInputEvent::Submit);
            }
            32..=126 => events.push(ModalInputEvent::Append(char::from(byte).to_string())),
            0x80..=0xff => {
                events.push(ModalInputEvent::Completeness(false));
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
        events: &mut Vec<ModalInputEvent>,
    ) {
        match scalar.push(byte) {
            Ok(None) => {
                self.phase = Phase::Utf8 {
                    scalar,
                    discard,
                    deadline: now + WAIT,
                }
            }
            Ok(Some(character)) => {
                self.finish_frame(now, events);
                if self.editing && !discard {
                    events.push(ModalInputEvent::Append(character.to_string()));
                }
            }
            Err(()) => {
                if self.editing {
                    events.push(ModalInputEvent::Fault);
                }
                self.finish_frame(now, events);
                if byte == 0x1b {
                    if self.editing {
                        events.push(ModalInputEvent::Completeness(false));
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
        events: &mut Vec<ModalInputEvent>,
    ) {
        let final_byte = (0x40..=0x7e).contains(&byte);
        if !sequence.drain {
            let valid = final_byte
                || (0x20..=0x2f).contains(&byte)
                || (!sequence.intermediate && (0x30..=0x3f).contains(&byte));
            if sequence.length == MAX_SEQUENCE || !valid {
                sequence.drain = true;
                sequence.deadline = None;
                if self.editing {
                    events.push(ModalInputEvent::Fault);
                }
            } else {
                sequence.bytes[sequence.length] = byte;
                sequence.length += 1;
                sequence.intermediate |= (0x20..=0x2f).contains(&byte);
                sequence.deadline = now.checked_add(WAIT);
            }
        }
        if final_byte {
            if !sequence.drain && sequence.is_paste_opener() {
                let mut paste =
                    Paste::new(self.capacity, remaining, sequence.bytes[0] == 0x9b, now);
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

    fn finish_frame(&mut self, _now: Instant, events: &mut Vec<ModalInputEvent>) {
        if self.editing {
            self.phase = Phase::Text;
            events.push(ModalInputEvent::Completeness(true));
        } else {
            self.release_tail();
        }
    }

    fn escape(&mut self, now: Instant, events: &mut Vec<ModalInputEvent>) {
        if let Some(deadline) = now.checked_add(WAIT) {
            self.phase = Phase::Escape(deadline);
        } else {
            self.suppress(events);
        }
    }

    fn suppress(&mut self, events: &mut Vec<ModalInputEvent>) {
        if self.editing {
            events.push(ModalInputEvent::Cancel);
        }
        self.editing = false;
        self.phase = Phase::Suppressed;
        events.push(ModalInputEvent::InputFailure);
    }
}

#[cfg(test)]
mod tests;
