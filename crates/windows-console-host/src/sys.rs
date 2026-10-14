//! The sole Win32 FFI and tagged-union boundary behind the crate's safe API.

use crate::{InputRecord, KeyRecord, MouseRecord};
use std::{
    io,
    os::windows::io::{AsRawHandle, BorrowedHandle},
};
use windows::Win32::{
    Foundation::HANDLE,
    System::Console::{
        CONSOLE_MODE, CONSOLE_SCREEN_BUFFER_INFO, FOCUS_EVENT, GetConsoleMode,
        GetConsoleScreenBufferInfo, GetNumberOfConsoleInputEvents, INPUT_RECORD, KEY_EVENT,
        MENU_EVENT, MOUSE_EVENT, ReadConsoleInputW, SetConsoleMode, WINDOW_BUFFER_SIZE_EVENT,
        WriteConsoleW,
    },
};

pub(super) fn console_mode(handle: BorrowedHandle<'_>) -> io::Result<u32> {
    let mut mode = CONSOLE_MODE::default();
    // SAFETY: `handle` is live for the call and `mode` is a writable value of the exact ABI type.
    unsafe { GetConsoleMode(raw_handle(handle), &mut mode) }.map_err(win32_io_error)?;
    Ok(mode.0)
}

pub(super) fn set_console_mode(handle: BorrowedHandle<'_>, mode: u32) -> io::Result<()> {
    // SAFETY: `handle` is live for the call and the mode word is passed by value.
    unsafe { SetConsoleMode(raw_handle(handle), CONSOLE_MODE(mode)) }.map_err(win32_io_error)
}

pub(super) fn console_input_count(handle: BorrowedHandle<'_>) -> io::Result<u32> {
    let mut count = 0;
    // SAFETY: `handle` is live for the call and `count` is a writable `u32`.
    unsafe { GetNumberOfConsoleInputEvents(raw_handle(handle), &mut count) }
        .map_err(win32_io_error)?;
    Ok(count)
}

pub(super) fn read_console_input(handle: BorrowedHandle<'_>) -> io::Result<InputRecord> {
    let mut records = [INPUT_RECORD::default()];
    let mut count = 0;
    // SAFETY: `handle` is live, the one-element slice is writable for its reported length, and
    // `count` remains live through the call. This module validates the returned count before it
    // reads any tagged-union arm.
    unsafe { ReadConsoleInputW(raw_handle(handle), &mut records, &mut count) }
        .map_err(win32_io_error)?;
    validated_input_record(records[0], count)
}

pub(super) fn console_window(handle: BorrowedHandle<'_>) -> io::Result<(i16, i16, i16, i16)> {
    let mut info = CONSOLE_SCREEN_BUFFER_INFO::default();
    // SAFETY: `handle` is live and `info` is a writable value of the exact ABI type.
    unsafe { GetConsoleScreenBufferInfo(raw_handle(handle), &mut info) }.map_err(win32_io_error)?;
    Ok((
        info.srWindow.Left,
        info.srWindow.Top,
        info.srWindow.Right,
        info.srWindow.Bottom,
    ))
}

pub(super) fn write_console(handle: BorrowedHandle<'_>, utf16_units: &[u16]) -> io::Result<usize> {
    let mut written = 0;
    // SAFETY: `handle` is live, `utf16_units` remains readable for the call, `written` is writable,
    // and the reserved pointer is null as required by `WriteConsoleW`.
    unsafe {
        WriteConsoleW(
            raw_handle(handle),
            utf16_units,
            Some(std::ptr::from_mut(&mut written)),
            None,
        )
    }
    .map_err(win32_io_error)?;
    Ok(usize::try_from(written).unwrap_or(usize::MAX))
}

fn raw_handle(handle: BorrowedHandle<'_>) -> HANDLE {
    HANDLE(handle.as_raw_handle())
}

fn win32_io_error(error: windows::core::Error) -> io::Error {
    let hresult = error.code().0;
    let bits = u32::from_ne_bytes(hresult.to_ne_bytes());
    let raw = if bits & 0xffff_0000 == 0x8007_0000 {
        i32::try_from(bits & 0x0000_ffff).unwrap_or(hresult)
    } else {
        hresult
    };
    io::Error::from_raw_os_error(raw)
}

fn decode_input_record(record: INPUT_RECORD) -> InputRecord {
    match u32::from(record.EventType) {
        KEY_EVENT => {
            // SAFETY: Win32 identifies this `INPUT_RECORD` as the key arm. Because it came from
            // `ReadConsoleInputW`, the nested character union contains `UnicodeChar`.
            let event = unsafe { record.Event.KeyEvent };
            // SAFETY: The W-suffixed read above establishes the Unicode arm of this key record.
            let utf16_unit = unsafe { event.uChar.UnicodeChar };
            InputRecord::Key(KeyRecord {
                key_down: event.bKeyDown.as_bool(),
                repeat_count: event.wRepeatCount,
                virtual_key_code: event.wVirtualKeyCode,
                virtual_scan_code: event.wVirtualScanCode,
                utf16_unit,
                control_key_state: event.dwControlKeyState,
            })
        }
        MOUSE_EVENT => {
            // SAFETY: Win32 identifies this `INPUT_RECORD` as the mouse arm before it is read.
            let event = unsafe { record.Event.MouseEvent };
            InputRecord::Mouse(MouseRecord {
                x: event.dwMousePosition.X,
                y: event.dwMousePosition.Y,
                button_state: event.dwButtonState,
                control_key_state: event.dwControlKeyState,
                event_flags: event.dwEventFlags,
            })
        }
        WINDOW_BUFFER_SIZE_EVENT => {
            // SAFETY: Win32 identifies this `INPUT_RECORD` as the resize arm before it is read.
            let event = unsafe { record.Event.WindowBufferSizeEvent };
            InputRecord::Resize {
                columns: event.dwSize.X,
                rows: event.dwSize.Y,
            }
        }
        MENU_EVENT => {
            // SAFETY: Win32 identifies this `INPUT_RECORD` as the menu arm before it is read.
            let event = unsafe { record.Event.MenuEvent };
            InputRecord::Menu {
                command_id: event.dwCommandId,
            }
        }
        FOCUS_EVENT => {
            // SAFETY: Win32 identifies this `INPUT_RECORD` as the focus arm before it is read.
            let event = unsafe { record.Event.FocusEvent };
            InputRecord::Focus(event.bSetFocus.as_bool())
        }
        _ => InputRecord::Unknown {
            event_type: record.EventType,
        },
    }
}

fn validated_input_record(record: INPUT_RECORD, count: u32) -> io::Result<InputRecord> {
    if count != 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("ReadConsoleInputW returned {count} records for a one-record buffer"),
        ));
    }
    Ok(decode_input_record(record))
}

#[cfg(test)]
#[deny(unsafe_code)]
mod tests;
