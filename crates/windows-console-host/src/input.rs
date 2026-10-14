//! Retained console-input ownership and count-validated raw record delivery.

use std::{
    fmt, io,
    os::windows::io::{AsHandle, BorrowedHandle, OwnedHandle},
};

/// One raw console input record with Win32 representation types removed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputRecord {
    /// A key transition carrying the untranslated UTF-16 code unit and keyboard metadata.
    Key(KeyRecord),
    /// A mouse record whose coordinates remain absolute screen-buffer coordinates.
    Mouse(MouseRecord),
    /// A console focus transition.
    Focus(bool),
    /// A screen-buffer resize carrying the signed values supplied by Win32.
    Resize {
        /// Reported screen-buffer columns.
        columns: i16,
        /// Reported screen-buffer rows.
        rows: i16,
    },
    /// A console menu event.
    Menu {
        /// Win32 menu command identifier.
        command_id: u32,
    },
    /// An event tag not recognized by this version of the boundary.
    Unknown {
        /// Original Win32 event tag. No union arm was read for this record.
        event_type: u16,
    },
}

/// Untranslated fields from one Win32 Unicode key record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KeyRecord {
    /// Whether this record is a key-down transition.
    pub key_down: bool,
    /// Repeat count attached to this record.
    pub repeat_count: u16,
    /// Virtual-key code supplied by the console.
    pub virtual_key_code: u16,
    /// Hardware scan code supplied by the console.
    pub virtual_scan_code: u16,
    /// Raw Unicode UTF-16 code unit from `ReadConsoleInputW`.
    pub utf16_unit: u16,
    /// Win32 control-key-state bit word.
    pub control_key_state: u32,
}

/// Untranslated fields from one Win32 mouse record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MouseRecord {
    /// Absolute screen-buffer column.
    pub x: i16,
    /// Absolute screen-buffer row.
    pub y: i16,
    /// Win32 button-state bit word, including wheel delta records.
    pub button_state: u32,
    /// Win32 control-key-state bit word.
    pub control_key_state: u32,
    /// Win32 mouse-event-flags bit word.
    pub event_flags: u32,
}

/// The exact caller-owned console input endpoint admitted for mode and record operations.
pub struct ConsoleInput {
    handle: OwnedHandle,
}

impl fmt::Debug for ConsoleInput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConsoleInput")
            .finish_non_exhaustive()
    }
}

impl ConsoleInput {
    /// Retains and validates a caller-supplied console input handle.
    ///
    /// # Errors
    ///
    /// Returns the Win32 error from inspecting the exact handle when it is not a usable console
    /// input endpoint.
    pub fn admit(handle: OwnedHandle) -> io::Result<Self> {
        let input = Self { handle };
        input.mode()?;
        crate::sys::console_input_count(input.handle.as_handle())?;
        Ok(input)
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

    /// Returns one currently ready record without reading after an observed zero count.
    ///
    /// The caller must provide one serialized reader for this endpoint. Once readiness is positive,
    /// this method requests exactly one record and requires Win32 to report exactly one result.
    ///
    /// # Errors
    ///
    /// Returns a raw Win32 error or `InvalidData` when Win32 reports an impossible read count.
    pub fn read_ready_record(&self) -> io::Result<Option<InputRecord>> {
        let mut port = NativeInputPort {
            handle: self.handle.as_handle(),
        };
        read_ready_record(&mut port)
    }
}

trait InputPort {
    fn ready_count(&mut self) -> io::Result<u32>;
    fn read_one(&mut self) -> io::Result<(InputRecord, u32)>;
}

struct NativeInputPort<'handle> {
    handle: BorrowedHandle<'handle>,
}

impl InputPort for NativeInputPort<'_> {
    fn ready_count(&mut self) -> io::Result<u32> {
        crate::sys::console_input_count(self.handle)
    }

    fn read_one(&mut self) -> io::Result<(InputRecord, u32)> {
        crate::sys::read_console_input(self.handle).map(|record| (record, 1))
    }
}

fn read_ready_record(port: &mut impl InputPort) -> io::Result<Option<InputRecord>> {
    if port.ready_count()? == 0 {
        return Ok(None);
    }
    let (record, count) = port.read_one()?;
    if count != 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("ReadConsoleInputW returned {count} records for a one-record buffer"),
        ));
    }
    Ok(Some(record))
}

#[cfg(test)]
mod tests;
