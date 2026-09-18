use super::*;
use crate::pty_client::ROWS;

const TOO_SMALL_ROWS: u16 = 4;

#[test]
fn too_small_startup_waits_then_exits_zero_without_any_service() -> TestResult {
    let _guard = process_witness_guard();
    let fixture = Fixture::new();
    let mut client =
        Client::spawn_sized(&fixture, "en_US.UTF-8", &fixture.base, 120, TOO_SMALL_ROWS);
    client.wait_frame_text_after(
        0,
        Instant::now() + OBSERVATION_TIMEOUT,
        &["too small"],
        &[],
        &[],
        &mut [],
    );
    client.close_before(Instant::now() + EXIT_TIMEOUT);
    assert!(fixture.processes("orchbee-server").is_empty());
    assert!(fixture.processes("orchbee-terminal-host").is_empty());
    assert!(fixture.processes("orchbee-worker").is_empty());
    Ok(())
}

#[test]
fn too_small_startup_recovers_on_resize_and_reaches_ready() -> TestResult {
    let _guard = process_witness_guard();
    let fixture = Fixture::new();
    let mut client =
        Client::spawn_sized(&fixture, "en_US.UTF-8", &fixture.base, 120, TOO_SMALL_ROWS);
    client.wait_frame_text_after(
        0,
        Instant::now() + OBSERVATION_TIMEOUT,
        &["too small"],
        &[],
        &[],
        &mut [],
    );
    client.resize_rows(ROWS as u16);
    client.wait_frame_text_after(
        0,
        Instant::now() + Duration::from_secs(40),
        &["[*1 1]", "Tab 1 · Ready"],
        &[],
        &[],
        &mut [],
    );
    client.close_before(Instant::now() + EXIT_TIMEOUT);
    Ok(())
}
