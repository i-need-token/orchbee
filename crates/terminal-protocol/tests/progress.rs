use orchbee_terminal_protocol::{TerminalProgress, decode_terminal_text_frame};
const FRAME: &[u8] =
    include_bytes!("../../../packages/terminal-protocol/fixtures/terminal-frame-v0.0.3.bin");
fn u16_at(bytes: &[u8], n: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[n..n + 2].try_into().unwrap()))
}
fn u32_at(bytes: &[u8], n: usize) -> usize {
    usize::try_from(u32::from_le_bytes(bytes[n..n + 4].try_into().unwrap())).unwrap()
}
fn frame(extensions: &[(u16, u16, &[u8])]) -> Vec<u8> {
    // Preserve the real contract-authored tables; replace only their optional extension list.
    let mut end = 160;
    for _ in 0..u32_at(FRAME, 124) {
        end += 4 + u32_at(FRAME, end);
    }
    end += 20 * u16_at(FRAME, 128);
    for _ in 0..u16_at(FRAME, 130) {
        end += 8 + 8 * u16_at(FRAME, end + 4);
    }
    let mut bytes = FRAME[..end].to_vec();
    bytes[132..134].copy_from_slice(&u16::try_from(extensions.len()).unwrap().to_le_bytes());
    for (kind, flags, data) in extensions {
        bytes.extend(kind.to_le_bytes());
        bytes.extend(flags.to_le_bytes());
        bytes.extend(u32::try_from(data.len()).unwrap().to_le_bytes());
        bytes.extend_from_slice(data);
    }
    let len = u32::try_from(bytes.len()).unwrap();
    bytes[8..12].copy_from_slice(&len.to_le_bytes());
    bytes
}
#[test]
fn all_progress_state_and_value_bytes_obey_closed_wire_combinations() {
    for state in 0..=4 {
        for value in 0..=255 {
            let bytes = frame(&[(3, 0, &[state, value])]);
            let actual = decode_terminal_text_frame(&bytes);
            let accepted = match state {
                0 | 3 => value == 255,
                1 | 2 | 4 => value <= 100 || value == 255,
                _ => unreachable!(),
            };
            assert_eq!(actual.is_ok(), accepted, "state={state} value={value}");
            if let Ok(f) = actual {
                let progress = f.progress.expect("present extension");
                let got = match progress {
                    TerminalProgress::Remove => {
                        assert_eq!(state, 0);
                        None
                    }
                    TerminalProgress::Set(p) => {
                        assert_eq!(state, 1);
                        p
                    }
                    TerminalProgress::Error(p) => {
                        assert_eq!(state, 2);
                        p
                    }
                    TerminalProgress::Indeterminate => {
                        assert_eq!(state, 3);
                        None
                    }
                    TerminalProgress::Pause(p) => {
                        assert_eq!(state, 4);
                        p
                    }
                };
                assert_eq!(got.map(|p| p.percent()), (value != 255).then_some(value));
            }
        }
    }
    for state in 5..=255 {
        assert!(decode_terminal_text_frame(&frame(&[(3, 0, &[state, 255])])).is_err());
    }
}
#[test]
fn presence_duplicate_length_flags_and_optional_unknown_extensions_are_distinct() {
    assert_eq!(
        decode_terminal_text_frame(&frame(&[])).unwrap().progress,
        None
    );
    assert_eq!(
        decode_terminal_text_frame(&frame(&[(3, 0, &[0, 255])]))
            .unwrap()
            .progress,
        Some(TerminalProgress::Remove)
    );
    for bytes in [
        frame(&[(3, 0, &[])]),
        frame(&[(3, 0, &[1])]),
        frame(&[(3, 0, &[1, 42, 0])]),
        frame(&[(3, 0, &[1, 42]), (3, 0, &[0, 255])]),
        frame(&[(3, 0x8000, &[1, 42])]),
    ] {
        assert!(decode_terminal_text_frame(&bytes).is_err());
    }
    let bytes = frame(&[(65000, 0, &[255, 255, 255]), (3, 0, &[4, 100])]);
    assert!(
        matches!(decode_terminal_text_frame(&bytes).unwrap().progress,Some(TerminalProgress::Pause(Some(p))) if p.percent()==100)
    );
    for end in 0..bytes.len() {
        assert!(decode_terminal_text_frame(&bytes[..end]).is_err());
    }
}
