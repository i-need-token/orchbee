use super::*;
use crate::pty_client::{COLUMNS, ROWS};

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
    let checkpoint = client.checkpoint();
    client.resize_rows(ROWS as u16);
    client.wait_resized_ready_after(
        checkpoint,
        Instant::now() + Duration::from_secs(40),
        ROWS as u16,
        1,
    );
    client.close_before(Instant::now() + EXIT_TIMEOUT);
    Ok(())
}

#[test]
fn initially_zero_cell_startup_recovers_on_resize_and_reaches_ready() -> TestResult {
    let _guard = process_witness_guard();
    let fixture = Fixture::new();
    for (index, (columns, rows)) in [(0_u16, 0_u16), (0, 24), (80, 0)].into_iter().enumerate() {
        let mut client = Client::spawn_sized(&fixture, "en_US.UTF-8", &fixture.base, columns, rows);
        client.wait_frame_text_after(
            0,
            Instant::now() + OBSERVATION_TIMEOUT,
            &["too small"],
            &[],
            &[],
            &mut [],
        );
        // Before the first successful Ready no Site exists yet, so waiting must
        // have started no services; later iterations legitimately see the site
        // the earlier recovery created.
        if index == 0 {
            assert!(fixture.processes("orchbee-server").is_empty());
            assert!(fixture.processes("orchbee-terminal-host").is_empty());
            assert!(fixture.processes("orchbee-worker").is_empty());
        }
        let checkpoint = client.checkpoint();
        client.resize(COLUMNS as u16, ROWS as u16);
        client.wait_resized_ready_after(
            checkpoint,
            Instant::now() + Duration::from_secs(40),
            ROWS as u16,
            1,
        );
        client.close_before(Instant::now() + EXIT_TIMEOUT);
    }
    Ok(())
}

#[test]
fn zero_cell_startup_cancel_exits_zero_without_any_service() -> TestResult {
    let _guard = process_witness_guard();
    let fixture = Fixture::new();
    let mut client = Client::spawn_sized(&fixture, "en_US.UTF-8", &fixture.base, 0, 0);
    client.wait_frame_text_after(
        0,
        Instant::now() + OBSERVATION_TIMEOUT,
        &["too small"],
        &[],
        &[],
        &mut [],
    );
    assert!(fixture.processes("orchbee-server").is_empty());
    assert!(fixture.processes("orchbee-terminal-host").is_empty());
    assert!(fixture.processes("orchbee-worker").is_empty());
    client.close_before(Instant::now() + EXIT_TIMEOUT);
    assert!(fixture.processes("orchbee-server").is_empty());
    assert!(fixture.processes("orchbee-terminal-host").is_empty());
    assert!(fixture.processes("orchbee-worker").is_empty());
    Ok(())
}

#[test]
fn unknown_pixel_startup_reaches_ready_on_the_real_last_row() -> TestResult {
    let _guard = process_witness_guard();
    let fixture = Fixture::new();
    let mut client = Client::spawn_geometry(
        &fixture,
        "en_US.UTF-8",
        &fixture.base,
        COLUMNS as u16,
        ROWS as u16,
        false,
    );
    client.wait_resized_ready_after(
        0,
        Instant::now() + STARTUP_OR_CREATE_TIMEOUT,
        ROWS as u16,
        1,
    );
    client.close_before(Instant::now() + EXIT_TIMEOUT);
    Ok(())
}
