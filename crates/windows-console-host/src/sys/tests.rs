//! Safe representation tests for the Win32 tagged-union boundary.

use super::{
    create_wait_timer, create_wake_event, decode_input_record, signal_wake_event,
    validated_input_records, wait_input, win32_io_error,
};
use crate::{InputRecord, InputWait, KeyRecord, MouseRecord};
use std::{os::windows::io::AsHandle, time::Duration};
use windows::{
    Win32::System::Console::{
        COORD, FOCUS_EVENT, FOCUS_EVENT_RECORD, INPUT_RECORD, INPUT_RECORD_0, KEY_EVENT,
        KEY_EVENT_RECORD, KEY_EVENT_RECORD_0, MENU_EVENT, MENU_EVENT_RECORD, MOUSE_EVENT,
        MOUSE_EVENT_RECORD, WINDOW_BUFFER_SIZE_EVENT, WINDOW_BUFFER_SIZE_RECORD,
    },
    core::{BOOL, Error, HRESULT},
};

#[test]
fn a_wait_names_the_object_that_ended_it_and_consumes_a_wake() {
    // Any waitable object stands in for the console handle at this boundary.
    let input = create_wake_event().unwrap();
    let wake = create_wake_event().unwrap();
    let timer = create_wait_timer().unwrap();
    let wait = |timeout| {
        wait_input(
            input.as_handle(),
            Some(wake.as_handle()),
            Some(timer.as_handle()),
            timeout,
        )
        .unwrap()
    };

    assert_eq!(wait(Duration::from_millis(1)), InputWait::TimedOut);
    signal_wake_event(wake.as_handle()).unwrap();
    assert_eq!(wait(Duration::from_secs(5)), InputWait::Woken);
    assert_eq!(wait(Duration::from_millis(1)), InputWait::TimedOut);
    signal_wake_event(input.as_handle()).unwrap();
    signal_wake_event(wake.as_handle()).unwrap();
    assert_eq!(wait(Duration::from_secs(5)), InputWait::Input);
    assert_eq!(wait(Duration::from_secs(5)), InputWait::Woken);
}

#[test]
fn unicode_key_fields_cross_the_union_boundary_unchanged() {
    let record = INPUT_RECORD {
        EventType: u16::try_from(KEY_EVENT).unwrap(),
        Event: INPUT_RECORD_0 {
            KeyEvent: KEY_EVENT_RECORD {
                bKeyDown: BOOL(1),
                wRepeatCount: 3,
                wVirtualKeyCode: 0x41,
                wVirtualScanCode: 0x1e,
                uChar: KEY_EVENT_RECORD_0 {
                    UnicodeChar: 0xd83d,
                },
                dwControlKeyState: 0x1122,
            },
        },
    };

    assert_eq!(
        decode_input_record(record),
        InputRecord::Key(KeyRecord {
            key_down: true,
            repeat_count: 3,
            virtual_key_code: 0x41,
            virtual_scan_code: 0x1e,
            utf16_unit: 0xd83d,
            control_key_state: 0x1122,
        })
    );
}

#[test]
fn mouse_focus_resize_and_menu_fields_cross_unchanged() {
    let mouse = INPUT_RECORD {
        EventType: u16::try_from(MOUSE_EVENT).unwrap(),
        Event: INPUT_RECORD_0 {
            MouseEvent: MOUSE_EVENT_RECORD {
                dwMousePosition: COORD { X: -7, Y: 12 },
                dwButtonState: 0x1020,
                dwControlKeyState: 0x3040,
                dwEventFlags: 0x5060,
            },
        },
    };
    assert_eq!(
        decode_input_record(mouse),
        InputRecord::Mouse(MouseRecord {
            x: -7,
            y: 12,
            button_state: 0x1020,
            control_key_state: 0x3040,
            event_flags: 0x5060,
        })
    );

    let focus = INPUT_RECORD {
        EventType: u16::try_from(FOCUS_EVENT).unwrap(),
        Event: INPUT_RECORD_0 {
            FocusEvent: FOCUS_EVENT_RECORD { bSetFocus: BOOL(1) },
        },
    };
    assert_eq!(decode_input_record(focus), InputRecord::Focus(true));

    let resize = INPUT_RECORD {
        EventType: u16::try_from(WINDOW_BUFFER_SIZE_EVENT).unwrap(),
        Event: INPUT_RECORD_0 {
            WindowBufferSizeEvent: WINDOW_BUFFER_SIZE_RECORD {
                dwSize: COORD { X: 120, Y: 40 },
            },
        },
    };
    assert_eq!(
        decode_input_record(resize),
        InputRecord::Resize {
            columns: 120,
            rows: 40,
        }
    );

    let menu = INPUT_RECORD {
        EventType: u16::try_from(MENU_EVENT).unwrap(),
        Event: INPUT_RECORD_0 {
            MenuEvent: MENU_EVENT_RECORD { dwCommandId: 91 },
        },
    };
    assert_eq!(
        decode_input_record(menu),
        InputRecord::Menu { command_id: 91 }
    );
}

#[test]
fn unknown_tag_does_not_require_a_union_arm() {
    let record = INPUT_RECORD {
        EventType: 0xffff,
        Event: INPUT_RECORD_0::default(),
    };
    assert_eq!(
        decode_input_record(record),
        InputRecord::Unknown { event_type: 0xffff }
    );
}

#[test]
fn invalid_read_count_is_rejected_before_record_admission() {
    let records = [INPUT_RECORD::default(); 2];
    for count in [0, 3, u32::MAX] {
        assert_eq!(
            validated_input_records(&records, count).unwrap_err().kind(),
            std::io::ErrorKind::InvalidData
        );
    }
}

#[test]
fn win32_hresult_errors_preserve_the_raw_operating_system_code() {
    let error = win32_io_error(Error::from_hresult(HRESULT::from_win32(1234)));

    assert_eq!(error.raw_os_error(), Some(1234));
}
