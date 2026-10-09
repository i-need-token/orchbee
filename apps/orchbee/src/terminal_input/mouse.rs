//! Mouse report vocabulary and pane passthrough encoding — one owner for the
//! three related contracts: decoding host SGR/X10 reports into `MouseEvent`s,
//! encoding pane-local `PaneMouseEvent`s into the tracking/format bytes a
//! pane terminal requested, and the alternate-screen wheel → cursor-key
//! fallback. Keeping all three in one module keeps the byte contract a
//! single table instead of three drifiting implementations.
//!
//! The encode side is a cell-coordinate port of the Host's pixel-space
//! pipeline (pinned ghostty-vt `mouse_encode.zig` plus the `input_encoding`
//! extended-button remap): same input facts, same bytes, because a cell
//! coordinate is the cell every pixel inside it maps to. Cell-only input
//! uses SGR cell fallback when the pane requests pixel reporting (`?1016`),
//! matching Herdr. This owner never invents an exact pixel position.

use crate::client_view::Modifiers;
use orchbee_terminal_protocol::{MouseFormat, MouseTracking};

/// One decoded host mouse report. `column`/`row` are 0-based host cells;
/// X10 and SGR wire forms are both 1-based and convert at decode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MouseEvent {
    pub(crate) action: MouseAction,
    pub(crate) button: MouseButton,
    pub(crate) mods: Modifiers,
    pub(crate) column: u16,
    pub(crate) row: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MouseAction {
    Press,
    Release,
    /// Motion with a button held (drag), or button-free hover while a menu
    /// temporarily requests any-motion tracking.
    Motion,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MouseButton {
    Left,
    Middle,
    Right,
    /// Extended buttons 8-11; `n` is the low bit pair (0-3), so the report
    /// code is `128 + n`. The decoder never produces `n > 3`.
    Extended(u8),
    WheelUp,
    WheelDown,
    WheelLeft,
    WheelRight,
    /// No button identity: a release in non-SGR encodings, or hover
    /// motion. Never a real button.
    None,
}

/// A pane-local semantic event, the encode side's input. Coordinates are
/// 0-based cells relative to the pane viewport and may be negative or
/// beyond the viewport when a drag leaves it — the encoder applies the
/// same out-of-viewport rules as the Host's pixel pipeline.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PaneMouseEvent {
    pub(crate) action: MouseAction,
    /// `None` only for hover motion; menu capture consumes its own reports
    /// before the pane encoder is reached.
    pub(crate) button: Option<MouseButton>,
    pub(crate) mods: Modifiers,
    pub(crate) column: i32,
    pub(crate) row: i32,
}

/// The outcome of encoding one pane-local event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PaneEncode {
    /// Encoded report bytes, always within `MAX_PANE_MOUSE_BYTES`.
    Bytes(Vec<u8>),
    /// The event produced no report under this tracking/format contract.
    Silent,
}

/// Matches the Host's `max_mouse_bytes` bound.
const MAX_PANE_MOUSE_BYTES: usize = 128;
/// The largest cell index the traditional/UTF-8 byte forms can encode
/// (value + 32 must fit one byte).
const LEGACY_CELL_MAX: i32 = 222;

