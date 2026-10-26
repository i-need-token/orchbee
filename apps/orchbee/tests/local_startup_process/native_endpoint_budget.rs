//! Endpoint-budget witnesses: every derived socket path — client, worker,
//! pending, and the Host endpoint — is checked before activation markers,
//! Host directories, or managed processes can appear.

use super::native_cohort::runtime_for_base;
use super::*;
use orchbee_local_client_protocol::{UNIX_SOCKET_PATH_CAPACITY, unix_socket_path_fits};
use orchbee_local_runtime::ModeCheckedLocalSessionRendezvous;

#[test]
fn overlong_host_endpoint_rejects_before_activation_or_host_side_effects() -> TestResult {
    let _guard = process_witness_guard();
    let fixture = Fixture::new();
    // Pad the runtime base until only the deepest derived address — the Host
    // endpoint — cannot fit one socket address; the sibling endpoints still fit.
    let host_endpoint = ModeCheckedLocalSessionRendezvous::host_endpoint_path(&fixture.runtime());
    let pad = UNIX_SOCKET_PATH_CAPACITY
        .checked_sub(host_endpoint.as_os_str().as_encoded_bytes().len())
        .expect("fixture Host endpoint must start inside the socket budget");
    let padded_base = fixture.base.join("x".repeat(pad));
    fs::create_dir(&padded_base)?;
    fs::set_permissions(&padded_base, fs::Permissions::from_mode(0o700))?;
    let padded_runtime = runtime_for_base(&padded_base);
    // The window is exact: the Host endpoint overflows by one byte while every
    // sibling still fits, so this rejection belongs to the Host budget alone.
    let [client_path, worker_path, pending_path, host_path] =
        ModeCheckedLocalSessionRendezvous::socket_budget_paths(&padded_runtime);
    for sibling in [&client_path, &worker_path, &pending_path] {
        assert!(unix_socket_path_fits(sibling), "{sibling:?} must still fit");
    }
    assert!(!unix_socket_path_fits(&host_path));

    let mut client = Client::spawn_at(&fixture, "en_US.UTF-8", &padded_base);
    let error = client.exit_message_before(Instant::now() + STARTUP_OR_CREATE_TIMEOUT);
    assert!(
        error.contains("socket path is too long"),
        "unexpected startup diagnostic: {error:?}"
    );
    // The over-budget verdict precedes every activation and Host side effect;
    // the already-prepared Session directory itself is allowed to remain.
    assert!(!padded_runtime.join("activation.started").exists());
    assert!(!ModeCheckedLocalSessionRendezvous::host_runtime_directory(&padded_runtime).exists());
    assert!(!worker_path.exists());
    assert!(!client_path.exists());
    assert!(fixture.processes("orchbee-server").is_empty());
    assert!(fixture.processes("orchbee-worker").is_empty());
    assert!(fixture.processes("orchbee-terminal-host").is_empty());
    Ok(())
}

#[test]
fn overlong_pending_endpoint_rejects_before_activation_or_service() -> TestResult {
    let _guard = process_witness_guard();
    let fixture = Fixture::new();
    // The pending name sits between the client/worker and Host lengths, so an
    // over-budget pending always implies an over-budget Host endpoint; padding
    // to the exact pending boundary covers both budgets crossing one byte over.
    let pending = ModeCheckedLocalSessionRendezvous::client_pending_socket_path(&fixture.runtime());
    let pad = UNIX_SOCKET_PATH_CAPACITY
        .checked_sub(pending.as_os_str().as_encoded_bytes().len())
        .expect("fixture pending path must start inside the socket budget");
    let padded_base = fixture.base.join("x".repeat(pad));
    fs::create_dir(&padded_base)?;
    fs::set_permissions(&padded_base, fs::Permissions::from_mode(0o700))?;
    let padded_runtime = runtime_for_base(&padded_base);
    let [client_path, worker_path, pending_path, host_path] =
        ModeCheckedLocalSessionRendezvous::socket_budget_paths(&padded_runtime);
    assert!(unix_socket_path_fits(&client_path));
    assert!(unix_socket_path_fits(&worker_path));
    assert!(!unix_socket_path_fits(&pending_path));
    assert!(!unix_socket_path_fits(&host_path));

    let mut client = Client::spawn_at(&fixture, "en_US.UTF-8", &padded_base);
    let error = client.exit_message_before(Instant::now() + STARTUP_OR_CREATE_TIMEOUT);
    assert!(
        error.contains("socket path is too long"),
        "unexpected startup diagnostic: {error:?}"
    );
    assert!(!padded_runtime.join("activation.started").exists());
    assert!(!ModeCheckedLocalSessionRendezvous::host_runtime_directory(&padded_runtime).exists());
    assert!(fixture.processes("orchbee-server").is_empty());
    assert!(fixture.processes("orchbee-worker").is_empty());
    Ok(())
}
