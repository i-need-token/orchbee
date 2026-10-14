//! Exact-handle console observation and bounded UTF-8 to `WriteConsoleW` adaptation.

use std::{
    error::Error,
    fmt,
    io::{self, Write},
    os::windows::io::{AsHandle, OwnedHandle},
    sync::Arc,
};

const MAX_UTF16_UNITS_PER_CALL: usize = 4096;

/// The visible console-buffer window associated with one exact output handle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConsoleWindowInfo {
    /// Inclusive left edge in screen-buffer coordinates.
    pub left: i16,
    /// Inclusive top edge in screen-buffer coordinates.
    pub top: i16,
    /// Visible cell columns, or zero for an inverted empty interval.
    pub columns: u16,
    /// Visible cell rows, or zero for an inverted empty interval.
    pub rows: u16,
}

/// The reason a console writer stopped accepting bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConsoleWriteErrorKind {
    /// `WriteConsoleW` failed and its effect is not known.
    Native,
    /// `WriteConsoleW` reported zero accepted UTF-16 units for nonempty input.
    WriteZero,
    /// `WriteConsoleW` reported a count outside the offered unit slice.
    InvalidAcceptedCount {
        /// Count reported by the port.
        accepted: usize,
        /// Number of UTF-16 units offered to the port.
        offered: usize,
    },
    /// The supplied bytes cannot begin or continue a valid UTF-8 scalar.
    InvalidUtf8,
    /// `flush` found a valid but incomplete UTF-8 scalar.
    IncompleteUtf8,
}

/// The native-progress classification attached to a writer fault.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConsoleWriteProgress {
    /// No earlier `WriteConsoleW` completion is recorded for this writer.
    None,
    /// The reported units ended after complete Unicode scalars.
    CompleteScalars {
        /// Newly consumed UTF-8 bytes represented by those complete scalars.
        utf8_bytes: usize,
        /// UTF-16 units accepted by that native call.
        utf16_units: usize,
    },
    /// The reported units ended after the high surrogate of one scalar.
    SplitScalar {
        /// Newly consumed UTF-8 bytes before the split scalar.
        complete_utf8_bytes: usize,
        /// UTF-16 units accepted by that native call.
        accepted_utf16_units: usize,
        /// The retained low surrogate that must be written exactly once.
        pending_low: u16,
    },
    /// The failing native call may have had an effect that cannot be measured safely.
    Unknown,
}

/// The writer's known boundary immediately before the operation that faulted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConsoleWriteBoundary {
    /// All accepted UTF-16 units end on a Unicode scalar boundary.
    ScalarBoundary,
    /// Valid UTF-8 prefix bytes are retained but do not complete a scalar.
    IncompleteUtf8 {
        /// Retained byte count, always at most three.
        bytes: usize,
    },
    /// A high surrogate is committed and this low surrogate remains pending.
    PendingLowSurrogate {
        /// The exact low surrogate retained by this writer.
        pending_low: u16,
    },
}

/// Typed detail stored inside an `io::Error` after a console writer fault.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConsoleWriteError {
    kind: ConsoleWriteErrorKind,
    progress: ConsoleWriteProgress,
    boundary_before: ConsoleWriteBoundary,
    raw_os_error: Option<i32>,
    io_kind: io::ErrorKind,
}

impl ConsoleWriteError {
    /// Returns the typed reason for the fault.
    pub const fn kind(&self) -> ConsoleWriteErrorKind {
        self.kind
    }

    /// Returns the fault's trusted progress, or `Unknown` for an unmeasured native effect.
    pub const fn progress(&self) -> ConsoleWriteProgress {
        self.progress
    }

    /// Returns the known writer boundary before the operation that faulted.
    pub const fn boundary_before(&self) -> ConsoleWriteBoundary {
        self.boundary_before
    }

    /// Returns the raw Win32 error code for a native fault, when supplied by the host.
    pub const fn raw_os_error(&self) -> Option<i32> {
        self.raw_os_error
    }

    fn io_error(&self) -> io::Error {
        io::Error::new(self.io_kind, self.clone())
    }
}