/// Decodes one SGR mouse body — the bytes between `ESC[<` and the `M`/`m`
/// final. `None` means malformed: the caller consumes the whole sequence
/// atomically and drops it, never reinterpreting its bytes as keys.
pub(crate) fn decode_sgr(body: &[u8]) -> Option<MouseEvent> {
    let (&final_byte, params) = body.split_last()?;
    let release = match final_byte {
        b'M' => false,
        b'm' => true,
        _ => return None,
    };
    // Each parameter is 1-5 ASCII digits and must fit u16: coordinates
    // beyond that cannot name a real cell, and anything else is a
    // malformed report, consumed and dropped whole.
    let mut fields = [0u16; 3];
    let mut count = 0usize;
    for part in params.split(|b| *b == b';') {
        if count >= 3 || part.is_empty() || part.len() > 5 {
            return None;
        }
        let mut value = 0u32;
        for b in part {
            if !b.is_ascii_digit() {
                return None;
            }
            value = value * 10 + u32::from(b - b'0');
        }
        *fields.get_mut(count)? = u16::try_from(value).ok()?;
        count += 1;
    }
    if count != 3 {
        return None;
    }
    let [cb, x, y] = fields;
    if x == 0 || y == 0 || cb > u16::from(u8::MAX) {
        return None;
    }
    let (button, motion) = button_from_cb(cb as u8);
    let action = if release {
        MouseAction::Release
    } else if motion {
        MouseAction::Motion
    } else {
        MouseAction::Press
    };
    Some(MouseEvent {
        action,
        button,
        mods: mods_from_cb(cb as u8),
        column: x - 1,
        row: y - 1,
    })
}

/// Decodes one X10 report body — exactly the three `Cb Cx Cy` bytes after
/// `ESC[M`. Same malformed contract as `decode_sgr`.
pub(crate) fn decode_x10(body: &[u8]) -> Option<MouseEvent> {
    let &[cb, x, y] = body else {
        return None;
    };
    // Wire values are offset by 32 and coordinates are 1-based, so the
    // smallest legal byte is 33.
    let (cb, x, y) = (cb.checked_sub(32)?, x.checked_sub(33)?, y.checked_sub(33)?);
    let (button, motion) = button_from_cb(cb);
    Some(MouseEvent {
        action: if motion {
            MouseAction::Motion
        } else if button == MouseButton::None {
            MouseAction::Release
        } else {
            MouseAction::Press
        },
        button,
        mods: mods_from_cb(cb),
        column: u16::from(x),
        row: u16::from(y),
    })
}

/// Splits the report's button byte into button identity and the motion
/// flag. The byte layout is shared by X10 bodies and SGR `Cb`.
fn button_from_cb(cb: u8) -> (MouseButton, bool) {
    let button = if cb & 64 != 0 {
        match cb & 3 {
            0 => MouseButton::WheelUp,
            1 => MouseButton::WheelDown,
            2 => MouseButton::WheelLeft,
            _ => MouseButton::WheelRight,
        }
    } else if cb & 128 != 0 {
        MouseButton::Extended(cb & 3)
    } else {
        match cb & 3 {
            0 => MouseButton::Left,
            1 => MouseButton::Middle,
            2 => MouseButton::Right,
            _ => MouseButton::None,
        }
    };
    (button, cb & 32 != 0)
}

/// Modifier bits shared by X10 bodies and SGR `Cb` (bit 2 shift, bit 3
/// alt, bit 4 ctrl). The super bit has no mouse-report representation.
fn mods_from_cb(cb: u8) -> Modifiers {
    let mut mods = Modifiers::NONE;
    if cb & 4 != 0 {
        mods = mods.with_shift();
    }
    if cb & 8 != 0 {
        mods = mods.with_alt();
    }
    if cb & 16 != 0 {
        mods = mods.with_ctrl();
    }
    mods
}

/// Who owns a pointer event inside a pane viewport, derived from the
/// pane's most recently validated interaction facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PointerOwner {
    /// Encode pane-local and forward through the existing input channel.
    Pane,
    /// The Client consumes it (chrome gesture or M1 drop).
    Client,
    /// Shift-modified: the host's native selection already claimed the
    /// gesture — never claimed, never forwarded, never a Client gesture.
    Host,
}

