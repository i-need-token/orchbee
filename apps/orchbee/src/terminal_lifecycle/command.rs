//! ANSI command encoding against the output stream admitted by the caller.

use std::{
    fmt,
    io::{self, Write},
};

/// Encodes one Crossterm command directly into `output` without consulting
/// Crossterm's process-global Windows console capability or WinAPI fallback.
///
/// The caller owns buffering and flush boundaries. On Windows it must call
/// this only while VT output processing is admitted for the same output path.
/// An error can follow a successfully written prefix; the helper never replays
/// that prefix and stops touching the writer after retaining the first fault.
pub(crate) fn write_ansi_command(
    output: &mut (impl Write + ?Sized),
    command: &impl crossterm::Command,
) -> io::Result<()> {
    let mut sink = AnsiSink {
        first_error: None,
        output,
    };
    let encoded = command.write_ansi(&mut sink);
    match sink.first_error {
        Some(error) => Err(error),
        None if encoded.is_ok() => Ok(()),
        None => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "terminal command failed to encode its ANSI representation",
        )),
    }
}

struct AnsiSink<'a, W: Write + ?Sized> {
    first_error: Option<io::Error>,
    output: &'a mut W,
}

impl<W: Write + ?Sized> fmt::Write for AnsiSink<'_, W> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if self.first_error.is_some() {
            return Err(fmt::Error);
        }
        match self.output.write_all(value.as_bytes()) {
            Ok(()) => Ok(()),
            Err(error) => {
                self.first_error = Some(error);
                Err(fmt::Error)
            }
        }
    }
}

#[cfg(test)]
#[path = "command/tests.rs"]
mod tests;
