//! The legacy host decoder: one bounded state machine that turns the raw host
//! byte stream into `DecodedItem`s — typed `KeyEvent`s, atomic paste payloads,
//! or `Nondecision` for complete sequences that are not key events. Every item
//! owns its source bytes, so cross-read fragments never borrow a dead chunk and
//! unconsumed items can forward their `raw` byte-for-byte in order.
//!
//! Ownership semantics: a prefix follower that decodes to any item — key or
//! non-decision — is exactly one event. UTF-8 continuation bytes and CSI tails
//! therefore can never reach the pane or a binding as partial input.

use std::time::{Duration, Instant};

use crate::client_view::{KeyCode, KeyEvent, KeyKind, Modifiers};

/// One decoded input item with its owned source bytes. `owned` marks an
/// item that completes a frame which began inside a captured packet, so
/// the dispatcher consumes it instead of forwarding or deciding it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DecodedItem {
    pub(crate) kind: DecodedKind,
    pub(crate) raw: Box<[u8]>,
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum DecodedKind {
    Key(KeyEvent),
    /// One bracketed-paste run (`ESC[200~` … `ESC[201~`). `text` is the
    /// payload decoded as UTF-8; `None` when the payload was malformed or
    /// exceeded the bound. The payload never enters binding matching.
    Paste {
        text: Option<String>,
    },
    /// A complete sequence that is not a key event (unknown CSI/SS3, stray
    /// paste terminator, malformed UTF-8 run). Consumed atomically.
    Nondecision,
}

const CSI_BOUND: usize = 64;
const SS3_BOUND: usize = 8;
const PASTE_RAW_BOUND: usize = 4 * 1024 * 1024;
const PASTE_TEXT_BOUND: usize = 256 * 1024;
/// A drained frame emits its leftover bytes in bounded non-decision chunks
/// so passthrough order survives without unbounded allocation. `TAIL`
/// keeps enough trailing bytes for a cross-chunk terminator match.
const DISCARD_CHUNK: usize = 64;
const DISCARD_TAIL: usize = 6;
const PASTE_END_ESC: &[u8] = b"\x1b[201~";
const PASTE_END_C1: &[u8] = b"\x9b201~";

/// Terminator-matching state for a paste run or its drained tail. It is a
/// prefix-progress counter over the opener-notation terminator plus the
/// UTF-8 continuation countdown, so the match survives an emit boundary —
/// bytes already emitted are never re-forwarded, and a `\x9b` consumed as
/// a UTF-8 continuation is never a C1 terminator lead.
#[derive(Debug)]
struct PasteEnd {
    term: &'static [u8],
    prefix: usize,
    utf8_need: u8,
}

impl PasteEnd {
    fn new(c1: bool) -> Self {
        Self {
            term: if c1 { PASTE_END_C1 } else { PASTE_END_ESC },
            prefix: 0,
            utf8_need: 0,
        }
    }