/// The §5.1 ownership rule: any tracking mode gives the pane the event;
/// without tracking only the alternate-screen DEC 1007 wheel fallback does
/// (cursor-key form); everything else is Client territory. Shift-modified
/// reports are host-owned regardless.
pub(crate) fn pane_owns_pointer(
    tracking: MouseTracking,
    event: &MouseEvent,
    alternate_scroll: bool,
) -> PointerOwner {
    if event.mods.shift {
        return PointerOwner::Host;
    }
    if tracking != MouseTracking::None {
        return PointerOwner::Pane;
    }
    if alternate_scroll && matches!(event.button, MouseButton::WheelUp | MouseButton::WheelDown) {
        return PointerOwner::Pane;
    }
    PointerOwner::Client
}

/// Encodes one pane-local event for the pane's requested tracking and
/// format. `viewport` is the pane's cell dimensions, `any_button_pressed`
/// mirrors the Host's gate for out-of-viewport motion — a press counts
/// itself, matching `Options::any_button_pressed`'s "reflect the state of
/// the current event" contract — and `last_cell` carries the motion dedup
/// state (updated on every reported event, read only for motion).
///
/// Cell-only events use SGR cell encoding for a pixel-format request.
/// Returns `Silent` when the contract produces no report — tracking gate misses, unencodable
/// buttons, out-of-range legacy coordinates, and same-cell motion.
pub(crate) fn encode_pane_report(
    event: &PaneMouseEvent,
    tracking: MouseTracking,
    format: MouseFormat,
    viewport: (u16, u16),
    any_button_pressed: bool,
    last_cell: &mut Option<(u16, u16)>,
) -> PaneEncode {
    if !should_report(event, tracking) {
        return PaneEncode::Silent;
    }
    let (columns, rows) = viewport;
    let outside = event.column < 0
        || event.row < 0
        || event.column >= i32::from(columns)
        || event.row >= i32::from(rows);
    if outside && event.action != MouseAction::Release {
        // Releases always report; other out-of-viewport events only
        // report under motion tracking while a button is held, so a drag
        // leaving the pane still reaches it. A press is itself held.
        let held = any_button_pressed || event.action == MouseAction::Press;
        if !event_sends_motion(tracking) || !held {
            return PaneEncode::Silent;
        }
    }
    let cell = (
        event.column.clamp(0, i32::from(columns.saturating_sub(1))) as u16,
        event.row.clamp(0, i32::from(rows.saturating_sub(1))) as u16,
    );
    if event.action == MouseAction::Motion && *last_cell == Some(cell) {
        return PaneEncode::Silent;
    }
    *last_cell = Some(cell);
    let Some(code) = button_code(event, tracking, format) else {
        return PaneEncode::Silent;
    };
    let bytes = match format {
        MouseFormat::Default => {
            if cell.0 > LEGACY_CELL_MAX as u16 || cell.1 > LEGACY_CELL_MAX as u16 {
                return PaneEncode::Silent;
            }
            vec![
                0x1b,
                b'[',
                b'M',
                32 + code,
                32 + cell.0 as u8 + 1,
                32 + cell.1 as u8 + 1,
            ]
        }
        MouseFormat::Utf8 => {
            let mut out = vec![0x1b, b'[', b'M', 32 + code];
            push_utf8(&mut out, u32::from(cell.0) + 33);
            push_utf8(&mut out, u32::from(cell.1) + 33);
            out
        }
        MouseFormat::Sgr | MouseFormat::SgrPixels => {
            let final_byte = if event.action == MouseAction::Release {
                b'm'
            } else {
                b'M'
            };
            format!(
                "\x1b[<{code};{};{}{}",
                cell.0 + 1,
                cell.1 + 1,
                final_byte as char
            )
            .into_bytes()
        }
        MouseFormat::Urxvt => format!(
            "\x1b[{};{};{}M",
            32 + u32::from(code),
            cell.0 + 1,
            cell.1 + 1
        )
        .into_bytes(),
    };
    debug_assert!(bytes.len() <= MAX_PANE_MOUSE_BYTES);
    PaneEncode::Bytes(bytes)
}

