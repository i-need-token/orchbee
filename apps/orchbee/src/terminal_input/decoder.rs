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

/// One decoded input item with its owned source bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DecodedItem {
    pub(crate) kind: DecodedKind,
    pub(crate) raw: Box<[u8]>,
}

impl DecodedItem {
    fn key(event: KeyEvent, raw: &[u8]) -> Self {
        Self {
            kind: DecodedKind::Key(event),
            raw: raw.into(),
        }
    }

    fn nondecision(raw: &[u8]) -> Self {
        Self {
            kind: DecodedKind::Nondecision,
            raw: raw.into(),
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

#[derive(Debug)]
enum State {
    Ground,
    /// `ESC` seen; resolves as a lone Esc on `deadline` or as Alt+<key> /
    /// CSI / SS3 when the next byte arrives.
    Esc {
        deadline: Instant,
    },
    /// CSI bytes (after `ESC[` or raw `0x9b`). `raw` starts at the leading
    /// ESC and collects through the final byte.
    Csi {
        raw: Vec<u8>,
    },
    /// SS3 bytes (after `ESCO` or raw `0x8d`).
    Ss3 {
        raw: Vec<u8>,
    },
    /// An in-progress UTF-8 scalar; `raw` includes an optional leading ESC
    /// (Alt-modified scalar) and `need` counts remaining continuation bytes.
    Utf8 {
        raw: Vec<u8>,
        need: u8,
        alt: bool,
    },
    /// Inside `ESC[200~` … `ESC[201~`. `raw` holds the whole run for
    /// passthrough; payload text is derived at the terminator.
    Paste {
        raw: Vec<u8>,
    },
}

#[derive(Debug)]
pub(crate) struct LegacyDecoder {
    state: State,
    esc_wait: Duration,
}

impl Default for LegacyDecoder {
    fn default() -> Self {
        Self::new(Duration::from_millis(50))
    }
}

impl LegacyDecoder {
    pub(crate) const fn new(esc_wait: Duration) -> Self {
        Self {
            state: State::Ground,
            esc_wait,
        }
    }

    /// Feeds one byte; returns any items completed by it. An item may surface
    /// several reads after its first byte — the decoder owns the bytes in the
    /// meantime.
    pub(crate) fn push(&mut self, byte: u8, now: Instant) -> Vec<DecodedItem> {
        let mut items = Vec::new();
        self.feed(byte, now, &mut items);
        items
    }

    /// Resolves the bounded lone-ESC wait. Only `Esc` has a clock; truncated
    /// UTF-8/CSI wait byte-boundedly for the next read instead.
    pub(crate) fn tick(&mut self, now: Instant) -> Vec<DecodedItem> {
        let mut items = Vec::new();
        if let State::Esc { deadline } = self.state
            && now >= deadline
        {
            self.state = State::Ground;
            items.push(DecodedItem::key(
                KeyEvent::press(KeyCode::Esc, Modifiers::NONE),
                &[0x1b],
            ));
        }
        items
    }

    fn feed(&mut self, byte: u8, now: Instant, items: &mut Vec<DecodedItem>) {
        let mut pending = Some(byte);
        while let Some(byte) = pending.take() {
            match std::mem::replace(&mut self.state, State::Ground) {
                State::Ground => self.feed_ground(byte, now, items),
                State::Esc { .. } => self.feed_esc(byte, now, items),
                State::Csi { mut raw } => {
                    raw.push(byte);
                    if (0x40..=0x7e).contains(&byte) {
                        self.finish_csi(&raw, items);
                    } else if raw.len() >= CSI_BOUND {
                        items.push(DecodedItem::nondecision(&raw));
                    } else {
                        self.state = State::Csi { raw };
                    }
                }
                State::Ss3 { mut raw } => {
                    raw.push(byte);
                    if self.finish_ss3(&raw, items) {
                        // finished
                    } else if raw.len() >= SS3_BOUND {
                        items.push(DecodedItem::nondecision(&raw));
                    } else {
                        self.state = State::Ss3 { raw };
                    }
                }
                State::Utf8 { mut raw, need, alt } => {
                    if (0x80..=0xbf).contains(&byte) && need > 0 {
                        raw.push(byte);
                        let need = need - 1;
                        if need == 0 {
                            self.finish_utf8(&raw, alt, items);
                        } else {
                            self.state = State::Utf8 { raw, need, alt };
                        }
                    } else {
                        items.push(DecodedItem::nondecision(&raw));
                        pending = Some(byte);
                    }
                }
                State::Paste { mut raw } => {
                    raw.push(byte);
                    if raw.len() >= PASTE_RAW_BOUND {
                        // Absurdly long paste: emit what was collected as one
                        // non-decision run and resynchronize on the next byte.
                        items.push(DecodedItem::nondecision(&raw));
                        self.state = State::Ground;
                        continue;
                    }
                    let terminator = if raw.ends_with(b"\x1b[201~") {
                        6
                    } else if raw.ends_with(b"\x9b201~") {
                        5
                    } else {
                        0
                    };
                    if terminator > 0 {
                        let payload_end = raw.len() - terminator;
                        let payload = &raw[paste_marker_len(&raw)..payload_end];
                        let text = if payload.len() <= PASTE_TEXT_BOUND {
                            String::from_utf8(payload.to_vec()).ok()
                        } else {
                            None
                        };
                        items.push(DecodedItem {
                            kind: DecodedKind::Paste { text },
                            raw: raw.into(),
                        });
                    } else {
                        self.state = State::Paste { raw };
                    }
                }
            }
        }
    }

    fn feed_ground(&mut self, byte: u8, now: Instant, items: &mut Vec<DecodedItem>) {
        match byte {
            0x1b => {
                self.state = State::Esc {
                    deadline: now + self.esc_wait,
                };
            }
            0x9b => self.state = State::Csi { raw: vec![byte] },
            0x8d => self.state = State::Ss3 { raw: vec![byte] },
            0x00..=0x1f => items.push(DecodedItem::key(c0_event(byte, Modifiers::NONE), &[byte])),
            0x7f => items.push(DecodedItem::key(
                KeyEvent::press(KeyCode::Backspace, Modifiers::NONE),
                &[byte],
            )),
            0x20..=0x7e => items.push(DecodedItem::key(
                KeyEvent::press(KeyCode::Char(byte as char), Modifiers::NONE),
                &[byte],
            )),
            _ => match utf8_need(byte) {
                Some(need) => {
                    self.state = State::Utf8 {
                        raw: vec![byte],
                        need,
                        alt: false,
                    };
                }
                None => items.push(DecodedItem::nondecision(&[byte])),
            },
        }
    }

    fn feed_esc(&mut self, byte: u8, now: Instant, items: &mut Vec<DecodedItem>) {
        match byte {
            b'[' => {
                self.state = State::Csi {
                    raw: vec![0x1b, byte],
                }
            }
            b'O' => {
                self.state = State::Ss3 {
                    raw: vec![0x1b, byte],
                }
            }
            0x1b => {
                items.push(DecodedItem::key(
                    KeyEvent::press(KeyCode::Esc, Modifiers::NONE),
                    &[0x1b],
                ));
                self.state = State::Esc {
                    deadline: now + self.esc_wait,
                };
            }
            0x9b => {
                self.state = State::Csi {
                    raw: vec![0x1b, byte],
                }
            }
            0x8d => {
                self.state = State::Ss3 {
                    raw: vec![0x1b, byte],
                }
            }
            0x00..=0x1f => items.push(DecodedItem::key(
                c0_event(byte, Modifiers::NONE.with_alt()),
                &[0x1b, byte],
            )),
            0x7f => items.push(DecodedItem::key(
                KeyEvent::press(KeyCode::Backspace, Modifiers::NONE.with_alt()),
                &[0x1b, byte],
            )),
            0x20..=0x7e => items.push(DecodedItem::key(
                KeyEvent::press(KeyCode::Char(byte as char), Modifiers::NONE.with_alt()),
                &[0x1b, byte],
            )),
            _ => match utf8_need(byte) {
                Some(need) => {
                    self.state = State::Utf8 {
                        raw: vec![0x1b, byte],
                        need,
                        alt: true,
                    };
                }
                None => items.push(DecodedItem::nondecision(&[0x1b, byte])),
            },
        }
    }

    fn finish_csi(&mut self, raw: &[u8], items: &mut Vec<DecodedItem>) {
        let body = &raw[csi_prefix_len(raw)..];
        match parse_csi(body) {
            Csi::Paste => {
                self.state = State::Paste { raw: raw.to_vec() };
            }
            Csi::Key(event) => items.push(DecodedItem::key(event, raw)),
            Csi::Nondecision => items.push(DecodedItem::nondecision(raw)),
        }
    }

    fn finish_ss3(&mut self, raw: &[u8], items: &mut Vec<DecodedItem>) -> bool {
        let Some(&last) = raw.last() else {
            return false;
        };
        if !last.is_ascii_alphabetic() {
            return false;
        }
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
            Some(code) => items.push(DecodedItem::key(
                KeyEvent::press(code, Modifiers::NONE),
                raw,
            )),
            None => items.push(DecodedItem::nondecision(raw)),
        }
        true
    }

    fn finish_utf8(&mut self, raw: &[u8], alt: bool, items: &mut Vec<DecodedItem>) {
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
            )),
            None => items.push(DecodedItem::nondecision(raw)),
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
