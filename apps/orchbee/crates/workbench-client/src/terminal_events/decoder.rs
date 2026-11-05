//! The legacy host decoder: one bounded state machine that turns the raw host
//! byte stream into `DecodedItem`s — typed `KeyEvent`s, atomic paste payloads,
//! or `Nondecision` for complete sequences that are not key events. Forwardable
//! items own their source bytes across reads; rejected paste retains no payload
//! and silently drains its frame before ordinary decoding resumes.
//!
//! Ownership semantics: a prefix follower that decodes to any item — key or
//! non-decision — is exactly one event. UTF-8 continuation bytes and CSI tails
//! therefore can never reach the pane or a binding as partial input.

use std::time::{Duration, Instant};

use super::mouse::{self, MouseEvent};
use super::utf8::{Continuation, continuation_count};
use crate::terminal_events::keys::{KeyCode, KeyEvent, KeyKind, Modifiers};

/// One decoded input item; rejected paste retains no raw bytes. `owned` marks an
/// item that completes a frame which began inside a captured packet, so
/// the dispatcher consumes it instead of forwarding or deciding it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedItem {
    pub kind: DecodedKind,
    pub raw: Box<[u8]>,
    pub(crate) owned: bool,
}

impl DecodedItem {
    fn key(event: KeyEvent, raw: &[u8], owned: bool) -> Self {
        Self {
            kind: DecodedKind::Key(event),
            raw: raw.into(),
            owned,
        }
    }

    fn nondecision(raw: &[u8], owned: bool) -> Self {
        Self {
            kind: DecodedKind::Nondecision,
            raw: raw.into(),
            owned,
        }
    }

    fn ignored(raw: &[u8], owned: bool) -> Self {
        Self {
            kind: DecodedKind::Ignored,
            raw: raw.into(),
            owned,
        }
    }