impl fmt::Display for ConsoleWriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            ConsoleWriteErrorKind::Native => match self.raw_os_error {
                Some(code) => write!(formatter, "WriteConsoleW failed with Win32 error {code}"),
                None => formatter.write_str("WriteConsoleW failed with an unknown native effect"),
            },
            ConsoleWriteErrorKind::WriteZero => {
                formatter.write_str("WriteConsoleW accepted zero UTF-16 units")
            }
            ConsoleWriteErrorKind::InvalidAcceptedCount { accepted, offered } => write!(
                formatter,
                "WriteConsoleW reported {accepted} accepted UTF-16 units for {offered} offered units"
            ),
            ConsoleWriteErrorKind::InvalidUtf8 => {
                formatter.write_str("console output received invalid UTF-8")
            }
            ConsoleWriteErrorKind::IncompleteUtf8 => {
                formatter.write_str("console output ended with incomplete UTF-8")
            }
        }
    }
}

impl Error for ConsoleWriteError {}

/// The exact caller-owned console output endpoint admitted for mode, geometry, and output.
pub struct ConsoleOutput {
    handle: Arc<OwnedHandle>,
}

impl fmt::Debug for ConsoleOutput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConsoleOutput")
            .finish_non_exhaustive()
    }
}

impl ConsoleOutput {
    /// Retains and validates a caller-supplied console output handle.
    ///
    /// # Errors
    ///
    /// Returns the Win32 or geometry error from inspecting the exact handle when it is not a
    /// usable console output endpoint.
    pub fn admit(handle: OwnedHandle) -> io::Result<Self> {
        let output = Self {
            handle: Arc::new(handle),
        };
        output.mode()?;
        output.window_info()?;
        Ok(output)
    }

    /// Reads the current console mode from this retained handle.
    ///
    /// # Errors
    ///
    /// Returns the raw Win32 error reported for this handle.
    pub fn mode(&self) -> io::Result<u32> {
        crate::sys::console_mode(self.handle.as_handle())
    }

    /// Sets the console mode on this retained handle.
    ///
    /// # Errors
    ///
    /// Returns the raw Win32 error reported for this handle. The caller owns any restoration debt.
    pub fn set_mode(&self, mode: u32) -> io::Result<()> {
        crate::sys::set_console_mode(self.handle.as_handle(), mode)
    }

    /// Observes the current visible console window through this retained handle.
    ///
    /// # Errors
    ///
    /// Returns the raw Win32 error or `InvalidData` when an inclusive extent exceeds `u16`.
    pub fn window_info(&self) -> io::Result<ConsoleWindowInfo> {
        let (left, top, right, bottom) = crate::sys::console_window(self.handle.as_handle())?;
        Ok(ConsoleWindowInfo {
            left,
            top,
            columns: inclusive_extent(left, right)?,
            rows: inclusive_extent(top, bottom)?,
        })
    }

    /// Creates an independent Unicode stream over this exact retained output handle.
    ///
    /// Each writer owns its UTF-8 tail, pending surrogate, and fault state. A caller must keep one
    /// writer for one ordered stream and flush it before normal drop. A fresh writer begins empty so
    /// bounded lifecycle cleanup can still be attempted after another writer faults.
    pub fn writer(&self) -> ConsoleWriter {
        ConsoleWriter {
            inner: UnicodeWriter::new(NativeOutputPort {
                handle: Arc::clone(&self.handle),
            }),
        }
    }
}

fn inclusive_extent(start: i16, end: i16) -> io::Result<u16> {
    let length = i32::from(end) - i32::from(start) + 1;
    if length <= 0 {
        return Ok(0);
    }
    u16::try_from(length).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "console window extent exceeds the supported cell range",
        )
    })
}

/// A per-stream UTF-8 adapter that writes through the retained console output handle.
pub struct ConsoleWriter {
    inner: UnicodeWriter<NativeOutputPort>,
}

impl fmt::Debug for ConsoleWriter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConsoleWriter")
            .finish_non_exhaustive()
    }
}

impl Write for ConsoleWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.inner.write(buffer)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

trait Utf16Port {
    fn write_utf16(&mut self, units: &[u16]) -> io::Result<usize>;
}

struct NativeOutputPort {
    handle: Arc<OwnedHandle>,
}

impl Utf16Port for NativeOutputPort {
    fn write_utf16(&mut self, units: &[u16]) -> io::Result<usize> {
        crate::sys::write_console(self.handle.as_handle(), units)
    }
}

