use super::*;
use std::cell::Cell;

mod unicode;

struct EmptyConsole<'a> {
    origin: Instant,
    elapsed: Duration,
    waits: usize,
    notification: &'a Cell<bool>,
}

impl RecordSource for EmptyConsole<'_> {
    fn read_ready(&mut self) -> io::Result<Option<InputRecord>> {
        Ok(None)
    }
    fn window_origin(&self) -> io::Result<(i16, i16)> {
        Ok((0, 0))
    }
    fn now(&self) -> Instant {
        self.origin + self.elapsed
    }
    fn park(&mut self, duration: Duration) {
        assert!(duration > Duration::ZERO);
        // A publication ends the wait at the next millisecond of this controlled clock.
        self.elapsed += duration.min(Duration::from_millis(1));
        self.waits += 1;
        if self.elapsed == Duration::from_millis(5) {
            self.notification.set(true);
        }
    }
}

#[test]
fn worker_notification_ends_the_fifty_millisecond_console_wait_at_publication() {
    let notification = Cell::new(false);
    let mut console = EmptyConsole {
        origin: Instant::now(),
        elapsed: Duration::ZERO,
        waits: 0,
        notification: &notification,
    };
    let mut codec = Codec::default();
    let mut pending = None;
    assert!(
        collect_input(
            &mut console,
            &mut codec,
            &mut pending,
            Duration::from_millis(50),
            || notification.replace(false)
        )
        .unwrap()
        .is_none()
    );
    assert_eq!(console.elapsed, Duration::from_millis(5));
    assert_eq!(console.waits, 5);
    console.elapsed = Duration::ZERO;
    console.waits = 0;
    // Control: the previous input-only wait ignores exactly the same event.
    assert!(
        collect_input(
            &mut console,
            &mut codec,
            &mut pending,
            Duration::from_millis(50),
            || false
        )
        .unwrap()
        .is_none()
    );
    assert_eq!(console.elapsed, Duration::from_millis(50));
    assert_eq!(console.waits, 50);
    println!(
        "controlled console trace: publication=5ms; wake-aware return=5ms; input-only return=50ms"
    );

    // A wake never consumes or discards the reader's pending repeat tail.
    pending = Pending::new(b"x".to_vec(), 5000);
    let first = collect_input(
        &mut console,
        &mut codec,
        &mut pending,
        Duration::ZERO,
        || true,
    )
    .unwrap()
    .unwrap();
    let second = collect_input(
        &mut console,
        &mut codec,
        &mut pending,
        Duration::ZERO,
        || true,
    )
    .unwrap()
    .unwrap();
    assert_eq!(first.len(), BURST);
    assert_eq!([first, second].concat(), vec![b'x'; 5000]);
    assert!(pending.is_none());
}

fn bytes(pending: Option<Pending>) -> Vec<u8> {
    let mut output = Vec::new();
    if let Some(mut pending) = pending {
        pending.drain(&mut output);
        assert_eq!(pending.remaining, 0);
    }
    output
}

fn mouse_bytes(codec: &mut Codec, buttons: u32, flags: u32, modifiers: KeyModifiers) -> Vec<u8> {
    bytes(codec.mouse(4, 9, buttons, flags, modifiers))
}