/// The tracking gate — one mode's worth of `shouldReport`: X10 sees only
/// left/middle/right presses, Normal sees everything but motion, Button
/// needs a held button, Any sees all.
fn should_report(event: &PaneMouseEvent, tracking: MouseTracking) -> bool {
    match tracking {
        MouseTracking::None => false,
        MouseTracking::X10 => {
            event.action == MouseAction::Press
                && matches!(
                    event.button,
                    Some(MouseButton::Left | MouseButton::Middle | MouseButton::Right)
                )
        }
        MouseTracking::Normal => event.action != MouseAction::Motion,
        MouseTracking::Button => event.button.is_some(),
        MouseTracking::Any => true,
    }
}

/// Whether the tracking mode reports motion at all (Button/Any) — the
/// out-of-viewport gate's first half.
fn event_sends_motion(tracking: MouseTracking) -> bool {
    matches!(tracking, MouseTracking::Button | MouseTracking::Any)
}

/// The report's button byte before the format wrapper, mirroring the
/// Host's `buttonCode` plus its extended-button remap: null and non-SGR
/// releases collapse to 3, extended buttons occupy 128-131, X10 tracking
/// strips modifiers, motion adds 32.
fn button_code(event: &PaneMouseEvent, tracking: MouseTracking, format: MouseFormat) -> Option<u8> {
    let mut acc: u8 = match event.button {
        None => 3,
        Some(_)
            if event.action == MouseAction::Release
                && !matches!(format, MouseFormat::Sgr | MouseFormat::SgrPixels) =>
        {
            3
        }
        Some(MouseButton::Left) => 0,
        Some(MouseButton::Middle) => 1,
        Some(MouseButton::Right) => 2,
        Some(MouseButton::WheelUp) => 64,
        Some(MouseButton::WheelDown) => 65,
        Some(MouseButton::WheelLeft) => 66,
        Some(MouseButton::WheelRight) => 67,
        Some(MouseButton::Extended(n)) if n <= 3 => 128 + n,
        Some(MouseButton::Extended(_) | MouseButton::None) => return None,
    };
    if tracking != MouseTracking::X10 {
        if event.mods.shift {
            acc += 4;
        }
        if event.mods.alt {
            acc += 8;
        }
        if event.mods.ctrl {
            acc += 16;
        }
    }
    if event.action == MouseAction::Motion {
        acc += 32;
    }
    Some(acc)
}

/// The alternate-screen wheel fallback: `count` cursor presses in the
/// pane's cursor-key mode — SS3 under application cursor keys, CSI
/// otherwise. Bounded by the caller to one viewport per flush.
pub(crate) fn encode_cursor_arrows(up: bool, count: usize, application_cursor: bool) -> Vec<u8> {
    let key = if up { b'A' } else { b'B' };
    let head = if application_cursor { b'O' } else { b'[' };
    let mut out = Vec::with_capacity(count * 3);
    for _ in 0..count {
        out.extend_from_slice(&[0x1b, head, key]);
    }
    out
}

/// Writes `cp` as UTF-8 without surrogate checks — the wire form encodes
/// a raw coordinate value, not a Unicode scalar.
fn push_utf8(out: &mut Vec<u8>, cp: u32) {
    if cp < 0x80 {
        out.push(cp as u8);
    } else if cp < 0x800 {
        out.push(0xc0 | (cp >> 6) as u8);
        out.push(0x80 | (cp & 63) as u8);
    } else if cp < 0x10000 {
        out.push(0xe0 | (cp >> 12) as u8);
        out.push(0x80 | ((cp >> 6) & 63) as u8);
        out.push(0x80 | (cp & 63) as u8);
    } else {
        out.push(0xf0 | (cp >> 18) as u8);
        out.push(0x80 | ((cp >> 12) & 63) as u8);
        out.push(0x80 | ((cp >> 6) & 63) as u8);
        out.push(0x80 | (cp & 63) as u8);
    }
}

#[cfg(test)]
mod tests;