struct UnicodeWriter<Port> {
    port: Port,
    state: WriterState,
    last_progress: ConsoleWriteProgress,
}

impl<Port: Utf16Port> UnicodeWriter<Port> {
    fn new(port: Port) -> Self {
        Self {
            port,
            state: WriterState::Clean,
            last_progress: ConsoleWriteProgress::None,
        }
    }

    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if let WriterState::Fault(failure) = &self.state {
            return Err(failure.io_error());
        }
        if buffer.is_empty() {
            return Ok(0);
        }
        self.drain_pending_low()?;
        let tail = match &self.state {
            WriterState::Utf8Tail(tail) => Some(*tail),
            WriterState::Clean => None,
            WriterState::PendingLowSurrogate(_) | WriterState::Fault(_) => {
                return Err(io::Error::other("console writer state did not settle"));
            }
        };
        let chunk = match EncodedChunk::new(tail, buffer) {
            Ok(chunk) => chunk,
            Err(InvalidUtf8) => {
                return Err(self.latch(
                    ConsoleWriteErrorKind::InvalidUtf8,
                    self.last_progress,
                    None,
                    io::ErrorKind::InvalidData,
                ));
            }
        };
        if chunk.units.is_empty() {
            if let Some(trailing) = chunk.trailing_tail {
                self.state = WriterState::Utf8Tail(trailing);
                return Ok(buffer.len());
            }
            return Err(self.latch(
                ConsoleWriteErrorKind::InvalidUtf8,
                self.last_progress,
                None,
                io::ErrorKind::InvalidData,
            ));
        }

        let offered = chunk.units.len();
        let accepted = match self.port.write_utf16(&chunk.units) {
            Ok(accepted) => accepted,
            Err(error) => {
                let raw_os_error = error.raw_os_error();
                let kind = error.kind();
                return Err(self.latch(
                    ConsoleWriteErrorKind::Native,
                    ConsoleWriteProgress::Unknown,
                    raw_os_error,
                    kind,
                ));
            }
        };
        if accepted == 0 {
            return Err(self.latch(
                ConsoleWriteErrorKind::WriteZero,
                self.last_progress,
                None,
                io::ErrorKind::WriteZero,
            ));
        }
        if accepted > offered {
            return Err(self.latch(
                ConsoleWriteErrorKind::InvalidAcceptedCount { accepted, offered },
                ConsoleWriteProgress::Unknown,
                None,
                io::ErrorKind::InvalidData,
            ));
        }

        match chunk.map_accepted(accepted) {
            Some(AcceptedPrefix::Complete { input_end }) => {
                self.last_progress = ConsoleWriteProgress::CompleteScalars {
                    utf8_bytes: input_end,
                    utf16_units: accepted,
                };
                if accepted == offered
                    && let Some(trailing) = chunk.trailing_tail
                {
                    self.state = WriterState::Utf8Tail(trailing);
                    return Ok(buffer.len());
                }
                self.state = WriterState::Clean;
                Ok(input_end)
            }
            Some(AcceptedPrefix::SplitScalar {
                complete_input_end,
                scalar_input_end,
                pending_low,
            }) => {
                self.last_progress = ConsoleWriteProgress::SplitScalar {
                    complete_utf8_bytes: complete_input_end,
                    accepted_utf16_units: accepted,
                    pending_low,
                };
                self.state = WriterState::PendingLowSurrogate(pending_low);
                Ok(scalar_input_end)
            }
            None => Err(self.latch(
                ConsoleWriteErrorKind::InvalidAcceptedCount { accepted, offered },
                ConsoleWriteProgress::Unknown,
                None,
                io::ErrorKind::InvalidData,
            )),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        if let WriterState::Fault(failure) = &self.state {
            return Err(failure.io_error());
        }
        self.drain_pending_low()?;
        match &self.state {
            WriterState::Clean => Ok(()),
            WriterState::Utf8Tail(_) => Err(self.latch(
                ConsoleWriteErrorKind::IncompleteUtf8,
                self.last_progress,
                None,
                io::ErrorKind::InvalidData,
            )),
            WriterState::PendingLowSurrogate(_) | WriterState::Fault(_) => {
                Err(io::Error::other("console writer state did not settle"))
            }
        }
    }