#[test]
fn zero_virtual_key_keeps_paste_delimiters_and_literal_control_characters() {
    let mut codec = Codec::default();
    let paste = "\x1b[200~A中𠮷🚀B\x02q\x1b[201~";
    let mut actual = Vec::new();
    for unit in paste.encode_utf16() {
        actual.extend(bytes(
            codec
                .key(0, unit, true, 1, KeyModifiers::NONE, false)
                .unwrap(),
        ));
    }
    assert_eq!(actual, paste.as_bytes());
}
#[test]
fn surrogate_state_and_large_repeats_survive_poll_boundaries_without_reordering() {
    let mut codec = Codec::default();
    assert!(
        codec
            .key(0, 0xd83d, true, 1, KeyModifiers::NONE, false)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        bytes(
            codec
                .key(0, 0xde00, true, 1, KeyModifiers::NONE, false)
                .unwrap()
        ),
        "😀".as_bytes()
    );
    let mut pending = codec
        .key(0, u16::from(b'x'), true, 5000, KeyModifiers::NONE, false)
        .unwrap()
        .unwrap();
    let mut first = Vec::new();
    pending.drain(&mut first);
    assert_eq!(first, vec![b'x'; BURST]);
    let mut second = Vec::new();
    pending.drain(&mut second);
    assert_eq!(second, vec![b'x'; 5000 - BURST]);
    assert_eq!(pending.remaining, 0);
}
#[test]
fn mouse_records_keep_right_button_and_1002_drag_mappings() {
    let mut codec = Codec::default();
    // Windows button bits are left=1, right=2, middle=4. A held button
    // keeps ordinary `1002` movement on the drag path for each identity.
    let cases: &[(u32, u32, &[u8], u8)] = &[
        (2, 0, b"\x1b[<2;5;10M", 4),
        (0, 0, b"\x1b[<2;5;10m", 0),
        (1, MOUSE_MOVED, b"\x1b[<32;5;10M", 1),
        (4, MOUSE_MOVED, b"\x1b[<33;5;10M", 2),
        (2, MOUSE_MOVED, b"\x1b[<34;5;10M", 4),
    ];
    for &(buttons, flags, expected, held) in cases {
        assert_eq!(
            mouse_bytes(&mut codec, buttons, flags, KeyModifiers::NONE),
            expected
        );
        assert_eq!(codec.buttons, held);
    }
}

#[test]
fn mouse_record_coordinates_are_relative_to_the_admitted_output_window() {
    let mut codec = Codec::default();
    let record = InputRecord::Mouse(orchbee_windows_console_host::MouseRecord {
        x: 14,
        y: 29,
        button_state: 0,
        control_key_state: 0,
        event_flags: MOUSE_MOVED,
    });
    assert_eq!(
        bytes(codec.record(record, Some((10, 20))).unwrap()),
        b"\x1b[<35;5;10M"
    );
}

#[test]
fn button_free_motion_records_encode_the_sgr_hover_report_when_admitted() {
    // Model a host that delivers globally admitted button-free any-motion as
    // a Win32 MOUSE_MOVED record. The codec must preserve it for dispatch.
    let mut codec = Codec::default();
    assert_eq!(
        mouse_bytes(&mut codec, 0, MOUSE_MOVED, KeyModifiers::NONE),
        b"\x1b[<35;5;10M"
    );
    assert_eq!(codec.buttons, 0);
    assert_eq!(
        mouse_bytes(&mut codec, 0, MOUSE_MOVED, KeyModifiers::SHIFT),
        b"\x1b[<39;5;10M"
    );
}
#[test]
fn physical_prefix_repeat_does_not_become_a_second_press_and_alt_graph_is_text() {
    let mut codec = Codec::default();
    assert_eq!(
        bytes(
            codec
                .key(0x42, 2, true, 5, KeyModifiers::CONTROL, false)
                .unwrap()
        ),
        [2]
    );
    assert!(
        codec
            .key(0x42, 2, true, 1, KeyModifiers::CONTROL, false)
            .unwrap()
            .is_none()
    );
    assert!(
        codec
            .key(0x42, 2, false, 1, KeyModifiers::CONTROL, false)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        bytes(
            codec
                .key(0x42, 2, true, 1, KeyModifiers::CONTROL, false)
                .unwrap()
        ),
        [2]
    );
    assert_eq!(
        bytes(
            codec
                .key(
                    0x51,
                    u16::from(b'@'),
                    true,
                    1,
                    KeyModifiers::CONTROL | KeyModifiers::ALT,
                    true
                )
                .unwrap()
        ),
        b"@"
    );
    assert_eq!(
        bytes(
            codec
                .key(0x26, 0, true, 1, KeyModifiers::CONTROL, false)
                .unwrap()
        ),
        b"\x1b[1;5A"
    );
}