    /// Feeds one payload byte; returns true when the byte completes the
    /// real terminator.
    fn push(&mut self, byte: u8) -> bool {
        let continuation = self.utf8_need > 0;
        if continuation {
            self.utf8_need -= 1;
        } else if let Some(need) = utf8_need(byte) {
            self.utf8_need = need;
        }
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
    Ss3,
    Paste(PasteEnd),
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
    /// An in-progress UTF-8 scalar; `raw` includes an optional leading ESC
    /// (Alt-modified scalar) and `need` counts remaining continuation bytes.
    Utf8 {
        raw: Vec<u8>,
        need: u8,
        alt: bool,
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
pub(crate) struct LegacyDecoder {
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
    pub(crate) const fn new(wait: Duration) -> Self {
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
    pub(crate) fn push(&mut self, byte: u8, now: Instant, owned: bool) -> Vec<DecodedItem> {
        let mut items = Vec::new();
        self.feed(byte, now, owned, &mut items);
        items
    }

    /// Resolves bounded waits: a lone ESC becomes an Esc event; a truncated
    /// CSI/SS3 emits its bytes and drains the frame tail; a truncated UTF-8
    /// scalar emits as non-decision. At most one item per call.
    pub(crate) fn tick(&mut self, now: Instant) -> Vec<DecodedItem> {
        let mut items = Vec::new();
        match std::mem::replace(&mut self.state, State::Ground) {
            State::Esc { deadline, owned } if now >= deadline => {
                items.push(DecodedItem::key(
                    KeyEvent::press(KeyCode::Esc, Modifiers::NONE),
                    &[0x1b],
                    owned,
                ));
            }
            State::Csi {
                raw,
                deadline,
                owned,
            } if now >= deadline => {
                items.push(DecodedItem::nondecision(&raw, owned));
                self.state = State::Discard {
                    raw: Vec::new(),
                    drain: Drain::Csi,
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
                deadline,
                owned,
                ..
            } if now >= deadline => {
                items.push(DecodedItem::nondecision(&raw, owned));
            }
            // A paste whose payload wait expired emits what it collected and
            // drains until the real terminator; the in-flight terminator
            // match survives the boundary.
            State::Paste {
                raw,
                end,
                deadline,
                owned,
            } if now >= deadline => {
                items.push(DecodedItem::nondecision(&raw, owned));
                self.state = State::Discard {
                    raw: Vec::new(),
                    drain: Drain::Paste(end),
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
                State::Esc { owned, .. } => self.feed_esc(byte, now, owned, items),
                State::Csi { mut raw, owned, .. } => {
                    if matches!(byte, 0x1b | 0x9b | 0x8f) {
                        // ESC/C1 aborts the sequence; the abort byte is
                        // reprocessed as fresh input.
                        items.push(DecodedItem::nondecision(&raw, owned));
                        pending = Some(byte);
                    } else {
                        raw.push(byte);
                        if (0x40..=0x7e).contains(&byte) {
                            self.finish_csi(&raw, now, owned, items);
                        } else if raw.len() >= CSI_BOUND {
                            items.push(DecodedItem::nondecision(&raw, owned));
                            self.state = State::Discard {
                                raw: Vec::new(),
                                drain: Drain::Csi,
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
                State::Paste {
                    mut raw,
                    mut end,
                    owned,
                    ..
                } => {
                    raw.push(byte);
                    if end.push(byte) {
                        // The real terminator completed — checked before the
                        // bound so a terminator ending exactly at the cap is
                        // still an atomic paste item.
                        let payload_end = raw.len() - end.term.len();
                        let payload = &raw[paste_marker_len(&raw)..payload_end];
                        let text = if payload.len() <= PASTE_TEXT_BOUND {
                            String::from_utf8(payload.to_vec()).ok()
                        } else {
                            None
                        };
                        items.push(DecodedItem {
                            kind: DecodedKind::Paste { text },
                            raw: raw.into(),
                            owned,
                        });
                    } else if raw.len() >= PASTE_RAW_BOUND {
                        items.push(DecodedItem::nondecision(&raw, owned));
                        self.state = State::Discard {
                            raw: Vec::new(),
                            drain: Drain::Paste(end),
                            owned,
                        };
                    } else {
                        self.state = State::Paste {
                            raw,
                            end,
                            deadline: now + self.wait,
                            owned,
                        };
                    }
                }
                State::Discard {
                    mut raw,
                    drain,
                    owned,
                } => match drain {
                    Drain::Csi | Drain::Ss3 => {
                        if matches!(byte, 0x1b | 0x9b | 0x8f) {
                            items.push(DecodedItem::nondecision(&raw, owned));
                            pending = Some(byte);
                        } else {
                            raw.push(byte);
                            let done = match drain {
                                Drain::Csi => (0x40..=0x7e).contains(&byte),
                                Drain::Ss3 => byte.is_ascii_alphabetic(),
                                Drain::Paste(_) => unreachable!(),
                            };
                            if done {
                                items.push(DecodedItem::nondecision(&raw, owned));
                            } else {
                                Self::drain_chunk(&mut self.state, raw, drain, owned, items);
                            }
                        }
                    }
                    Drain::Paste(mut end) => {
                        // The terminator match survived the emit boundary:
                        // already-emitted bytes are never re-forwarded —
                        // only the un-emitted tail in `raw` is.
                        raw.push(byte);
                        if end.push(byte) {
                            items.push(DecodedItem::nondecision(&raw, owned));
                        } else {
                            Self::drain_chunk(
                                &mut self.state,
                                raw,
                                Drain::Paste(end),
                                owned,
                                items,
                            );
                        }
                    }
                },
            }
        }
    }

    /// Emits a bounded non-decision chunk of the drained run, holding back
    /// a short tail so the state it carries stays a small suffix. A paste
    /// terminator split across the chunk boundary still completes: the
    /// match lives in `PasteEnd`, not in the retained bytes.
    fn drain_chunk(
        state: &mut State,
        mut raw: Vec<u8>,
        drain: Drain,
        owned: bool,
        items: &mut Vec<DecodedItem>,
    ) {
        if raw.len() >= DISCARD_CHUNK + DISCARD_TAIL {
            items.push(DecodedItem::nondecision(&raw[..DISCARD_CHUNK], owned));
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
            _ => match utf8_need(byte) {
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

    fn feed_esc(&mut self, byte: u8, now: Instant, owned: bool, items: &mut Vec<DecodedItem>) {
        match byte {
            b'[' => {
                self.state = State::Csi {
                    raw: vec![0x1b, byte],
                    deadline: now + self.wait,
                    owned,
                }
            }
            b'O' => {
                self.state = State::Ss3 {
                    raw: vec![0x1b, byte],
                    deadline: now + self.wait,
                    owned,
                }
            }
            0x1b => {
                items.push(DecodedItem::key(
                    KeyEvent::press(KeyCode::Esc, Modifiers::NONE),
                    &[0x1b],
                    owned,
                ));
                self.state = State::Esc {
                    deadline: now + self.wait,
                    owned,
                };
            }
            0x9b => {
                self.state = State::Csi {
                    raw: vec![0x1b, byte],
                    deadline: now + self.wait,
                    owned,
                }
            }
            0x8f => {
                self.state = State::Ss3 {
                    raw: vec![0x1b, byte],
                    deadline: now + self.wait,
                    owned,
                }
            }
            0x00..=0x1f => items.push(DecodedItem::key(
                c0_event(byte, Modifiers::NONE.with_alt()),
                &[0x1b, byte],
                owned,
            )),
            0x7f => items.push(DecodedItem::key(
                KeyEvent::press(KeyCode::Backspace, Modifiers::NONE.with_alt()),
                &[0x1b, byte],
                owned,
            )),
            0x20..=0x7e => items.push(DecodedItem::key(
                KeyEvent::press(KeyCode::Char(byte as char), Modifiers::NONE.with_alt()),
                &[0x1b, byte],
                owned,
            )),
            _ => match utf8_need(byte) {
                Some(need) => {
                    self.state = State::Utf8 {
                        raw: vec![0x1b, byte],
                        need,
                        alt: true,
                        deadline: now + self.wait,
                        owned,
                    };
                }
                None => items.push(DecodedItem::nondecision(&[0x1b, byte], owned)),
            },
        }
    }

    fn finish_csi(&mut self, raw: &[u8], now: Instant, owned: bool, items: &mut Vec<DecodedItem>) {
        let body = &raw[csi_prefix_len(raw)..];
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
            Csi::Nondecision => items.push(DecodedItem::nondecision(raw, owned)),
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

fn ss3_prefix_len(raw: &[u8]) -> usize {
    match raw.first() {
        Some(0x8f) => 1,
        _ => 2, // `ESC O`
    }
}

fn utf8_need(lead: u8) -> Option<u8> {
    match lead {
        0xc2..=0xdf => Some(1),
        0xe0..=0xef => Some(2),
        0xf0..=0xf4 => Some(3),
        _ => None,
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
    Nondecision,
}

fn parse_csi(body: &[u8]) -> Csi {
    match body {
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
    let params = split_params(&body[..param_len]);
    match last {
        b'~' => {
            let mods = params
                .get(1)
                .copied()
                .flatten()
                .map_or(Modifiers::NONE, |n| mods_from_param(n, Modifiers::NONE));
            let code = match params.first().copied().flatten() {
                Some(1 | 7) => Some(KeyCode::Home),
                Some(2) => Some(KeyCode::Insert),
                Some(3) => Some(KeyCode::Delete),
                Some(4 | 8) => Some(KeyCode::End),
                Some(5) => Some(KeyCode::PageUp),
                Some(6) => Some(KeyCode::PageDown),
                Some(n @ 11..=24) => Some(KeyCode::F((n - 10) as u8)),
                _ => None,
            };
            match code {
                Some(code) => Csi::Key(KeyEvent::press(code, mods)),
                None => Csi::Nondecision,
            }
        }
        b'A' | b'B' | b'C' | b'D' | b'H' | b'F' => {
            let code = match last {
                b'A' => KeyCode::Up,
                b'B' => KeyCode::Down,
                b'C' => KeyCode::Right,
                b'D' => KeyCode::Left,
                b'H' => KeyCode::Home,
                _ => KeyCode::End,
            };
            let mods = params
                .get(1)
                .copied()
                .flatten()
                .map_or(Modifiers::NONE, |n| mods_from_param(n, Modifiers::NONE));
            match params.first().copied().flatten() {
                Some(1) | None => Csi::Key(KeyEvent::press(code, mods)),
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
    // Tail form: `<mods>[:<event>]`. Repeat/release are decoded so owners can
    // consume press+repeat and never dispatch release.
    let (mods_param, kind) = if semicolon < body.len() {
        let tail = &body[semicolon + 1..];
        let colon = tail.iter().position(|b| *b == b':').unwrap_or(tail.len());
        let mods = split_params(&tail[..colon])
            .first()
            .copied()
            .flatten()
            .unwrap_or(1);
        let kind = if colon < tail.len() {
            match split_params(&tail[colon + 1..]).first().copied().flatten() {
                Some(2) => KeyKind::Repeat,
                Some(3) => KeyKind::Release,
                _ => KeyKind::Press,
            }
        } else {
            KeyKind::Press
        };
        (mods, kind)
    } else {
        (1, KeyKind::Press)
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
        n @ 57364..=57375 => KeyCode::F((n - 57363) as u8),
        n if n < 57344 => match char::from_u32(n) {
            Some(c) if !c.is_control() => KeyCode::Char(c),
            _ => return Csi::Nondecision,
        },
        _ => return Csi::Nondecision,
    };
    let mods = mods_from_param(mods_param, Modifiers::NONE);
    Csi::Key(KeyEvent { code, mods, kind })
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