    fn drain_pending_low(&mut self) -> io::Result<()> {
        let pending_low = match &self.state {
            WriterState::PendingLowSurrogate(pending_low) => *pending_low,
            WriterState::Clean | WriterState::Utf8Tail(_) => return Ok(()),
            WriterState::Fault(failure) => return Err(failure.io_error()),
        };
        let accepted = match self.port.write_utf16(&[pending_low]) {
            Ok(accepted) => accepted,
            Err(error) => {
                let raw_os_error = error.raw_os_error();
                let kind = error.kind();
                return Err(self.latch(
                    ConsoleWriteErrorKind::Native,
                    ConsoleWriteProgress::Unknown,
                    raw_os_error,
                    kind,
                ));
            }
        };
        match accepted {
            1 => {
                self.state = WriterState::Clean;
                self.last_progress = ConsoleWriteProgress::CompleteScalars {
                    utf8_bytes: 0,
                    utf16_units: 1,
                };
                Ok(())
            }
            0 => Err(self.latch(
                ConsoleWriteErrorKind::WriteZero,
                self.last_progress,
                None,
                io::ErrorKind::WriteZero,
            )),
            accepted => Err(self.latch(
                ConsoleWriteErrorKind::InvalidAcceptedCount {
                    accepted,
                    offered: 1,
                },
                ConsoleWriteProgress::Unknown,
                None,
                io::ErrorKind::InvalidData,
            )),
        }
    }

    fn latch(
        &mut self,
        kind: ConsoleWriteErrorKind,
        progress: ConsoleWriteProgress,
        raw_os_error: Option<i32>,
        io_kind: io::ErrorKind,
    ) -> io::Error {
        let failure = ConsoleWriteError {
            kind,
            progress,
            boundary_before: self.boundary(),
            raw_os_error,
            io_kind,
        };
        self.state = WriterState::Fault(failure.clone());
        failure.io_error()
    }

    fn boundary(&self) -> ConsoleWriteBoundary {
        match &self.state {
            WriterState::Clean | WriterState::Fault(_) => ConsoleWriteBoundary::ScalarBoundary,
            WriterState::Utf8Tail(tail) => ConsoleWriteBoundary::IncompleteUtf8 { bytes: tail.len },
            WriterState::PendingLowSurrogate(pending_low) => {
                ConsoleWriteBoundary::PendingLowSurrogate {
                    pending_low: *pending_low,
                }
            }
        }
    }
}

impl<Port: Utf16Port> Write for UnicodeWriter<Port> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        UnicodeWriter::write(self, buffer)
    }

    fn flush(&mut self) -> io::Result<()> {
        UnicodeWriter::flush(self)
    }
}

#[derive(Clone)]
enum WriterState {
    Clean,
    Utf8Tail(Utf8Tail),
    PendingLowSurrogate(u16),
    Fault(ConsoleWriteError),
}

#[derive(Clone, Copy)]
struct Utf8Tail {
    bytes: [u8; 4],
    len: usize,
}

impl Utf8Tail {
    fn from_slice(bytes: &[u8]) -> Self {
        debug_assert!(!bytes.is_empty() && bytes.len() <= 3);
        let mut stored = [0; 4];
        stored[..bytes.len()].copy_from_slice(bytes);
        Self {
            bytes: stored,
            len: bytes.len(),
        }
    }

