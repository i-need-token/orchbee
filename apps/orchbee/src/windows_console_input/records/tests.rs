use super::*;
use std::cell::Cell;

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
    fn now(&self) -> Instant {
        self.origin + self.elapsed
    }
    fn park(&mut self, duration: Duration) {
        assert!(duration > Duration::ZERO && duration <= Duration::from_millis(1));
        self.elapsed += duration;
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
#[test]
fn zero_virtual_key_keeps_paste_delimiters_and_literal_control_characters() {
    let mut codec = Codec::default();
    let paste = "\x1b[200~中\x02q\x1b[201~";
    let mut actual = Vec::new();
    for unit in paste.encode_utf16() {
        actual.extend(bytes(codec.key(
            0,
            unit,
            true,
            1,
            KeyModifiers::NONE,
            false,
        )));
    }
    assert_eq!(actual, paste.as_bytes());
}
#[test]
fn surrogate_state_and_large_repeats_survive_poll_boundaries_without_reordering() {
    let mut codec = Codec::default();
    assert!(
        codec
            .key(0, 0xd83d, true, 1, KeyModifiers::NONE, false)
            .is_none()
    );
    assert_eq!(
        bytes(codec.key(0, 0xde00, true, 1, KeyModifiers::NONE, false)),
        "😀".as_bytes()
    );
    let mut pending = codec
        .key(0, u16::from(b'x'), true, 5000, KeyModifiers::NONE, false)
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
fn physical_prefix_repeat_does_not_become_a_second_press_and_alt_graph_is_text() {
    let mut codec = Codec::default();
    assert_eq!(
        bytes(codec.key(0x42, 2, true, 5, KeyModifiers::CONTROL, false)),
        [2]
    );
    assert!(
        codec
            .key(0x42, 2, true, 1, KeyModifiers::CONTROL, false)
            .is_none()
    );
    assert!(
        codec
            .key(0x42, 2, false, 1, KeyModifiers::CONTROL, false)
            .is_none()
    );
    assert_eq!(
        bytes(codec.key(0x42, 2, true, 1, KeyModifiers::CONTROL, false)),
        [2]
    );
    assert_eq!(
        bytes(codec.key(
            0x51,
            u16::from(b'@'),
            true,
            1,
            KeyModifiers::CONTROL | KeyModifiers::ALT,
            true
        )),
        b"@"
    );
    assert_eq!(
        bytes(codec.key(0x26, 0, true, 1, KeyModifiers::CONTROL, false)),
        b"\x1b[1;5A"
    );
}
