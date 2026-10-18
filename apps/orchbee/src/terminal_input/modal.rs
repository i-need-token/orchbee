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
    #[cfg(windows)]
    Bytes1024,
}

impl ModalCapacity {
    fn bound(self, remaining: usize) -> usize {
        remaining.min(match self {
            Self::Scalars512 => 512,
            Self::Bytes4096 => 4096,
            #[cfg(windows)]
            Self::Bytes1024 => 1024,
        })
    }

    fn cost(self, character: char) -> usize {
        match self {
            Self::Scalars512 => 1,
            Self::Bytes4096 => character.len_utf8(),
            #[cfg(windows)]
            Self::Bytes1024 => character.len_utf8(),
        }
    }
}

/// The frozen typed-input contract shared with U5's bridge: `Append` is one
/// scalar of typed text, `Paste` one atomic pasted span, and the rest the
/// editing keys a single-line field consumes. U9 merges this enum with the
/// authoritative decoder; the names and meanings do not diverge.
#[derive(Debug, Eq, PartialEq)]
pub(super) enum ModalInputEvent {
    NextField,
    PreviousChoice,
    NextChoice,
    Append(String),
    Paste(String),
    Left,
    Right,
    Home,
    End,
    Delete,
    Backspace,
    WordBackspace,
    Clear,
    Fault,
    Completeness(bool),
    Focus(bool),
    HostReply(super::HostReply),
    Mouse(super::MouseEvent),
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
    Mouse {
        bytes: [u8; 3],
        length: usize,
        discard: bool,
        deadline: Instant,
    },
    GraphicsStart(Instant),
    Graphics(Box<crate::terminal_graphics::ReplyString>),
    /// A possible OSC 10/11 response; unrelated escape text returns to ordinary modal decoding.
    Osc {
        introducer: Vec<u8>,
        frame: crate::terminal_host_appearance::OscReplyFrame,
        selector_deadline: Option<Instant>,
    },
    /// A response-shaped OSC exceeded its bound or deadline and owns a bounded tail.
    OscDrain {
        drain: crate::terminal_host_appearance::OscDrain,
    },
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

    /// A sequence opened directly by a C1 initiator (`0x9B` CSI, `0x8F` SS3).
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

    fn host_metadata(&self) -> bool {
        let raw = &self.bytes[..self.length];
        let body = if raw.starts_with(b"\x1b[") {
            &raw[2..]
        } else if raw.starts_with(b"\x9b") {
            &raw[1..]
        } else {
            return false;
        };
        super::decoder::csi_host_metadata_shaped(body)
    }

    fn is_paste_opener(&self) -> bool {
        matches!(&self.bytes[..self.length], b"\x1b[200~" | b"\x9b200~")
    }
}

pub(super) struct ModalDecoder {
    capacity: ModalCapacity,
    phase: Phase,
    editing: bool,
    choice_keys: bool,
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
            choice_keys: false,
            swallow_lf: false,
            released_at: None,
        }
    }
}

