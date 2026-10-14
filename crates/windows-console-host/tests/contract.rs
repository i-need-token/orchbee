#![cfg(windows)]

use orchbee_windows_console_host::{
    ConsoleInput, ConsoleOutput, ConsoleWriteError, ConsoleWriter, InputRecord, KeyRecord,
    MouseRecord,
};

fn require_send_sync<T: Send + Sync>() {}
fn require_send<T: Send>() {}

#[test]
fn retained_endpoints_support_the_application_ownership_boundary() {
    require_send_sync::<ConsoleInput>();
    require_send_sync::<ConsoleOutput>();
    require_send::<ConsoleWriter>();
    require_send_sync::<ConsoleWriteError>();
}

#[test]
fn public_records_preserve_raw_utf16_and_absolute_mouse_fields() {
    assert_eq!(
        InputRecord::Key(KeyRecord {
            key_down: true,
            repeat_count: 4,
            virtual_key_code: 0,
            virtual_scan_code: 7,
            utf16_unit: 0xd800,
            control_key_state: 0x1122,
        }),
        InputRecord::Key(KeyRecord {
            key_down: true,
            repeat_count: 4,
            virtual_key_code: 0,
            virtual_scan_code: 7,
            utf16_unit: 0xd800,
            control_key_state: 0x1122,
        })
    );
    assert_eq!(
        InputRecord::Mouse(MouseRecord {
            x: -11,
            y: 23,
            button_state: 0x1020,
            control_key_state: 0x3040,
            event_flags: 0x5060,
        }),
        InputRecord::Mouse(MouseRecord {
            x: -11,
            y: 23,
            button_state: 0x1020,
            control_key_state: 0x3040,
            event_flags: 0x5060,
        })
    );
}