    fn rejected_paste(owned: bool) -> Self {
        Self {
            kind: DecodedKind::RejectedPaste,
            raw: Box::default(),
            owned,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedKind {
    Key(KeyEvent),
    /// One bounded UTF-8 bracketed-paste run. The payload never enters binding matching.
    Paste {
        text: String,
    },
    /// One malformed, over-bound or timed-out paste refusal, with no forwardable bytes.
    RejectedPaste,
    /// A complete, well-formed host mouse report (SGR `ESC[<…M/m`, or the
    /// defensive X10 `ESC[M`+3 form). Mouse reports are host→client data:
    /// the dispatcher routes them to hit-testing or pane passthrough and
    /// never forwards their raw bytes as pending input.
    Mouse(MouseEvent),
    /// Host focus metadata, never terminal input or a prefix key.
    Focus(bool),
    /// Host query metadata cannot participate in key ownership or human activity.
    HostReply(super::HostReply),
    /// A mouse-shaped sequence that failed validation or never completed —
    /// malformed SGR parameters, an X10 body aborted or timed out, an
    /// over-bound `<`-prefixed run. These bytes are the host's pointer
    /// protocol: consumed atomically and dropped, never reinterpreted as
    /// keys and never forwarded raw to the pane.
    Ignored,
    /// A complete sequence that is not a key event (unknown CSI/SS3, stray
    /// paste terminator, malformed UTF-8 run). Consumed atomically and kept
    /// in pending order — input the client does not claim still reaches the
    /// pane.
    Nondecision,
}

const CSI_BOUND: usize = 64;
const SS3_BOUND: usize = 8;
const PASTE_RAW_BOUND: usize = 4 * 1024 * 1024;
const PASTE_TEXT_BOUND: usize = 256 * 1024;
/// Generic sequence drains preserve passthrough order in bounded chunks with a small suffix.
/// Rejected paste has its own payload-free state and never enters this path.
const DISCARD_CHUNK: usize = 64;
const DISCARD_TAIL: usize = 6;
const PASTE_END_ESC: &[u8] = b"\x1b[201~";
const PASTE_END_C1: &[u8] = b"\x9b201~";

/// Terminator-matching state for a paste run or its drained tail. It is a
/// prefix-progress counter over the opener-notation terminator plus the
/// UTF-8 continuation countdown, so the match survives rejection without retaining
/// payload bytes, and a `\x9b` consumed as
/// a UTF-8 continuation is never a C1 terminator lead.
#[derive(Debug)]
struct PasteEnd {
    term: &'static [u8],
    prefix: usize,
    utf8: Continuation,
}

impl PasteEnd {
    fn new(c1: bool) -> Self {
        Self {
            term: if c1 { PASTE_END_C1 } else { PASTE_END_ESC },
            prefix: 0,
            utf8: Continuation::default(),
        }
    }

    /// Feeds one payload byte; returns true when the byte completes the
    /// real terminator.
    fn push(&mut self, byte: u8) -> bool {
        let continuation = self.utf8.push(byte);
        if self.prefix > 0 && byte == self.term[self.prefix] {
            self.prefix += 1;
            return self.prefix == self.term.len();
        }
        // A `0x9b` that is a UTF-8 continuation byte is payload, never the
        // C1 terminator lead; `0x1b` can never be a continuation byte.
        self.prefix =
            usize::from(byte == self.term[0] && !(continuation && self.term == PASTE_END_C1));
        false
    }
}

#[derive(Debug)]
enum Drain {
    Csi,
    /// Mouse and cell-query metadata retain their tail through overflow/timeout.
    CsiMetadata,
    Ss3,
    /// An X10 frame whose bounded wait expired mid-body: exactly
    /// `remaining` more body bytes stay owned so a late tail can never
    /// surface as fresh keys.
    X10 {
        remaining: u8,
    },
}

#[derive(Debug)]
enum State {
    Ground,
    /// `ESC` seen; resolves as a lone Esc on `deadline` or as Alt+<key> /
    /// CSI / SS3 when the next byte arrives.
    Esc {
        deadline: Instant,
        owned: bool,
    },
    /// CSI bytes (after `ESC[` or raw `0x9b`). `raw` starts at the leading
    /// ESC and collects through the final byte.
    Csi {
        raw: Vec<u8>,
        deadline: Instant,
        owned: bool,
    },
    /// SS3 bytes (after `ESCO` or raw `0x8f`; `0x8d` is RI, never SS3).
    Ss3 {
        raw: Vec<u8>,
        deadline: Instant,
        owned: bool,
    },
    GraphicsStart {
        deadline: Instant,
        owned: bool,
    },
    Graphics {
        reply: Box<crate::terminal_events::graphics::ReplyString>,
        owned: bool,
    },
    /// A possible OSC 10/11 response; selector bytes stay bounded until ownership is known.
    Osc {
        introducer: Vec<u8>,
        frame: crate::terminal_events::appearance::OscReplyFrame,
        selector_deadline: Option<Instant>,
        owned: bool,
    },
    /// A targeted appearance string expired mid-frame; drain a bounded tail through BEL/ST.
    OscDrain {
        drain: crate::terminal_events::appearance::OscDrain,
        owned: bool,
    },
    /// An in-progress UTF-8 scalar; `raw` includes an optional leading ESC
    /// (Alt-modified scalar) and `need` counts remaining continuation bytes.
    Utf8 {
        raw: Vec<u8>,
        need: u8,
        alt: bool,
        deadline: Instant,
        owned: bool,
    },
    /// A truncated scalar whose bounded wait expired keeps exactly its
    /// remaining continuation bytes under the frame's provenance — a late
    /// `0x9b`/`0x8f` tail byte is never re-decoded as a fresh control
    /// sequence. A non-continuation byte ends the frame and is reprocessed
    /// under its own provenance. Same semantics as `ModalDecoder`'s
    /// `Utf8Tail`.
    Utf8Tail {
        remaining: u8,
        deadline: Instant,
        owned: bool,
    },
    /// Inside a bracketed-paste run. `raw` holds the whole run for
    /// passthrough; `end` matches the terminator by the opener's notation
    /// and survives the cap/timeout drain boundary.
    Paste {
        raw: Vec<u8>,
        end: PasteEnd,
        deadline: Instant,
        owned: bool,
    },
    /// A rejected paste retains only its terminator matcher and captured provenance.
    /// No payload, partial terminator or later timeout can emit another item.
    PasteTail {
        end: PasteEnd,
        owned: bool,
    },
    /// Inside an X10 mouse report: the `ESC[M`/`9b M` head already sits in
    /// `raw`, and exactly `remaining` body bytes (always ≤ 3) complete the
    /// frame atomically across reads.
    X10 {
        raw: Vec<u8>,
        remaining: u8,
        deadline: Instant,
        owned: bool,
    },
    /// A frame that overflowed its bound or timed out keeps ownership of
    /// its tail: bytes are consumed until a reliable boundary (final byte,
    /// or an ESC/C1 abort which is reprocessed as fresh input) and emitted
    /// as bounded non-decision chunks.
    Discard {
        raw: Vec<u8>,
        drain: Drain,
        owned: bool,
    },
}

#[derive(Debug)]
pub struct LegacyDecoder {
    state: State,
    /// The bounded wait shared by the lone-ESC disambiguation and every
    /// truncated frame: a frame's deadline refreshes per byte, so only a
    /// real gap longer than this ends the wait and drains the tail.
    wait: Duration,
}

impl Default for LegacyDecoder {
    fn default() -> Self {
        Self::new(Duration::from_millis(50))
    }
}

impl LegacyDecoder {
    /// A surface switch claims a frame already in flight without discarding its terminator.
    pub(crate) fn capture_pending(&mut self) {
        match &mut self.state {
            State::Ground => {}
            State::Esc { owned, .. }
            | State::Csi { owned, .. }
            | State::Ss3 { owned, .. }
            | State::GraphicsStart { owned, .. }
            | State::Graphics { owned, .. }
            | State::Osc { owned, .. }
            | State::OscDrain { owned, .. }
            | State::Utf8 { owned, .. }
            | State::Utf8Tail { owned, .. }
            | State::Paste { owned, .. }
            | State::PasteTail { owned, .. }
            | State::X10 { owned, .. }
            | State::Discard { owned, .. } => *owned = true,
        }
    }

    pub const fn new(wait: Duration) -> Self {
        Self {
            state: State::Ground,
            wait,
        }
    }

    /// Feeds one byte; returns any items completed by it. `owned` marks
    /// bytes that arrive inside a captured packet: a frame they open keeps
    /// its captured provenance even when it completes in a later read. An
    /// item may surface several reads after its first byte — the decoder
    /// owns the bytes in the meantime.
    pub fn push(&mut self, byte: u8, now: Instant, owned: bool) -> Vec<DecodedItem> {
        let mut items = Vec::new();
        self.feed(byte, now, owned, &mut items);
        items
    }

    /// Resolves bounded waits: a lone ESC becomes an Esc event; a truncated
    /// CSI/SS3 emits its bytes and drains the frame tail; a truncated UTF-8
    /// scalar emits as non-decision. At most one item per call.
    pub fn tick(&mut self, now: Instant) -> Vec<DecodedItem> {
        let mut items = Vec::new();
        match std::mem::replace(&mut self.state, State::Ground) {
            State::Esc { deadline, owned } if now >= deadline => {
                items.push(DecodedItem::key(
                    KeyEvent::press(KeyCode::Esc, Modifiers::NONE),
                    &[0x1b],
                    owned,
                ));
            }
            State::GraphicsStart { deadline, owned } if now >= deadline => {
                items.push(DecodedItem::key(
                    KeyEvent::press(KeyCode::Char('_'), Modifiers::NONE.with_alt()),
                    b"\x1b_",
                    owned,
                ));
            }
            State::Graphics { mut reply, owned } => {
                reply.tick(now);
                self.state = State::Graphics { reply, owned };
            }
            State::Osc {
                introducer,
                frame,
                selector_deadline,
                owned,
            } => {
                if frame.target().is_none()
                    && selector_deadline.is_some_and(|deadline| now >= deadline)
                {
                    replay_osc_candidate(
                        self,
                        &introducer,
                        frame.selector_bytes(),
                        now,
                        owned,
                        &mut items,
                    );
                } else if frame.expired(now) {
                    if frame.target().is_some() {
                        self.state = State::OscDrain {
                            drain: frame.drain(now),
                            owned,
                        };
                    } else {
                        replay_osc_candidate(
                            self,
                            &introducer,
                            frame.selector_bytes(),
                            now,
                            owned,
                            &mut items,
                        );
                    }
                } else {
                    self.state = State::Osc {
                        introducer,
                        frame,
                        selector_deadline,
                        owned,
                    };
                }
            }
            State::OscDrain { drain, owned } => {
                if !drain.expired(now) {
                    self.state = State::OscDrain { drain, owned };
                }
            }
            State::Csi {
                raw,
                deadline,
                owned,
            } if now >= deadline => {
                // Query and pointer metadata own their tail even after a bounded wait.
                let metadata = csi_metadata_shaped(&raw);
                items.push(if metadata {
                    DecodedItem::ignored(&raw, owned)
                } else {
                    DecodedItem::nondecision(&raw, owned)
                });
                self.state = State::Discard {
                    raw: Vec::new(),
                    drain: if metadata {
                        Drain::CsiMetadata
                    } else {
                        Drain::Csi
                    },
                    owned,
                };
            }
            State::Ss3 {
                raw,
                deadline,
                owned,
            } if now >= deadline => {
                items.push(DecodedItem::nondecision(&raw, owned));
                self.state = State::Discard {
                    raw: Vec::new(),
                    drain: Drain::Ss3,
                    owned,
                };
            }
            State::Utf8 {
                raw,
                need,
                deadline,
                owned,
                ..
            } if now >= deadline => {
                items.push(DecodedItem::nondecision(&raw, owned));
                // The scalar's remaining continuation bytes stay owned by
                // this frame — they are never re-decoded as fresh input.
                self.state = State::Utf8Tail {
                    remaining: need,
                    deadline: now + self.wait,
                    owned,
                };
            }
            // The late tail never arrived inside its bounded wait; the
            // scalar residue was already emitted, nothing left to forward.
            State::Utf8Tail { deadline, .. } if now >= deadline => {}
            // Reject once and release the payload. The in-flight terminator
            // match survives, so late fragments cannot become fresh input.
            State::Paste {
                end,
                deadline,
                owned,
                ..
            } if now >= deadline => {
                items.push(DecodedItem::rejected_paste(owned));
                self.state = State::PasteTail { end, owned };
            }
            // An X10 body that never completed is pointer protocol —
            // the partial frame is dropped and its exact remainder drains
            // bounded to three bytes like the frame itself.
            State::X10 {
                raw,
                remaining,
                deadline,
                owned,
            } if now >= deadline => {
                items.push(DecodedItem::ignored(&raw, owned));
                self.state = State::Discard {
                    raw: Vec::new(),
                    drain: Drain::X10 { remaining },
                    owned,
                };
            }
            state => self.state = state,
        }
        items
    }

    fn feed(&mut self, byte: u8, now: Instant, owned: bool, items: &mut Vec<DecodedItem>) {
        let mut pending = Some(byte);
        while let Some(byte) = pending.take() {
            match std::mem::replace(&mut self.state, State::Ground) {
                State::Ground => self.feed_ground(byte, now, owned, items),
                State::Esc {
                    deadline,
                    owned: frame_owned,
                    ..
                } => self.feed_esc(byte, now, deadline, frame_owned, owned, items),
                State::Csi { mut raw, owned, .. } => {
                    if matches!(byte, 0x1b | 0x9b | 0x8f) {
                        // ESC/C1 aborts the sequence; the abort byte is
                        // reprocessed as fresh input. Host metadata is dropped atomically.
                        items.push(if csi_metadata_shaped(&raw) {
                            DecodedItem::ignored(&raw, owned)
                        } else {
                            DecodedItem::nondecision(&raw, owned)
                        });
                        pending = Some(byte);
                    } else {
                        raw.push(byte);
                        if (0x40..=0x7e).contains(&byte) {
                            self.finish_csi(&raw, now, owned, items);
                        } else if raw.len() >= CSI_BOUND {
                            let metadata = csi_metadata_shaped(&raw);
                            items.push(if metadata {
                                DecodedItem::ignored(&raw, owned)
                            } else {
                                DecodedItem::nondecision(&raw, owned)
                            });
                            self.state = State::Discard {
                                raw: Vec::new(),
                                drain: if metadata {
                                    Drain::CsiMetadata
                                } else {
                                    Drain::Csi
                                },
                                owned,
                            };
                        } else {
                            self.state = State::Csi {
                                raw,
                                deadline: now + self.wait,
                                owned,
                            };
                        }
                    }
                }
                State::GraphicsStart {
                    owned: frame_owned, ..
                } => {
                    if byte == b'G' {
                        self.state = State::Graphics {
                            reply: Box::new(crate::terminal_events::graphics::ReplyString::new(
                                now, self.wait,
                            )),
                            owned: frame_owned,
                        };
                    } else {
                        items.push(DecodedItem::key(
                            KeyEvent::press(KeyCode::Char('_'), Modifiers::NONE.with_alt()),
                            b"\x1b_",
                            frame_owned,
                        ));
                        pending = Some(byte);
                    }
                }
                State::Graphics { mut reply, owned } => {
                    if let Some(completed) = reply.push(byte, now) {
                        items.push(DecodedItem {
                            kind: completed.map_or(DecodedKind::Ignored, |reply| {
                                DecodedKind::HostReply(super::HostReply::Graphics(reply))
                            }),
                            raw: Box::default(),
                            owned,
                        });
                    } else {
                        self.state = State::Graphics { reply, owned };
                    }
                }
                State::Osc {
                    introducer,
                    mut frame,
                    selector_deadline,
                    owned,
                } => {
                    if frame.target().is_some() && frame.expired(now) {
                        self.state = State::OscDrain {
                            drain: frame.drain(now),
                            owned,
                        };
                        pending = Some(byte);
                    } else if frame.target().is_none()
                        && (frame.expired(now)
                            || selector_deadline.is_some_and(|deadline| now >= deadline))
                    {
                        replay_osc_candidate(
                            self,
                            &introducer,
                            frame.selector_bytes(),
                            now,
                            owned,
                            items,
                        );
                        pending = Some(byte);
                    } else {
                        match frame.push(byte) {
                            crate::terminal_events::appearance::OscStep::Pending => {
                                self.state = State::Osc {
                                    introducer,
                                    selector_deadline: if frame.target().is_some() {
                                        None
                                    } else {
                                        selector_deadline
                                    },
                                    frame,
                                    owned,
                                };
                            }
                            crate::terminal_events::appearance::OscStep::NotColorReply(
                                selector,
                            ) => {
                                replay_osc_candidate(
                                    self,
                                    &introducer,
                                    &selector,
                                    now,
                                    owned,
                                    items,
                                );
                            }
                            crate::terminal_events::appearance::OscStep::OverBound(target) => {
                                items.push(DecodedItem {
                                    kind: DecodedKind::HostReply(super::HostReply::Appearance(
                                        crate::terminal_events::appearance::ColorReply {
                                            target,
                                            rgb: None,
                                        },
                                    )),
                                    raw: Box::default(),
                                    owned,
                                });
                                self.state = State::OscDrain {
                                    drain: frame.drain(now),
                                    owned,
                                };
                            }
                            crate::terminal_events::appearance::OscStep::Complete(reply) => {
                                items.push(DecodedItem {
                                    kind: DecodedKind::HostReply(super::HostReply::Appearance(
                                        reply,
                                    )),
                                    raw: Box::default(),
                                    owned,
                                });
                            }
                        }
                    }
                }
                State::OscDrain { mut drain, owned } => match drain.push(byte, now) {
                    crate::terminal_events::appearance::OscDrainStep::Pending => {
                        self.state = State::OscDrain { drain, owned };
                    }
                    crate::terminal_events::appearance::OscDrainStep::Finished => {}
                    crate::terminal_events::appearance::OscDrainStep::Reprocess => {
                        pending = Some(byte);
                    }
                },
                State::Ss3 { mut raw, owned, .. } => {
                    if matches!(byte, 0x1b | 0x9b | 0x8f) {
                        items.push(DecodedItem::nondecision(&raw, owned));
                        pending = Some(byte);
                    } else {
                        raw.push(byte);
                        if self.finish_ss3(&raw, owned, items) {
                            // finished
                        } else if raw.len() >= SS3_BOUND {
                            items.push(DecodedItem::nondecision(&raw, owned));
                            self.state = State::Discard {
                                raw: Vec::new(),
                                drain: Drain::Ss3,
                                owned,
                            };
                        } else {
                            self.state = State::Ss3 {
                                raw,
                                deadline: now + self.wait,
                                owned,
                            };
                        }
                    }
                }
                State::Utf8 {
                    mut raw,
                    need,
                    alt,
                    owned,
                    ..
                } => {
                    if (0x80..=0xbf).contains(&byte) && need > 0 {
                        raw.push(byte);
                        let need = need - 1;
                        if need == 0 {
                            self.finish_utf8(&raw, alt, owned, items);
                        } else {
                            self.state = State::Utf8 {
                                raw,
                                need,
                                alt,
                                deadline: now + self.wait,
                                owned,
                            };
                        }
                    } else {
                        items.push(DecodedItem::nondecision(&raw, owned));
                        pending = Some(byte);
                    }
                }
                State::Utf8Tail {
                    remaining, owned, ..
                } => {
                    if (0x80..=0xbf).contains(&byte) {
                        // A late tail byte is part of the dead scalar's frame
                        // — owned, never re-decoded as a control sequence.
                        items.push(DecodedItem::nondecision(&[byte], owned));
                        let remaining = remaining - 1;
                        if remaining > 0 {
                            self.state = State::Utf8Tail {
                                remaining,
                                deadline: now + self.wait,
                                owned,
                            };
                        }
                    } else {
                        // Not a continuation: the tail ends here and the byte
                        // is reprocessed under its own provenance.
                        pending = Some(byte);
                    }
                }
                State::Paste {
                    mut raw,
                    mut end,
                    owned,
                    ..
                } => {
                    raw.push(byte);
                    if end.push(byte) {
                        // A complete frame is admitted only as bounded UTF-8 text.
                        let payload_end = raw.len() - end.term.len();
                        let payload = &raw[paste_marker_len(&raw)..payload_end];
                        let text = if payload.len() <= PASTE_TEXT_BOUND {
                            String::from_utf8(payload.to_vec()).ok()
                        } else {
                            None
                        };
                        if let Some(text) = text {
                            items.push(DecodedItem {
                                kind: DecodedKind::Paste { text },
                                raw: raw.into(),
                                owned,
                            });
                        } else {
                            items.push(DecodedItem::rejected_paste(owned));
                        }
                    } else if raw.len() >= PASTE_RAW_BOUND {
                        items.push(DecodedItem::rejected_paste(owned));
                        self.state = State::PasteTail { end, owned };
                    } else {
                        self.state = State::Paste {
                            raw,
                            end,
                            deadline: now + self.wait,
                            owned,
                        };
                    }
                }
                State::PasteTail { mut end, owned } => {
                    if !end.push(byte) {
                        self.state = State::PasteTail { end, owned };
                    }
                }
                State::X10 {
                    mut raw,
                    remaining,
                    owned,
                    ..
                } => {
                    if matches!(byte, 0x1b | 0x9b | 0x8f) {
                        // ESC/C1 aborts the report — the partial frame is
                        // pointer protocol, dropped; the abort byte is
                        // reprocessed as fresh input.
                        items.push(DecodedItem::ignored(&raw, owned));
                        pending = Some(byte);
                    } else {
                        raw.push(byte);
                        let remaining = remaining - 1;
                        if remaining == 0 {
                            // Three body bytes complete the report
                            // atomically: a well-formed one decodes to a
                            // mouse item, a malformed one is consumed and
                            // dropped as ignored — never keys.
                            match mouse::decode_x10(&raw[csi_prefix_len(&raw) + 1..]) {
                                Some(event) => items.push(DecodedItem {
                                    kind: DecodedKind::Mouse(event),
                                    raw: raw.into(),
                                    owned,
                                }),
                                None => items.push(DecodedItem::ignored(&raw, owned)),
                            }
                        } else {
                            self.state = State::X10 {
                                raw,
                                remaining,
                                deadline: now + self.wait,
                                owned,
                            };
                        }
                    }
                }
                State::Discard {
                    mut raw,
                    drain,
                    owned,
                } => match drain {
                    Drain::Csi | Drain::CsiMetadata | Drain::Ss3 => {
                        if matches!(byte, 0x1b | 0x9b | 0x8f) {
                            items.push(drain_emit(&drain, &raw, owned));
                            pending = Some(byte);
                        } else {
                            raw.push(byte);
                            let done = match drain {
                                Drain::Csi | Drain::CsiMetadata => (0x40..=0x7e).contains(&byte),
                                Drain::Ss3 => byte.is_ascii_alphabetic(),
                                Drain::X10 { .. } => unreachable!(),
                            };
                            if done {
                                items.push(drain_emit(&drain, &raw, owned));
                            } else {
                                Self::drain_chunk(&mut self.state, raw, drain, owned, items);
                            }
                        }
                    }
                    Drain::X10 { remaining } => {
                        if matches!(byte, 0x1b | 0x9b | 0x8f) {
                            items.push(DecodedItem::ignored(&raw, owned));
                            pending = Some(byte);
                        } else {
                            raw.push(byte);
                            if remaining == 1 {
                                // The last expected body byte ends the
                                // drain — the frame's ownership ends with it.
                                items.push(DecodedItem::ignored(&raw, owned));
                            } else {
                                self.state = State::Discard {
                                    raw,
                                    drain: Drain::X10 {
                                        remaining: remaining - 1,
                                    },
                                    owned,
                                };
                            }
                        }
                    }
                },
            }
        }
    }

    /// Emits a bounded non-decision chunk of the drained run, holding back
    /// a short tail so the in-progress generic sequence stays bounded.
    fn drain_chunk(
        state: &mut State,
        mut raw: Vec<u8>,
        drain: Drain,
        owned: bool,
        items: &mut Vec<DecodedItem>,
    ) {
        if raw.len() >= DISCARD_CHUNK + DISCARD_TAIL {
            items.push(if matches!(drain, Drain::CsiMetadata) {
                DecodedItem::ignored(&raw[..DISCARD_CHUNK], owned)
            } else {
                DecodedItem::nondecision(&raw[..DISCARD_CHUNK], owned)
            });
            raw.drain(..DISCARD_CHUNK);
        }
        *state = State::Discard { raw, drain, owned };
    }

    fn feed_ground(&mut self, byte: u8, now: Instant, owned: bool, items: &mut Vec<DecodedItem>) {
        match byte {
            0x1b => {
                self.state = State::Esc {
                    deadline: now + self.wait,
                    owned,
                };
            }
            0x9d => {
                self.state = State::Osc {
                    introducer: vec![byte],
                    frame: crate::terminal_events::appearance::OscReplyFrame::new(now),
                    selector_deadline: None,
                    owned,
                };
            }
            0x9b => {
                self.state = State::Csi {
                    raw: vec![byte],
                    deadline: now + self.wait,
                    owned,
                }
            }
            0x8f => {
                self.state = State::Ss3 {
                    raw: vec![byte],
                    deadline: now + self.wait,
                    owned,
                }
            }
            0x00..=0x1f => items.push(DecodedItem::key(
                c0_event(byte, Modifiers::NONE),
                &[byte],
                owned,
            )),
            0x7f => items.push(DecodedItem::key(
                KeyEvent::press(KeyCode::Backspace, Modifiers::NONE),
                &[byte],
                owned,
            )),
            0x20..=0x7e => items.push(DecodedItem::key(
                KeyEvent::press(KeyCode::Char(byte as char), Modifiers::NONE),
                &[byte],
                owned,
            )),
            _ => match continuation_count(byte) {
                Some(need) => {
                    self.state = State::Utf8 {
                        raw: vec![byte],
                        need,
                        alt: false,
                        deadline: now + self.wait,
                        owned,
                    };
                }
                None => items.push(DecodedItem::nondecision(&[byte], owned)),
            },
        }
    }

    /// `frame_owned` is the provenance of the frame the pending ESC opened;
    /// `byte_owned` is the current byte's own provenance. A second ESC ends
    /// the old frame and re-arms a new one — the re-armed frame belongs to
    /// the second byte's packet, not to the old frame's.
    fn feed_esc(
        &mut self,
        byte: u8,
        now: Instant,
        escape_deadline: Instant,
        frame_owned: bool,
        byte_owned: bool,
        items: &mut Vec<DecodedItem>,
    ) {
        match byte {
            b']' => {
                self.state = State::Osc {
                    introducer: vec![0x1b, byte],
                    frame: crate::terminal_events::appearance::OscReplyFrame::new(now),
                    selector_deadline: Some(escape_deadline),
                    owned: frame_owned,
                };
            }
            b'_' => {
                self.state = State::GraphicsStart {
                    deadline: now + self.wait,
                    owned: frame_owned,
                };
            }
            b'[' => {
                self.state = State::Csi {
                    raw: vec![0x1b, byte],
                    deadline: now + self.wait,
                    owned: frame_owned,
                }
            }
            b'O' => {
                self.state = State::Ss3 {
                    raw: vec![0x1b, byte],
                    deadline: now + self.wait,
                    owned: frame_owned,
                }
            }
            0x1b => {
                items.push(DecodedItem::key(
                    KeyEvent::press(KeyCode::Esc, Modifiers::NONE),
                    &[0x1b],
                    frame_owned,
                ));
                self.state = State::Esc {
                    deadline: now + self.wait,
                    owned: byte_owned,
                };
            }
            0x9b => {
                self.state = State::Csi {
                    raw: vec![0x1b, byte],
                    deadline: now + self.wait,
                    owned: frame_owned,
                }
            }
            0x8f => {
                self.state = State::Ss3 {
                    raw: vec![0x1b, byte],
                    deadline: now + self.wait,
                    owned: frame_owned,
                }
            }
            // A C0 control after ESC is never an Alt chord: the ESC resolves
            // as a lone Esc and the control byte decodes as its own key, so
            // Esc followed quickly by a prefix such as Ctrl+B keeps both.
            0x00..=0x1f => {
                items.push(DecodedItem::key(
                    KeyEvent::press(KeyCode::Esc, Modifiers::NONE),
                    &[0x1b],
                    frame_owned,
                ));
                self.feed_ground(byte, now, byte_owned, items);
            }
            0x7f => items.push(DecodedItem::key(
                KeyEvent::press(KeyCode::Backspace, Modifiers::NONE.with_alt()),
                &[0x1b, byte],
                frame_owned,
            )),
            0x20..=0x7e => items.push(DecodedItem::key(
                KeyEvent::press(KeyCode::Char(byte as char), Modifiers::NONE.with_alt()),
                &[0x1b, byte],
                frame_owned,
            )),
            _ => match continuation_count(byte) {
                Some(need) => {
                    self.state = State::Utf8 {
                        raw: vec![0x1b, byte],
                        need,
                        alt: true,
                        deadline: now + self.wait,
                        owned: frame_owned,
                    };
                }
                None => items.push(DecodedItem::nondecision(&[0x1b, byte], frame_owned)),
            },
        }
    }

    fn finish_csi(&mut self, raw: &[u8], now: Instant, owned: bool, items: &mut Vec<DecodedItem>) {
        let body = &raw[csi_prefix_len(raw)..];
        if let Some(reply) = csi_host_reply(body) {
            items.push(DecodedItem {
                kind: DecodedKind::HostReply(reply),
                raw: raw.into(),
                owned,
            });
            return;
        }
        if csi_host_metadata_shaped(body) {
            items.push(DecodedItem::ignored(raw, owned));
            return;
        }
        if body == b"M" {
            // X10 report head (`ESC[M` or `9b M`): exactly three body
            // bytes complete the frame — the head alone never emits.
            self.state = State::X10 {
                raw: raw.to_vec(),
                remaining: 3,
                deadline: now + self.wait,
                owned,
            };
            return;
        }
        match parse_csi(body) {
            Csi::Paste => {
                self.state = State::Paste {
                    raw: raw.to_vec(),
                    end: PasteEnd::new(raw[0] == 0x9b),
                    deadline: now + self.wait,
                    owned,
                };
            }
            Csi::Key(event) => items.push(DecodedItem::key(event, raw, owned)),
            Csi::Mouse(event) => items.push(DecodedItem {
                kind: DecodedKind::Mouse(event),
                raw: raw.into(),
                owned,
            }),
            Csi::Focus(focused) => items.push(DecodedItem {
                kind: DecodedKind::Focus(focused),
                raw: raw.into(),
                owned,
            }),
            Csi::Ignored => items.push(DecodedItem::ignored(raw, owned)),
            Csi::Nondecision => items.push(if csi_metadata_shaped(raw) {
                DecodedItem::ignored(raw, owned)
            } else {
                DecodedItem::nondecision(raw, owned)
            }),
        }
    }

    fn finish_ss3(&mut self, raw: &[u8], owned: bool, items: &mut Vec<DecodedItem>) -> bool {
        let Some(&last) = raw.last() else {
            return false;
        };
        if !last.is_ascii_alphabetic() {
            return false;
        }
        // Interior bytes between the SS3 marker and the final are xterm-style
        // modifier params (`O5A` = Ctrl+Up). Invalid forms emit non-decision
        // instead of silently downgrading to an unmodified key.
        let interior = &raw[ss3_prefix_len(raw)..raw.len() - 1];
        let params = split_params(interior);
        let mods = match params.as_slice() {
            [] | [Some(1)] => Modifiers::NONE,
            [Some(n)] => mods_from_param(*n, Modifiers::NONE),
            [Some(1), Some(n)] => mods_from_param(*n, Modifiers::NONE),
            _ => {
                items.push(DecodedItem::nondecision(raw, owned));
                return true;
            }
        };
        let code = match last {
            b'A' => Some(KeyCode::Up),
            b'B' => Some(KeyCode::Down),
            b'C' => Some(KeyCode::Right),
            b'D' => Some(KeyCode::Left),
            b'H' => Some(KeyCode::Home),
            b'F' => Some(KeyCode::End),
            b'P' => Some(KeyCode::F(1)),
            b'Q' => Some(KeyCode::F(2)),
            b'R' => Some(KeyCode::F(3)),
            b'S' => Some(KeyCode::F(4)),
            _ => None,
        };
        match code {
            Some(code) => items.push(DecodedItem::key(KeyEvent::press(code, mods), raw, owned)),
            None => items.push(DecodedItem::nondecision(raw, owned)),
        }
        true
    }

    fn finish_utf8(&mut self, raw: &[u8], alt: bool, owned: bool, items: &mut Vec<DecodedItem>) {
        let bytes = if alt { &raw[1..] } else { raw };
        match std::str::from_utf8(bytes)
            .ok()
            .and_then(|s| s.chars().next())
        {
            Some(c) => items.push(DecodedItem::key(
                KeyEvent::press(
                    KeyCode::Char(c),
                    if alt {
                        Modifiers::NONE.with_alt()
                    } else {
                        Modifiers::NONE
                    },
                ),
                raw,
                owned,
            )),
            None => items.push(DecodedItem::nondecision(raw, owned)),
        }
    }
}

fn replay_osc_candidate(
    decoder: &mut LegacyDecoder,
    introducer: &[u8],
    selector: &[u8],
    now: Instant,
    owned: bool,
    items: &mut Vec<DecodedItem>,
) {
    if introducer == b"\x1b]" {
        items.push(DecodedItem::key(
            KeyEvent::press(KeyCode::Char(']'), Modifiers::NONE.with_alt()),
            introducer,
            owned,
        ));
    } else {
        items.push(DecodedItem::nondecision(introducer, owned));
    }
    for byte in selector.iter().copied() {
        decoder.feed(byte, now, owned, items);
    }
}

/// The byte count of the paste-start marker inside a paste run's `raw`.
/// `ESC [ 2 0 0 ~` is 6; raw `9b 2 0 0 ~` is 5.
fn paste_marker_len(raw: &[u8]) -> usize {
    if raw.starts_with(&[0x9b]) { 5 } else { 6 }
}

fn csi_prefix_len(raw: &[u8]) -> usize {
    match raw.first() {
        Some(0x9b) => 1,
        _ => 2, // `ESC [`
    }
}

/// Parses the bounded outer-terminal metadata that input owns. Pointer
/// admission is intentionally strict: only mode 1003 and a syntactically
/// complete primary device-attributes reply are admitted as typed replies.
pub(crate) fn csi_host_reply(body: &[u8]) -> Option<super::HostReply> {
    if let Some(reply) = crate::terminal_events::graphics::cell_reply(body) {
        return Some(super::HostReply::Graphics(reply));
    }
    let pointer = match body {
        b"?1003;0$y" => Some(crate::terminal_events::pointer::ModeReportState::Unsupported),
        b"?1003;1$y" => Some(crate::terminal_events::pointer::ModeReportState::Set),
        b"?1003;2$y" => Some(crate::terminal_events::pointer::ModeReportState::Reset),
        b"?1003;3$y" => Some(crate::terminal_events::pointer::ModeReportState::PermanentlySet),
        b"?1003;4$y" => Some(crate::terminal_events::pointer::ModeReportState::PermanentlyReset),
        _ => None,
    };
    if let Some(state) = pointer {
        return Some(super::HostReply::Pointer(
            crate::terminal_events::pointer::HostReply::Mode1003(state),
        ));
    }
    primary_device_attributes(body).then_some(super::HostReply::Pointer(
        crate::terminal_events::pointer::HostReply::PrimaryDeviceAttributes,
    ))
}

fn primary_device_attributes(body: &[u8]) -> bool {
    let Some(parameters) = body
        .strip_prefix(b"?")
        .and_then(|body| body.strip_suffix(b"c"))
    else {
        return false;
    };
    !parameters.is_empty()
        && parameters
            .split(|byte| *byte == b';')
            .all(|parameter| !parameter.is_empty() && parameter.iter().all(u8::is_ascii_digit))
}

/// A complete private CSI is host metadata only when it carries a reply final.
/// Unknown private modes such as `?25h` remain generic input and pass through.
pub(crate) fn csi_host_metadata_shaped(body: &[u8]) -> bool {
    body.starts_with(b"6;")
        || (body.starts_with(b"?") && (body.ends_with(b"c") || body.ends_with(b"$y")))
}

/// A malformed mouse/host-metadata report cannot expose its residue as typed input.
fn csi_metadata_shaped(raw: &[u8]) -> bool {
    let body = raw.get(csi_prefix_len(raw)..).unwrap_or_default();
    body.starts_with(b"<") || csi_host_metadata_shaped(body)
}

/// Host metadata drains are dropped; unrelated sequences retain passthrough order.
fn drain_emit(drain: &Drain, raw: &[u8], owned: bool) -> DecodedItem {
    if matches!(drain, Drain::CsiMetadata | Drain::X10 { .. }) {
        DecodedItem::ignored(raw, owned)
    } else {
        DecodedItem::nondecision(raw, owned)
    }
}

fn ss3_prefix_len(raw: &[u8]) -> usize {
    match raw.first() {
        Some(0x8f) => 1,
        _ => 2, // `ESC O`
    }
}

/// Maps a C0 byte to its Ctrl chord (`0x0d`→`Ctrl+M`, `0x09`→`Ctrl+I`,
/// `0x08`→`Ctrl+H`); `0x00` is `Ctrl+Space`. ESC/DEL never reach here — the
/// caller owns the `0x1b`/`0x7f` exceptions.
fn c0_event(byte: u8, mods: Modifiers) -> KeyEvent {
    let code = match byte {
        // `0x00` is the historical Ctrl+Space encoding; the Space alias
        // canonicalizes to `Char(' ')` for matching.
        0x00 => KeyCode::Space,
        0x01..=0x1a => KeyCode::Char((byte + 0x60) as char),
        0x1c..=0x1f => KeyCode::Char((byte + 0x40) as char),
        _ => unreachable!("C0 dispatch covers 0x00..=0x1f"),
    };
    KeyEvent::press(code, mods.with_ctrl())
}

enum Csi {
    Paste,
    Key(KeyEvent),
    Mouse(MouseEvent),
    /// Host focus metadata, never terminal input or a prefix key.
    Focus(bool),
    /// A complete but malformed mouse report — pointer protocol, dropped.
    Ignored,
    Nondecision,
}

fn parse_csi(body: &[u8]) -> Csi {
    // SGR mouse reports (`<Cb;Cx;CyM`/`m`) decode atomically: malformed
    // forms are consumed and dropped, never reinterpreted as keys.
    if body.first() == Some(&b'<') && matches!(body.last(), Some(b'M' | b'm')) {
        return match mouse::decode_sgr(&body[1..]) {
            Some(event) => Csi::Mouse(event),
            None => Csi::Ignored,
        };
    }
    match body {
        b"I" => return Csi::Focus(true),
        b"O" => return Csi::Focus(false),
        b"A" => return Csi::Key(KeyEvent::press(KeyCode::Up, Modifiers::NONE)),
        b"B" => return Csi::Key(KeyEvent::press(KeyCode::Down, Modifiers::NONE)),
        b"C" => return Csi::Key(KeyEvent::press(KeyCode::Right, Modifiers::NONE)),
        b"D" => return Csi::Key(KeyEvent::press(KeyCode::Left, Modifiers::NONE)),
        b"H" => return Csi::Key(KeyEvent::press(KeyCode::Home, Modifiers::NONE)),
        b"F" => return Csi::Key(KeyEvent::press(KeyCode::End, Modifiers::NONE)),
        b"Z" => {
            return Csi::Key(KeyEvent::press(KeyCode::Tab, Modifiers::NONE.with_shift()));
        }
        b"200~" => return Csi::Paste,
        _ => {}
    }
    let Some(&last) = body.last() else {
        return Csi::Nondecision;
    };
    if last == b'u' {
        return parse_kitty_u(&body[..body.len() - 1]);
    }
    // `<params>~` and `<params><letter>` forms share one parameter parser.
    let param_len = body
        .iter()
        .position(|b| !(0x30..=0x3f).contains(b))
        .unwrap_or(body.len());
    let Some((number, mods, kind)) = special_key_parameters(&body[..param_len]) else {
        return Csi::Nondecision;
    };
    match last {
        b'~' => {
            let code = match number {
                Some(1 | 7) => Some(KeyCode::Home),
                Some(2) => Some(KeyCode::Insert),
                Some(3) => Some(KeyCode::Delete),
                Some(4 | 8) => Some(KeyCode::End),
                Some(5) => Some(KeyCode::PageUp),
                Some(6) => Some(KeyCode::PageDown),
                Some(n @ 11..=15) => Some(KeyCode::F((n - 10) as u8)),
                Some(n @ 17..=21) => Some(KeyCode::F((n - 11) as u8)),
                Some(n @ 23..=24) => Some(KeyCode::F((n - 12) as u8)),
                _ => None,
            };
            match code {
                Some(code) => Csi::Key(KeyEvent { code, mods, kind }),
                None => Csi::Nondecision,
            }
        }
        b'A' | b'B' | b'C' | b'D' | b'H' | b'F' | b'P' | b'Q' | b'R' | b'S' => {
            let code = match last {
                b'A' => KeyCode::Up,
                b'B' => KeyCode::Down,
                b'C' => KeyCode::Right,
                b'D' => KeyCode::Left,
                b'H' => KeyCode::Home,
                b'P'..=b'S' => KeyCode::F(last - b'P' + 1),
                _ => KeyCode::End,
            };
            match number {
                Some(1) | None => Csi::Key(KeyEvent { code, mods, kind }),
                _ => Csi::Nondecision,
            }
        }
        _ => Csi::Nondecision,
    }
}

fn parse_kitty_u(body: &[u8]) -> Csi {
    let semicolon = body.iter().position(|b| *b == b';').unwrap_or(body.len());
    let key_params = split_params(&body[..semicolon]);
    let Some(Some(number)) = key_params.first().copied() else {
        return Csi::Nondecision;
    };
    let tail = if semicolon < body.len() {
        &body[semicolon + 1..]
    } else {
        b"1"
    };
    let Some((mods, kind)) = modifier_event(tail) else {
        return Csi::Nondecision;
    };
    let code = match number {
        13 => KeyCode::Enter,
        9 => KeyCode::Tab,
        27 => KeyCode::Esc,
        127 => KeyCode::Backspace,
        57344 => KeyCode::Esc,
        57345 => KeyCode::Enter,
        57346 => KeyCode::Tab,
        57347 => KeyCode::Backspace,
        57348 => KeyCode::Insert,
        57349 => KeyCode::Delete,
        57350 => KeyCode::Left,
        57351 => KeyCode::Right,
        57352 => KeyCode::Up,
        57353 => KeyCode::Down,
        57354 => KeyCode::PageUp,
        57355 => KeyCode::PageDown,
        57356 => KeyCode::Home,
        57357 => KeyCode::End,
        n @ 57364..=57387 => KeyCode::F((n - 57363) as u8),
        n if n < 57344 => match char::from_u32(n) {
            Some(c) if !c.is_control() => KeyCode::Char(c),
            _ => return Csi::Nondecision,
        },
        _ => return Csi::Nondecision,
    };
    Csi::Key(KeyEvent { code, mods, kind })
}

/// Xterm and kitty share the modifier/event tail. Unknown modifiers are not
/// collapsed onto an ordinary shortcut, and malformed event kinds never become presses.
fn modifier_event(bytes: &[u8]) -> Option<(Modifiers, KeyKind)> {
    let mut parts = bytes.split(|byte| *byte == b':');
    let modifier = decimal(parts.next()?)?;
    if !(1..=256).contains(&modifier) || (modifier - 1) & 0x30 != 0 {
        return None;
    }
    let kind = match parts.next() {
        None | Some(b"1") => KeyKind::Press,
        Some(b"2") => KeyKind::Repeat,
        Some(b"3") => KeyKind::Release,
        _ => return None,
    };
    if parts.next().is_some() {
        return None;
    }
    Some((mods_from_param(modifier, Modifiers::NONE), kind))
}

fn decimal(bytes: &[u8]) -> Option<u32> {
    let parts = split_params(bytes);
    (parts.len() == 1).then(|| parts[0]).flatten()
}

fn special_key_parameters(bytes: &[u8]) -> Option<(Option<u32>, Modifiers, KeyKind)> {
    if bytes.is_empty() {
        return Some((None, Modifiers::NONE, KeyKind::Press));
    }
    let mut parts = bytes.split(|byte| *byte == b';');
    let number = decimal(parts.next()?)?;
    let (mods, kind) = modifier_event(parts.next().unwrap_or(b"1"))?;
    if parts.next().is_some() {
        return None;
    }
    Some((Some(number), mods, kind))
}

fn split_params(bytes: &[u8]) -> Vec<Option<u32>> {
    if bytes.is_empty() {
        return Vec::new();
    }
    bytes
        .split(|b| *b == b';')
        .map(|part| {
            if part.is_empty() {
                None
            } else {
                part.iter().try_fold(0u32, |n, b| {
                    if b.is_ascii_digit() {
                        n.checked_mul(10)
                            .and_then(|n| n.checked_add(u32::from(b - b'0')))
                    } else {
                        None
                    }
                })
            }
        })
        .collect()
}

/// Kitty-style modifier parameter: `1 + bitmask(shift=1, alt=2, ctrl=4, super=8)`.
fn mods_from_param(param: u32, base: Modifiers) -> Modifiers {
    let mask = param.saturating_sub(1);
    let mut mods = base;
    if mask & 1 != 0 {
        mods = mods.with_shift();
    }
    if mask & 2 != 0 {
        mods = mods.with_alt();
    }
    if mask & 4 != 0 {
        mods = mods.with_ctrl();
    }
    if mask & 8 != 0 {
        mods = mods.with_super_();
    }
    mods
}

#[cfg(test)]
mod tests;
