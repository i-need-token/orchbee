//! Linux and macOS outer-terminal title and clipboard sequences, written through the admitted
//! output. Control characters never reach a sequence body, so no text can end it early.
use crate::terminal_device::InheritedTerminal;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::{
    io::{self, Write as _},
    sync::atomic::{AtomicBool, Ordering},
};

/// Saves the terminal's own title on the xterm title stack before the first change, then sets
/// the window and icon title (OSC 0).
pub(super) fn set_window_title(
    terminal: &InheritedTerminal,
    saved: &AtomicBool,
    title: &str,
) -> io::Result<()> {
    let push = !saved.load(Ordering::Acquire);
    let bytes = window_title(title, push);
    let mut output = terminal.output()?;
    if push {
        // Marked before the write: a partial push must still be popped on exit.
        saved.store(true, Ordering::Release);
    }
    output.write_all(&bytes)?;
    output.flush()
}

/// Restores the title saved before the first change; nothing was saved when no title was set.
pub(super) fn restore_window_title(
    terminal: &InheritedTerminal,
    saved: &AtomicBool,
) -> io::Result<()> {
    if !saved.load(Ordering::Acquire) {
        return Ok(());
    }
    let mut output = terminal.output()?;
    output.write_all(TITLE_POP)?;
    output.flush()?;
    saved.store(false, Ordering::Release);
    Ok(())
}

pub(super) fn write_clipboard(terminal: &InheritedTerminal, text: &str) -> io::Result<()> {
    let mut output = terminal.output()?;
    output.write_all(&clipboard(text))?;
    output.flush()
}

const TITLE_PUSH: &[u8] = b"\x1b[22;0t";
const TITLE_POP: &[u8] = b"\x1b[23;0t";

fn window_title(title: &str, push: bool) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(title.len() + 16);
    if push {
        bytes.extend_from_slice(TITLE_PUSH);
    }
    bytes.extend_from_slice(b"\x1b]0;");
    let clean: String = title.chars().filter(|ch| !ch.is_control()).collect();
    bytes.extend_from_slice(clean.as_bytes());
    bytes.extend_from_slice(b"\x1b\\");
    bytes
}

/// OSC 52 for the clipboard selection; the base64 body carries every byte of the text unchanged.
fn clipboard(text: &str) -> Vec<u8> {
    let mut bytes = b"\x1b]52;c;".to_vec();
    bytes.extend_from_slice(STANDARD.encode(text).as_bytes());
    bytes.extend_from_slice(b"\x1b\\");
    bytes
}

#[cfg(test)]
mod tests;
