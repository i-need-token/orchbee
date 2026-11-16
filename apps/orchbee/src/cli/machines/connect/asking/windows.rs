//! The Windows console of `machine connect` asking: its input mode, and its UTF-16 reads, each
//! wiped once it is copied.

use super::{Line, Typed};
use orchbee_machine_protocol::{MAXIMUM_ANSWER_BYTES, wipe};
use winsafe::{self as w, co};

/// UTF-16 units one read takes: the longest admissible answer, one unit for each byte, and the
/// line's CR LF.
const UNITS: usize = MAXIMUM_ANSWER_BYTES + 2;
const LINE_FEED: u16 = 0x0A;
const CARRIAGE_RETURN: u16 = 0x0D;

/// The input mode before any question.
pub(super) type Earlier = co::CONSOLE;

/// The input mode when standard input and standard error are both consoles.
pub(super) fn detect() -> Option<Earlier> {
    let mode = standard(co::STD_HANDLE::INPUT)?.GetConsoleMode().ok()?;
    standard(co::STD_HANDLE::ERROR)?.GetConsoleMode().ok()?;
    Some(mode)
}

/// The console's input mode while a question reads; the earlier mode returns when it drops.
pub(super) struct Mode {
    earlier: co::CONSOLE,
    pub(super) echo: bool,
}

impl Mode {
    pub(super) fn set(earlier: &Earlier, echo: bool) -> Option<Self> {
        let earlier = *earlier;
        let line = earlier | co::CONSOLE::ENABLE_LINE_INPUT | co::CONSOLE::ENABLE_PROCESSED_INPUT;
        let reading = if echo {
            line | co::CONSOLE::ENABLE_ECHO_INPUT
        } else {
            line & !co::CONSOLE::ENABLE_ECHO_INPUT
        };
        let input = standard(co::STD_HANDLE::INPUT)?;
        // A failed set may still have changed the mode, so the earlier one returns either way.
        let mode = Self { earlier, echo };
        input.SetConsoleMode(reading).ok()?;
        Some(mode)
    }
}

impl Drop for Mode {
    fn drop(&mut self) {
        if let Some(input) = standard(co::STD_HANDLE::INPUT) {
            let _ = input.SetConsoleMode(self.earlier);
        }
    }
}

/// A standard handle, left open: it belongs to the process, not to this command.
fn standard(which: co::STD_HANDLE) -> Option<w::HSTD> {
    let mut guard = w::HSTD::GetStdHandle(which).ok()?;
    Some(guard.leak())
}

/// The console's UTF-16 read buffer, wiped when dropped. It has no debug form.
struct Units(w::WString);

impl Drop for Units {
    fn drop(&mut self) {
        wipe_units(self.0.as_mut_slice());
    }
}

fn wipe_units(units: &mut [u16]) {
    units.fill(0);
    std::hint::black_box(units);
}

/// Reads one line from the console, to its end even when it is too long, so nothing typed is left
/// for whatever reads the console next.
pub(super) fn read_line() -> Line {
    let Some(input) = standard(co::STD_HANDLE::INPUT) else {
        return Line::Failed;
    };
    let mut units = Units(w::WString::new_alloc_buf(UNITS));
    // One character past the bound is refused before it is pushed, so the buffer never grows.
    let mut typed = Typed(Vec::with_capacity(MAXIMUM_ANSWER_BYTES));
    let mut fits = true;
    loop {
        let read = match input.ReadConsole(&mut units.0, None) {
            Ok(read) => usize::try_from(read).unwrap_or(0),
            Err(_) => return Line::Failed,
        };
        if read == 0 {
            return Line::Aborted;
        }
        let buffer = units.0.as_slice();
        let chunk = buffer.get(..read).unwrap_or(buffer);
        let ended = chunk.last() == Some(&LINE_FEED);
        let mut content = chunk;
        while let [rest @ .., LINE_FEED | CARRIAGE_RETURN] = content {
            content = rest;
        }
        fits = fits && append(&mut typed.0, content);
        wipe_units(units.0.as_mut_slice());
        if ended {
            break;
        }
    }
    match (fits, typed.0.is_empty()) {
        (false, _) => Line::TooLong,
        (true, true) => Line::Empty,
        (true, false) => Line::Answer(typed),
    }
}

/// Appends `units` as UTF-8 within the longest admissible answer; `false` once it would not fit.
fn append(typed: &mut Vec<u8>, units: &[u16]) -> bool {
    for decoded in char::decode_utf16(units.iter().copied()) {
        let mut bytes = [0; 4];
        let length = decoded
            .unwrap_or(char::REPLACEMENT_CHARACTER)
            .encode_utf8(&mut bytes)
            .len();
        let fits = typed.len() + length <= MAXIMUM_ANSWER_BYTES;
        if fits {
            typed.extend_from_slice(&bytes[..length]);
        }
        wipe(&mut bytes);
        if !fits {
            return false;
        }
    }
    true
}
