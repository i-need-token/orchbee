use super::completed;
use orchbee_terminal_protocol::{
    TerminalCommandObservation, TerminalObservedCommand, decode_terminal_text_frame,
};
use std::num::NonZeroU64;

#[test]
fn only_the_exact_completed_invocation_can_finish_a_wait() {
    let mut frame = decode_terminal_text_frame(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/terminal/v1/fixtures/terminal-frame.bin"
    )))
    .unwrap();
    let id = "11".repeat(16);
    assert!(completed(&frame, &id).is_none());
    let mut command = TerminalObservedCommand {
        id: [0x11; 16],
        completed: false,
        exit_code: 0,
        start_row: 0,
        start_column: 0,
        end_row: 0,
        end_column: 0,
        generation: NonZeroU64::new(1).unwrap(),
        output_epoch: NonZeroU64::new(1).unwrap(),
        started_at: None,
    };
    frame.command_observation = Some(TerminalCommandObservation {
        output_epoch: NonZeroU64::new(1).unwrap(),
        ready: false,
        command: Some(command),
    });
    assert!(completed(&frame, &id).is_none());
    command.completed = true;
    command.exit_code = 1;
    frame.command_observation.as_mut().unwrap().command = Some(command);
    assert!(completed(&frame, &"22".repeat(16)).is_none());
    assert_eq!(completed(&frame, &id).unwrap().exit_code, 1);
}
