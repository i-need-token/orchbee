use super::*;

/// The Server's row meets only the screen it was computed on: one Host terminal generation at one
/// content revision. A later revision, another generation, terminal or Host, or a row beyond the
/// screen gets no row, so a read never cuts a screen the row was not computed on.
#[test]
fn the_row_meets_only_the_sampled_screen() {
    let frame = orchbee_terminal_protocol::decode_terminal_text_frame(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/terminal/v1/fixtures/terminal-frame.bin"
    )))
    .unwrap();
    let host = HostTerminalId::parse(&"6".repeat(32)).unwrap();
    let other_host = HostTerminalId::parse(&"7".repeat(32)).unwrap();
    let binding = |epoch: String| -> AgentLaunchBinding {
        serde_json::from_value(serde_json::json!({
            "presentationId": "3".repeat(32),
            "generation": "4".repeat(32),
            "hostEpoch": epoch,
            "hostTerminalId": host.to_text(),
        }))
        .unwrap()
    };
    let area = |epoch: String, generation: u64, revision: u64, row: u16| InputArea {
        binding: binding(epoch),
        generation,
        revision,
        row,
    };
    let epoch = frame.route.host_epoch.to_text();
    let generation = frame.route.generation.get();
    let revision = frame.route.revision;
    let last = frame.geometry.rows - 1;
    assert_eq!(
        area(epoch.clone(), generation, revision, last).row_for(&host, &frame),
        Some(last)
    );
    for (candidate, terminal) in [
        (area(epoch.clone(), generation, revision + 1, 0), host),
        (area(epoch.clone(), generation + 1, revision, 0), host),
        (area("8".repeat(32), generation, revision, 0), host),
        (area(epoch.clone(), generation, revision, 0), other_host),
        (area(epoch.clone(), generation, revision, last + 1), host),
    ] {
        assert_eq!(candidate.row_for(&terminal, &frame), None);
    }
}