impl ModalDecoder {
    pub(super) fn choice_keys(&mut self, enabled: bool) {
        self.choice_keys = enabled;
    }
    pub(super) fn set_capacity(&mut self, capacity: ModalCapacity) {
        self.capacity = capacity;
    }
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
    /// before the first byte of that packet. Ready bytes precede idle timeouts.
    pub(super) fn begin_packet(&mut self, now: Instant) {
        self.end_released(now);
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
            Phase::Mouse { discard, .. } => *discard = true,
            Phase::GraphicsStart(_)
            | Phase::Graphics(_)
            | Phase::Osc { .. }
            | Phase::OscDrain { .. } => {}
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
                    if self.editing && !sequence.host_metadata() {
                        events.push(ModalInputEvent::Fault);
                    }
                }
            }
            Phase::GraphicsStart(deadline) if now >= *deadline => {
                self.finish_frame(now, &mut events);
            }
            Phase::Graphics(reply) => reply.tick(now),
            Phase::Osc {
                frame,
                selector_deadline,
                ..
            } if frame.target().is_none()
                && selector_deadline.is_some_and(|deadline| now >= deadline) =>
            {
                let (introducer, selector) =
                    match std::mem::replace(&mut self.phase, Phase::Suppressed) {
                        Phase::Osc {
                            introducer, frame, ..
                        } => (introducer, frame.selector_bytes().to_vec()),
                        _ => unreachable!("the OSC selector phase was matched immediately above"),
                    };
                self.replay_osc_candidate(&introducer, &selector, now, &mut events);
            }
            Phase::Osc { frame, .. } if frame.expired(now) => {
                if frame.target().is_some() {
                    self.phase = Phase::OscDrain {
                        drain: frame.drain(now),
                    };
                } else {
                    let (introducer, selector) =
                        match std::mem::replace(&mut self.phase, Phase::Suppressed) {
                            Phase::Osc {
                                introducer, frame, ..
                            } => (introducer, frame.selector_bytes().to_vec()),
                            _ => unreachable!("the OSC phase was matched immediately above"),
                        };
                    self.replay_osc_candidate(&introducer, &selector, now, &mut events);
                }
            }
            Phase::OscDrain { drain } if drain.expired(now) => {
                self.finish_frame(now, &mut events);
            }
            Phase::Mouse {
                discard, deadline, ..
            } if now >= *deadline => {
                // Retain the fixed three-byte frame across timeout/cancellation;
                // late coordinates must never become printable modal input.
                *discard = true;
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
        let mut events = Vec::new();
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
                        0x9d => {
                            self.phase = Phase::Osc {
                                introducer: vec![byte],
                                frame: crate::terminal_host_appearance::OscReplyFrame::new(now),
                                selector_deadline: None,
                            }
                        }
                        0x9b | 0x8f => self.phase = Phase::Sequence(Sequence::c1(byte, now)),
                        0x80..=0xff => {
                            self.scalar_byte(Scalar::default(), true, byte, now, &mut events)
                        }
                        _ => {}
                    }
                }
                Phase::Escape(deadline) => match byte {
                    b'_' => self.phase = Phase::GraphicsStart(now + WAIT),
                    b']' => {
                        self.phase = Phase::Osc {
                            introducer: vec![0x1b, byte],
                            frame: crate::terminal_host_appearance::OscReplyFrame::new(now),
                            selector_deadline: Some(deadline),
                        }
                    }
                    b'[' | b'O' => self.phase = Phase::Sequence(Sequence::new(byte, now)),
                    0x9b | 0x8f => self.phase = Phase::Sequence(Sequence::c1(byte, now)),
                    0x80..=0xff => {
                        self.scalar_byte(Scalar::default(), true, byte, now, &mut events)
                    }
                    _ => self.finish_frame(now, &mut events),
                },
                Phase::GraphicsStart(_) => {
                    if byte == b'G' {
                        self.phase = Phase::Graphics(Box::new(
                            crate::terminal_graphics::ReplyString::new(now, WAIT),
                        ));
                    } else {
                        self.finish_frame(now, &mut events);
                        pending = Some(byte);
                    }
                }
                Phase::Graphics(mut reply) => {
                    if let Some(completed) = reply.push(byte, now) {
                        self.finish_frame(now, &mut events);
                        if let Some(reply) = completed {
                            events.push(ModalInputEvent::HostReply(super::HostReply::Graphics(
                                reply,
                            )));
                        }
                    } else {
                        self.phase = Phase::Graphics(reply);
                    }
                }
                Phase::Osc {
                    introducer,
                    mut frame,
                    selector_deadline,
                } => {
                    if frame.target().is_none()
                        && selector_deadline.is_some_and(|deadline| now >= deadline)
                    {
                        let selector = frame.selector_bytes().to_vec();
                        self.replay_osc_candidate(&introducer, &selector, now, &mut events);
                        pending = Some(byte);
                    } else {
                        match frame.push(byte) {
                            crate::terminal_host_appearance::OscStep::Pending => {
                                self.phase = Phase::Osc {
                                    introducer,
                                    selector_deadline: if frame.target().is_some() {
                                        None
                                    } else {
                                        selector_deadline
                                    },
                                    frame,
                                };
                            }
                            crate::terminal_host_appearance::OscStep::NotColorReply(selector) => {
                                self.replay_osc_candidate(&introducer, &selector, now, &mut events);
                            }
                            crate::terminal_host_appearance::OscStep::Complete(reply) => {
                                self.finish_frame(now, &mut events);
                                events.push(ModalInputEvent::HostReply(
                                    super::HostReply::Appearance(reply),
                                ));
                            }
                            crate::terminal_host_appearance::OscStep::OverBound(target) => {
                                events.push(ModalInputEvent::HostReply(
                                    super::HostReply::Appearance(
                                        crate::terminal_host_appearance::ColorReply {
                                            target,
                                            rgb: None,
                                        },
                                    ),
                                ));
                                self.phase = Phase::OscDrain {
                                    drain: frame.drain(now),
                                };
                            }
                        }
                    }
                }
                Phase::OscDrain { mut drain } => match drain.push(byte, now) {
                    crate::terminal_host_appearance::OscDrainStep::Pending => {
                        self.phase = Phase::OscDrain { drain };
                    }
                    crate::terminal_host_appearance::OscDrainStep::Finished => {
                        self.finish_frame(now, &mut events);
                    }
                    crate::terminal_host_appearance::OscDrainStep::Reprocess => {
                        self.finish_frame(now, &mut events);
                        pending = Some(byte);
                    }
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
                Phase::Mouse {
                    mut bytes,
                    length,
                    discard,
                    ..
                } => {
                    bytes[length] = byte;
                    if length + 1 == bytes.len() {
                        self.finish_frame(now, &mut events);
                        if self.editing
                            && !discard
                            && let Some(event) = super::mouse::decode_x10(&bytes)
                        {
                            events.push(ModalInputEvent::Mouse(event));
                        }
                    } else {
                        self.phase = Phase::Mouse {
                            bytes,
                            length: length + 1,
                            discard,
                            deadline: now + WAIT,
                        };
                    }
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
                                .push(text.map_or(ModalInputEvent::Fault, ModalInputEvent::Paste));
                        }
                    }
                    PasteStep::Drained => self.finish_frame(now, &mut events),
                },
                Phase::Suppressed | Phase::Idle => self.phase = Phase::Suppressed,
            }
        }
        (true, events)
    }

    fn text_byte(&mut self, byte: u8, now: Instant, events: &mut Vec<ModalInputEvent>) {
        self.phase = Phase::Text;
        match byte {
            b'\t' => events.push(ModalInputEvent::NextField),
            0 if self.capacity == ModalCapacity::Bytes4096 => {
                events.push(ModalInputEvent::Append("\0".to_owned()));
            }
            0x1b => {
                events.push(ModalInputEvent::Completeness(false));
                self.escape(now, events);
            }
            0x9b | 0x8f => {
                events.push(ModalInputEvent::Completeness(false));
                self.phase = Phase::Sequence(Sequence::c1(byte, now));
            }
            0x9d => {
                events.push(ModalInputEvent::Completeness(false));
                self.phase = Phase::Osc {
                    introducer: vec![byte],
                    frame: crate::terminal_host_appearance::OscReplyFrame::new(now),
                    selector_deadline: None,
                };
            }
            // Ctrl+A/Ctrl+E collapse onto Home/End at the input boundary, the
            // same fold U5's bridge performs.
            1 => events.push(ModalInputEvent::Home),
            5 => events.push(ModalInputEvent::End),
            8 | 127 => events.push(ModalInputEvent::Backspace),
            21 => events.push(ModalInputEvent::Clear),
            23 => events.push(ModalInputEvent::WordBackspace),
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
                if self.editing && !sequence.host_metadata() {
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
            if !sequence.drain && matches!(&sequence.bytes[..sequence.length], b"\x1b[M" | b"\x9bM")
            {
                self.phase = Phase::Mouse {
                    bytes: [0; 3],
                    length: 0,
                    discard: !self.editing,
                    deadline: now + WAIT,
                };
            } else if !sequence.drain && sequence.is_paste_opener() {
                let mut paste =
                    Paste::new(self.capacity, remaining, sequence.bytes[0] == 0x9b, now);
                if !self.editing {
                    paste.discard();
                }
                self.phase = Phase::Paste(paste);
            } else {
                self.finish_frame(now, events);
                let bytes = &sequence.bytes[..sequence.length];
                let body = if bytes[0] == 0x9b {
                    Some(&bytes[1..])
                } else {
                    bytes.strip_prefix(b"\x1b[")
                };
                if !sequence.drain
                    && let Some(reply) = body.and_then(super::decoder::csi_host_reply)
                {
                    events.push(ModalInputEvent::HostReply(reply));
                    return;
                }
                if body.is_some_and(super::decoder::csi_host_metadata_shaped) {
                    return;
                }
                if !sequence.drain
                    && self.editing
                    && let Some(event) = body
                        .and_then(|body| body.strip_prefix(b"<"))
                        .and_then(super::mouse::decode_sgr)
                {
                    events.push(ModalInputEvent::Mouse(event));
                    return;
                }
                if !sequence.drain
                    && let Some(event) =
                        sequence_edit(&sequence.bytes[..sequence.length], self.choice_keys)
                    && (self.editing || matches!(event, ModalInputEvent::Focus(_)))
                {
                    events.push(event);
                }
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

    fn replay_osc_candidate(
        &mut self,
        _introducer: &[u8],
        selector: &[u8],
        now: Instant,
        events: &mut Vec<ModalInputEvent>,
    ) {
        self.finish_frame(now, events);
        if self.editing {
            for byte in selector.iter().copied() {
                self.text_byte(byte, now, events);
            }
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

/// Maps a complete, valid CSI/SS3 frame to its editing event. Only the exact
/// unmodified forms resolve; a parameterized or modified frame (Ctrl+arrows,
/// `3;5~`, kitty-style `u` events, unbound Up/Down/Insert/PageUp/PageDown/F1-F4)
/// returns `None` so it is consumed whole — never simplified to a plain key.
fn sequence_edit(bytes: &[u8], choices: bool) -> Option<ModalInputEvent> {
    Some(match bytes {
        b"\x1b[I" | b"\x9bI" => ModalInputEvent::Focus(true),
        b"\x1b[O" | b"\x9bO" => ModalInputEvent::Focus(false),
        b"\x1b[A" | b"\x9bA" | b"\x1bOA" | b"\x8fA" if choices => ModalInputEvent::PreviousChoice,
        b"\x1b[B" | b"\x9bB" | b"\x1bOB" | b"\x8fB" if choices => ModalInputEvent::NextChoice,
        b"\x1b[D" | b"\x9bD" | b"\x1bOD" | b"\x8fD" => ModalInputEvent::Left,
        b"\x1b[C" | b"\x9bC" | b"\x1bOC" | b"\x8fC" => ModalInputEvent::Right,
        b"\x1b[H" | b"\x9bH" | b"\x1bOH" | b"\x8fH" | b"\x1b[1~" | b"\x9b1~" | b"\x1b[7~"
        | b"\x9b7~" => ModalInputEvent::Home,
        b"\x1b[F" | b"\x9bF" | b"\x1bOF" | b"\x8fF" | b"\x1b[4~" | b"\x9b4~" | b"\x1b[8~"
        | b"\x9b8~" => ModalInputEvent::End,
        b"\x1b[3~" | b"\x9b3~" => ModalInputEvent::Delete,
        _ => return None,
    })
}

#[cfg(test)]
mod tests;
