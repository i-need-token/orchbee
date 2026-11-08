use orchbee_terminal_protocol::{
    TerminalGraphicsImageFormat, TerminalGraphicsUpdateKind, TerminalProgramState,
    TerminalProgress, decode_terminal_text_frame,
};
const FRAME: &[u8] = include_bytes!("../../../schemas/terminal/v1/fixtures/terminal-frame.bin");
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
fn graphics_record(kind: u8, image: bool, placement_image: u32) -> Vec<u8> {
    // Authored from the OBKG wire contract: one 2x1 RGB image and one placement.
    let mut bytes = vec![0_u8; 40];
    bytes[..8].copy_from_slice(&[b'O', b'B', b'K', b'G', 1, 0, 0, kind]);
    bytes[12..20].copy_from_slice(&7_u64.to_le_bytes());
    bytes[28..30].copy_from_slice(&u16::from(image).to_le_bytes());
    bytes[30..32].copy_from_slice(&1_u16.to_le_bytes());
    if image {
        bytes[32..36].copy_from_slice(&6_u32.to_le_bytes());
        bytes.extend(9_u32.to_le_bytes());
        bytes.extend([3, 0, 0, 0]);
        bytes.extend(1_u64.to_le_bytes());
        for field in [2_u32, 1, 6, 0] {
            bytes.extend(field.to_le_bytes());
        }
        bytes.extend([1, 2, 3, 4, 5, 6]);
    }
    for field in [
        placement_image,
        0,
        3,
        u32::MAX,
        0,
        0,
        0,
        0,
        2,
        1,
        20,
        10,
        0,
        0,
    ] {
        bytes.extend(field.to_le_bytes());
    }
    let total = u32::try_from(bytes.len()).unwrap();
    bytes[8..12].copy_from_slice(&total.to_le_bytes());
    bytes
}
#[test]
fn graphics_are_decoded_strictly_and_prompt_rows_cannot_be_mistaken_for_unstyled_blank_space() {
    let mut empty = vec![0_u8; 40];
    empty[..7].copy_from_slice(b"OBKG\x01\0\0");
    empty[7] = 1;
    empty[8..12].copy_from_slice(&40_u32.to_le_bytes());
    let decoded = decode_terminal_text_frame(&frame(&[(2, 0, &empty)])).unwrap();
    assert_eq!(decoded.graphics_present, Some(false));
    assert!(decoded.graphics.is_some_and(|graphics| graphics.is_empty()));
    // The fixture is a delta: its placement-only update names resources the projection retains.
    empty[7] = 2;
    let placements = graphics_record(2, false, 9);
    for (record, present) in [(&empty, false), (&placements, true)] {
        let decoded = decode_terminal_text_frame(&frame(&[(2, 0, record)])).unwrap();
        assert_eq!(decoded.graphics_present, Some(present));
        assert_eq!(
            decoded.graphics.unwrap().kind,
            TerminalGraphicsUpdateKind::Placements
        );
        // A terminal snapshot retains nothing such an update could name.
        let snapshot = |extensions: &[(u16, u16, &[u8])]| {
            let mut bytes = frame(extensions);
            bytes[6] = 1;
            bytes[56..64].copy_from_slice(&u64::MAX.to_le_bytes());
            decode_terminal_text_frame(&bytes)
        };
        assert!(snapshot(&[]).is_ok());
        assert!(snapshot(&[(2, 0, record)]).is_err());
    }
    let decoded =
        decode_terminal_text_frame(&frame(&[(2, 0, &graphics_record(1, true, 9))])).unwrap();
    assert_eq!(decoded.graphics_present, Some(true));
    let graphics = decoded.graphics.unwrap();
    assert_eq!(
        (graphics.kind, graphics.resource_revision),
        (TerminalGraphicsUpdateKind::Snapshot, 7)
    );
    let image = graphics.image(9).unwrap();
    assert_eq!(image.format, TerminalGraphicsImageFormat::Rgb);
    assert_eq!(image.pixels.as_slice(), &[1, 2, 3, 4, 5, 6]);
    let placement = graphics.placements[0];
    assert_eq!(
        (placement.column, placement.row, placement.destination_width),
        (3, -1, 20)
    );
    // An unknown image, malformed bytes, or a repeated record reject the whole frame.
    for record in [graphics_record(1, true, 8), vec![1, 2, 3]] {
        assert!(decode_terminal_text_frame(&frame(&[(2, 0, &record)])).is_err());
    }
    assert!(decode_terminal_text_frame(&frame(&[(2, 0, &empty), (2, 0, &empty)])).is_err());
    let absent = decode_terminal_text_frame(&frame(&[])).unwrap();
    assert_eq!(absent.graphics_present, None);
    assert!(absent.graphics.is_none());
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

#[test]
fn a_command_start_time_annotates_only_the_same_frames_invocation() {
    let mut running = vec![0_u8; 66];
    running[..8].copy_from_slice(&1_u64.to_le_bytes());
    running[9] = 1;
    running[10..26].fill(0x11);
    running[46..54].copy_from_slice(&3_u64.to_le_bytes());
    running[54..62].copy_from_slice(&1_u64.to_le_bytes());
    let start = 7_u64.to_le_bytes();
    let started = |extensions: &[(u16, u16, &[u8])]| {
        decode_terminal_text_frame(&frame(extensions)).map(|frame| {
            frame
                .command_observation
                .unwrap()
                .command
                .unwrap()
                .started_at
        })
    };
    for order in [
        [(5, 0, &running[..]), (6, 0, &start[..])],
        [(6, 0, &start[..]), (5, 0, &running[..])],
    ] {
        assert_eq!(
            started(&order).unwrap().map(std::num::NonZeroU64::get),
            Some(7)
        );
    }
    assert_eq!(started(&[(5, 0, &running)]).unwrap(), None);
    // Without an invocation to annotate, duplicated, zero, or mis-sized, the frame is invalid.
    let idle = &running[..10];
    let mut absent = idle.to_vec();
    absent[9] = 0;
    assert!(decode_terminal_text_frame(&frame(&[(6, 0, &start)])).is_err());
    assert!(decode_terminal_text_frame(&frame(&[(5, 0, &absent), (6, 0, &start)])).is_err());
    assert!(started(&[(5, 0, &running), (6, 0, &start), (6, 0, &start)]).is_err());
    assert!(started(&[(5, 0, &running), (6, 0, &0_u64.to_le_bytes())]).is_err());
    assert!(started(&[(5, 0, &running), (6, 0, &start[..7])]).is_err());
}

#[test]
fn program_status_states_and_report_times_obey_the_closed_wire_contract() {
    let payload = |state: u8, reported_at: u64| {
        let mut bytes = vec![state];
        bytes.extend(reported_at.to_le_bytes());
        bytes
    };
    let decoded = |extensions: &[(u16, u16, &[u8])]| {
        decode_terminal_text_frame(&frame(extensions)).map(|frame| frame.program_status)
    };
    assert_eq!(decoded(&[]).unwrap(), None);
    for (byte, state) in [
        (1, TerminalProgramState::Idle),
        (2, TerminalProgramState::Working),
        (3, TerminalProgramState::Blocked),
        (4, TerminalProgramState::Done),
        (5, TerminalProgramState::Error),
    ] {
        for reported_at in [1, u64::MAX] {
            let status = decoded(&[(7, 0, &payload(byte, reported_at))])
                .unwrap()
                .unwrap();
            assert_eq!(status.state, state);
            assert_eq!(status.reported_at_nanoseconds.get(), reported_at);
        }
    }
    let valid = payload(2, 9);
    for extensions in [
        vec![(7, 0, payload(0, 9))],
        vec![(7, 0, payload(6, 9))],
        vec![(7, 0, payload(2, 0))],
        vec![(7, 0, valid[..8].to_vec())],
        vec![(7, 0, [valid.as_slice(), &[0]].concat())],
        vec![(7, 0, valid.clone()), (7, 0, valid.clone())],
    ] {
        let extensions: Vec<_> = extensions
            .iter()
            .map(|(kind, flags, bytes)| (*kind, *flags, bytes.as_slice()))
            .collect();
        assert!(decoded(&extensions).is_err());
    }
}