    fn as_slice(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

struct EncodedChunk {
    units: Vec<u16>,
    scalars: Vec<ScalarBoundary>,
    trailing_tail: Option<Utf8Tail>,
}

impl EncodedChunk {
    fn new(tail: Option<Utf8Tail>, input: &[u8]) -> Result<Self, InvalidUtf8> {
        let mut chunk = Self {
            units: Vec::with_capacity(MAX_UTF16_UNITS_PER_CALL),
            scalars: Vec::with_capacity(MAX_UTF16_UNITS_PER_CALL),
            trailing_tail: None,
        };
        let mut input_offset = 0;

        if let Some(tail) = tail {
            let expected = expected_utf8_len(tail.as_slice()[0]).ok_or(InvalidUtf8)?;
            let needed = expected.checked_sub(tail.len).ok_or(InvalidUtf8)?;
            if input.len() < needed {
                let mut combined = [0; 4];
                combined[..tail.len].copy_from_slice(tail.as_slice());
                combined[tail.len..tail.len + input.len()].copy_from_slice(input);
                let bytes = &combined[..tail.len + input.len()];
                if !is_valid_incomplete_utf8(bytes) {
                    return Err(InvalidUtf8);
                }
                chunk.trailing_tail = Some(Utf8Tail::from_slice(bytes));
                return Ok(chunk);
            }
            let mut scalar = [0; 4];
            scalar[..tail.len].copy_from_slice(tail.as_slice());
            scalar[tail.len..expected].copy_from_slice(&input[..needed]);
            let character = decode_scalar(&scalar[..expected])?;
            chunk.push_scalar(character, 0, needed);
            input_offset = needed;
        }

        while input_offset < input.len() {
            let remaining = &input[input_offset..];
            let Some(expected) = expected_utf8_len(remaining[0]) else {
                if chunk.units.is_empty() {
                    return Err(InvalidUtf8);
                }
                break;
            };
            if remaining.len() < expected {
                if !is_valid_incomplete_utf8(remaining) {
                    if chunk.units.is_empty() {
                        return Err(InvalidUtf8);
                    }
                    break;
                }
                chunk.trailing_tail = Some(Utf8Tail::from_slice(remaining));
                break;
            }
            let character = match decode_scalar(&remaining[..expected]) {
                Ok(character) => character,
                Err(error) if chunk.units.is_empty() => return Err(error),
                Err(_) => break,
            };
            if chunk.units.len() + character.len_utf16() > MAX_UTF16_UNITS_PER_CALL {
                break;
            }
            let input_end = input_offset + expected;
            chunk.push_scalar(character, input_offset, input_end);
            input_offset = input_end;
        }

        Ok(chunk)
    }

    fn push_scalar(&mut self, character: char, input_start: usize, input_end: usize) {
        let utf16_start = self.units.len();
        let mut encoded = [0; 2];
        let encoded = character.encode_utf16(&mut encoded);
        self.units.extend_from_slice(encoded);
        self.scalars.push(ScalarBoundary {
            utf16_start,
            utf16_end: self.units.len(),
            input_start,
            input_end,
            pending_low: encoded.get(1).copied(),
        });
    }

    fn map_accepted(&self, accepted: usize) -> Option<AcceptedPrefix> {
        self.scalars.iter().find_map(|scalar| {
            if accepted == scalar.utf16_end {
                Some(AcceptedPrefix::Complete {
                    input_end: scalar.input_end,
                })
            } else if scalar.pending_low.is_some()
                && accepted == scalar.utf16_start + 1
                && scalar.utf16_end == scalar.utf16_start + 2
            {
                Some(AcceptedPrefix::SplitScalar {
                    complete_input_end: scalar.input_start,
                    scalar_input_end: scalar.input_end,
                    pending_low: scalar.pending_low?,
                })
            } else {
                None
            }
        })
    }
}

struct ScalarBoundary {
    utf16_start: usize,
    utf16_end: usize,
    input_start: usize,
    input_end: usize,
    pending_low: Option<u16>,
}

enum AcceptedPrefix {
    Complete {
        input_end: usize,
    },
    SplitScalar {
        complete_input_end: usize,
        scalar_input_end: usize,
        pending_low: u16,
    },
}

#[derive(Clone, Copy)]
struct InvalidUtf8;

fn expected_utf8_len(first: u8) -> Option<usize> {
    match first {
        0x00..=0x7f => Some(1),
        0xc2..=0xdf => Some(2),
        0xe0..=0xef => Some(3),
        0xf0..=0xf4 => Some(4),
        _ => None,
    }
}

fn decode_scalar(bytes: &[u8]) -> Result<char, InvalidUtf8> {
    let text = std::str::from_utf8(bytes).map_err(|_| InvalidUtf8)?;
    let mut characters = text.chars();
    let character = characters.next().ok_or(InvalidUtf8)?;
    if characters.next().is_some() {
        return Err(InvalidUtf8);
    }
    Ok(character)
}

fn is_valid_incomplete_utf8(bytes: &[u8]) -> bool {
    std::str::from_utf8(bytes).is_err_and(|error| error.error_len().is_none())
}

#[cfg(test)]
mod tests;
