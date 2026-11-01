use super::*;
use orchbee_windows_console_host::KeyRecord;
use std::collections::VecDeque;

struct PacketConsole {
    records: VecDeque<InputRecord>,
    now: Instant,
}

impl PacketConsole {
    fn new() -> Self {
        Self {
            records: VecDeque::new(),
            now: Instant::now(),
        }
    }

    fn read(&mut self, codec: &mut Codec, pending: &mut Option<Pending>) -> Option<Vec<u8>> {
        collect_input(self, codec, pending, Duration::ZERO, || false).unwrap()
    }
}

impl RecordSource for PacketConsole {
    fn read_ready(&mut self) -> io::Result<Option<InputRecord>> {
        Ok(self.records.pop_front())
    }

    fn window_origin(&self) -> io::Result<(i16, i16)> {
        Ok((0, 0))
    }

    fn now(&self) -> Instant {
        self.now
    }

    fn park(&mut self, duration: Duration) {
        self.now += duration;
    }
}

fn key(vk: u16, unit: u16, down: bool, repeat: u16, control: u32) -> InputRecord {
    InputRecord::Key(KeyRecord {
        key_down: down,
        repeat_count: repeat,
        virtual_key_code: vk,
        virtual_scan_code: 0,
        utf16_unit: unit,
        control_key_state: control,
    })
}

fn text_records(text: &str, alt_codes: bool) -> Vec<InputRecord> {
    text.encode_utf16()
        .flat_map(|unit| {
            if alt_codes && (0xd800..=0xdfff).contains(&unit) {
                // Native ConPTY emits each supplementary UTF-16 unit on
                // VK_MENU release, with a character-free Alt press before it.
                [key(0x12, 0, true, 1, 2), key(0x12, unit, false, 1, 0)]
            } else {
                [key(0, unit, true, 1, 0), key(0, unit, false, 1, 0)]
            }
        })
        .collect()
}

#[test]
fn supplementary_text_and_paste_are_exact_at_every_native_record_split() {
    for text in [
        "A中𠮷🚀B",
        "\x1b[200~A中𠮷🚀🇨🇳B\x1b[201~",
        "\u{10000}\u{10ffff}�",
    ] {
        for alt_codes in [false, true] {
            let records = text_records(text, alt_codes);
            for split in 0..=records.len() {
                let mut source = PacketConsole::new();
                let mut codec = Codec::default();
                let mut pending = None;
                source.records.extend(records[..split].iter().copied());
                let mut actual = source.read(&mut codec, &mut pending).unwrap_or_default();
                // Idle polling and phase changes do not complete a surrogate.
                source.now += Duration::from_secs(1);
                assert!(source.read(&mut codec, &mut pending).is_none());
                source.records.extend(records[split..].iter().copied());
                actual.extend(source.read(&mut codec, &mut pending).unwrap_or_default());
                assert_eq!(actual, text.as_bytes(), "split={split}, Alt={alt_codes}");
                assert!(codec.high.is_none());
                assert!(pending.is_none());
            }
        }
    }
}

#[test]
fn character_free_records_cannot_turn_a_surrogate_pair_into_replacement_text() {
    let mut codec = Codec::default();
    let mut actual = Vec::new();
    for record in [
        key(0, 0xd842, true, 1, 0),
        key(0xe5, 0, true, 1, 0), // VK_PROCESSKEY: no committed IME text.
        key(0xe7, 0, true, 1, 0), // VK_PACKET without a character.
        key(0x10, 0, true, 1, 0x10),
        key(0x10, 0, false, 1, 0),
        InputRecord::Resize {
            columns: 120,
            rows: 40,
        },
        InputRecord::Menu { command_id: 0 },
        InputRecord::Unknown { event_type: 0xffff },
        key(0, 0xdfb7, true, 1, 0),
    ] {
        actual.extend(bytes(codec.record(record, None).unwrap()));
    }
    assert_eq!(actual, "𠮷".as_bytes());
}

#[test]
fn supplementary_repeat_tails_stay_ordered_when_a_read_splits_utf8() {
    let mut source = PacketConsole::new();
    source.records.extend([
        key(0, u16::from(b'A'), true, 1, 0),
        key(0, 0xd83d, true, 5000, 0),
        key(0, 0xde80, true, 5000, 0),
        key(0, u16::from(b'B'), true, 1, 0),
    ]);
    let mut codec = Codec::default();
    let mut pending = None;
    let mut decoder = crate::terminal_input::LegacyDecoder::default();
    let mut decoded = String::new();
    let mut actual = Vec::new();
    while let Some(packet) = source.read(&mut codec, &mut pending) {
        assert!(packet.len() <= BURST);
        for &byte in &packet {
            for item in decoder.push(byte, source.now, false) {
                let crate::terminal_input::DecodedKind::Key(crate::client_view::KeyEvent {
                    code: crate::client_view::KeyCode::Char(ch),
                    ..
                }) = item.kind
                else {
                    panic!("text must decode as a complete scalar: {item:?}");
                };
                decoded.push(ch);
            }
        }
        actual.extend(packet);
    }
    let expected = format!("A{}B", "🚀".repeat(5000));
    assert_eq!(actual, expected.as_bytes());
    assert_eq!(decoded, expected);
    assert!(pending.is_none());
    assert!(codec.high.is_none());
}

#[test]
fn malformed_utf16_and_inconsistent_pair_metadata_refuse_the_read() {
    for records in [
        vec![key(0, 0xdc00, true, 1, 0)],
        vec![key(0, 0xd83d, true, 1, 0), key(0, 0xd842, true, 1, 0)],
        vec![
            key(0, 0xd83d, true, 1, 0),
            key(0, u16::from(b'x'), true, 1, 0),
        ],
        vec![key(0, 0xd83d, true, 1, 0), key(13, 13, true, 1, 0)],
        vec![key(0, 0xd83d, true, 2, 0), key(0, 0xde80, true, 1, 0)],
        vec![
            key(0xe7, 0xd83d, true, 1, 0),
            key(0xe7, 0xde80, true, 1, 0x10),
        ],
    ] {
        let mut source = PacketConsole::new();
        source.records.extend(records);
        let result = collect_input(
            &mut source,
            &mut Codec::default(),
            &mut None,
            Duration::ZERO,
            || false,
        );
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::InvalidData);
    }
}
