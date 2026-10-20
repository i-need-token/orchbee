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
fn opaque_graphics_and_prompt_rows_cannot_be_mistaken_for_unstyled_blank_space() {
    let mut empty = vec![0_u8; 40];
    empty[..7].copy_from_slice(b"OBKG\0\0\x02");
    empty[7] = 1;
    empty[8..12].copy_from_slice(&40_u32.to_le_bytes());
    assert_eq!(
        decode_terminal_text_frame(&frame(&[(2, 0, &empty)]))
            .unwrap()
            .graphics_present,
        Some(false)
    );
    empty[7] = 2;
    assert_eq!(
        decode_terminal_text_frame(&frame(&[(2, 0, &empty)]))
            .unwrap()
            .graphics_present,
        Some(false)
    );
    empty[30] = 1;
    assert_eq!(
        decode_terminal_text_frame(&frame(&[(2, 0, &empty)]))
            .unwrap()
            .graphics_present,
        Some(true)
    );
    assert_eq!(
        decode_terminal_text_frame(&frame(&[(2, 0, &[1, 2, 3])]))
            .unwrap()
            .graphics_present,
        Some(true)
    );
    assert_eq!(
        decode_terminal_text_frame(&frame(&[]))
            .unwrap()
            .graphics_present,
        None
    );
    let mut bytes = frame(&[]);
    let mut row_start = 160;
    for _ in 0..u32_at(&bytes, 124) {
        row_start += 4 + u32_at(&bytes, row_start);
    }
    row_start += 20 * u16_at(&bytes, 128);
    bytes[row_start + 3] = 2;
    assert_eq!(
        decode_terminal_text_frame(&bytes).unwrap().rows[0].prompt_class,
        2
    );
}

#[test]
fn command_completion_uses_the_gui_wire_contract_and_rejects_ambiguous_facts() {
    let payload = |sequence: u64, duration: u64, completed: u64, exit: u8| {
        let mut bytes = Vec::new();
        bytes.extend(sequence.to_le_bytes());
        bytes.extend(duration.to_le_bytes());
        bytes.extend(completed.to_le_bytes());
        bytes.push(exit);
        bytes
    };
    for (sequence, duration, completed, exit) in [(1, 0, 1, 0), (u64::MAX, u64::MAX, u64::MAX, 255)]
    {
        let bytes = payload(sequence, duration, completed, exit);
        let completion = decode_terminal_text_frame(&frame(&[(4, 0, &bytes)]))
            .unwrap()
            .command_completion
            .unwrap();
        assert_eq!(completion.sequence.get(), sequence);
        assert_eq!(completion.duration_milliseconds, duration);
        assert_eq!(completion.completed_at_milliseconds.get(), completed);
        assert_eq!(completion.exit_code, exit);
        assert!(decode_terminal_text_frame(&frame(&[(4, 0, &bytes), (4, 0, &bytes)])).is_err());
    }
    assert!(
        decode_terminal_text_frame(&frame(&[]))
            .unwrap()
            .command_completion
            .is_none()
    );
    for bytes in [
        payload(0, 1, 1, 0),
        payload(1, 1, 0, 0),
        vec![1; 24],
        vec![1; 26],
    ] {
        assert!(decode_terminal_text_frame(&frame(&[(4, 0, &bytes)])).is_err());
    }
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

#[test]
fn correlated_command_observations_are_closed_and_transactional() {
    let mut payload = vec![0_u8; 66];
    payload[..8].copy_from_slice(&1_u64.to_le_bytes());
    payload[8] = 1;
    assert!(
        decode_terminal_text_frame(&frame(&[(5, 0, &payload[..10])]))
            .unwrap()
            .command_observation
            .unwrap()
            .ready
    );
    payload[9] = 2;
    payload[10..26].fill(0x11);
    payload[26..34].copy_from_slice(&2_u64.to_le_bytes());
    payload[36..44].copy_from_slice(&4_u64.to_le_bytes());
    payload[46..54].copy_from_slice(&3_u64.to_le_bytes());
    payload[54..62].copy_from_slice(&1_u64.to_le_bytes());
    payload[62..66].copy_from_slice(&1_i32.to_le_bytes());
    let command = decode_terminal_text_frame(&frame(&[(5, 0, &payload)]))
        .unwrap()
        .command_observation
        .unwrap()
        .command
        .unwrap();
    assert!(command.completed);
    assert_eq!(command.start_row, 2);
    assert_eq!(command.end_row, 4);
    assert_eq!(command.exit_code, 1);
    assert_eq!(command.id, [0x11; 16]);
    assert!(decode_terminal_text_frame(&frame(&[(5, 0, &payload), (5, 0, &payload)])).is_err());
    for (offset, value) in [(0, 0), (8, 2), (9, 3), (46, 0), (54, 0)] {
        let mut invalid = payload.clone();
        invalid[offset] = value;
        assert!(decode_terminal_text_frame(&frame(&[(5, 0, &invalid)])).is_err());
    }
    payload[9] = 1;
    assert!(decode_terminal_text_frame(&frame(&[(5, 0, &payload)])).is_err());
}
