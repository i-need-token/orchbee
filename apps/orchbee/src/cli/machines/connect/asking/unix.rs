//! The terminal of `machine connect` asking on macOS and Linux: a canonical line read with echo
//! off for a secret, as `readpassphrase` reads one, straight from the standard input descriptor
//! so no buffer outside this module ever holds the answer, each read wiped once it is copied. An
//! interrupt reaches the command's own signal capture, and the end of input aborts the question.

use super::{Line, Typed};
use nix::sys::termios::{LocalFlags, SetArg, Termios, tcgetattr, tcsetattr};
use orchbee_machine_protocol::{MAXIMUM_ANSWER_BYTES, wipe};
use std::{
    fs::File,
    io::{self, IsTerminal, Read},
    os::fd::AsFd,
};

/// Bytes one read takes: the longest admissible answer and the line's end.
const BYTES: usize = MAXIMUM_ANSWER_BYTES + 2;
const LINE_FEED: u8 = b'\n';
const CARRIAGE_RETURN: u8 = b'\r';

/// The terminal settings before any question.
pub(super) type Earlier = Termios;

/// The terminal settings when standard input and standard error are both terminals.
pub(super) fn detect() -> Option<Earlier> {
    if !io::stdin().is_terminal() || !io::stderr().is_terminal() {
        return None;
    }
    tcgetattr(io::stdin().as_fd()).ok()
}

/// The terminal's settings while a question reads; the earlier settings return when it drops.
pub(super) struct Mode {
    earlier: Termios,
    pub(super) echo: bool,
}

impl Mode {
    pub(super) fn set(earlier: &Earlier, echo: bool) -> Option<Self> {
        let mut reading = earlier.clone();
        reading.local_flags |= LocalFlags::ICANON | LocalFlags::ISIG;
        reading.local_flags.set(LocalFlags::ECHO, echo);
        reading.local_flags.remove(LocalFlags::ECHONL);
        // A failed set may still have changed the settings, so the earlier ones return either way.
        let mode = Self {
            earlier: earlier.clone(),
            echo,
        };
        // Input typed before the question appeared is discarded rather than taken as its answer.
        tcsetattr(io::stdin().as_fd(), SetArg::TCSAFLUSH, &reading).ok()?;
        Some(mode)
    }
}

impl Drop for Mode {
    fn drop(&mut self) {
        let _ = tcsetattr(io::stdin().as_fd(), SetArg::TCSANOW, &self.earlier);
    }
}

/// The read buffer, wiped when dropped. It has no debug form.
struct Bytes([u8; BYTES]);

impl Drop for Bytes {
    fn drop(&mut self) {
        wipe(&mut self.0);
    }
}

/// Reads one line from the terminal, to its end even when it is too long, so nothing typed is left
/// for whatever reads the terminal next.
pub(super) fn read_line() -> Line {
    // An unbuffered handle of its own: standard input's shared buffer would keep answer bytes.
    let Ok(mut input) = io::stdin().as_fd().try_clone_to_owned().map(File::from) else {
        return Line::Failed;
    };
    let mut bytes = Bytes([0; BYTES]);
    // One byte past the bound is refused before it is pushed, so the buffer never grows.
    let mut typed = Typed(Vec::with_capacity(MAXIMUM_ANSWER_BYTES));
    let mut fits = true;
    loop {
        let read = match input.read(&mut bytes.0) {
            Ok(read) => read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => return Line::Failed,
        };
        if read == 0 {
            return Line::Aborted;
        }
        let chunk = bytes.0.get(..read).unwrap_or_default();
        let ended = chunk.last() == Some(&LINE_FEED);
        let mut content = chunk;
        while let [rest @ .., LINE_FEED | CARRIAGE_RETURN] = content {
            content = rest;
        }
        fits = fits && typed.0.len() + content.len() <= MAXIMUM_ANSWER_BYTES;
        if fits {
            typed.0.extend_from_slice(content);
        }
        wipe(&mut bytes.0);
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
