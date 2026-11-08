use super::super::*;
use super::ACK;
use crate::local_client_service::tests::agent_harness;

#[test]
fn fragmented_native_envelope_is_acknowledged_without_half_close_and_listener_survives_bad_input() {
    // This scope derives only pipe names; it creates no filesystem resources.
    let scope =
        std::env::temp_dir().join(format!("orchbee-observation-native-{}", std::process::id()));
    let bound = bind(&scope).unwrap();
    let path = bound.path().to_owned();
    let endpoint = bound.start(agent_harness::seeded(&[]).unwrap());
    assert_eq!(endpoint.path(), path);
    let body = serde_json::json!({
        "adapterId":"codex", "correlations":[{"correlationId":"correlation-0000000000000001","visibility":"unknown"}], "eventId":"native-test-event-1", "version":1,
        "observation":{"activity":null,"agentKey":null,"event":"session-started","operationKey":null,"sessionKey":"session-1","turnKey":null,"version":1}
    }).to_string();
    let mut invalid = UnixStream::connect(&path).unwrap();
    invalid.write_all(b"invalid-json").unwrap();
    drop(invalid);
    let mut client = UnixStream::connect(&path).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let split = body.len() / 2;
    client.write_all(&body.as_bytes()[..split]).unwrap();
    thread::sleep(Duration::from_millis(10));
    client.write_all(&body.as_bytes()[split..]).unwrap();
    let mut reply = vec![0; ACK.len()];
    client.read_exact(&mut reply).unwrap();
    assert_eq!(reply, ACK);
    drop(client);
    let idle = UnixStream::connect(&path).unwrap();
    let began = Instant::now();
    drop(endpoint);
    assert!(
        began.elapsed() < Duration::from_secs(1),
        "shutdown joins despite an idle client"
    );
    drop(idle);
    assert!(!orchbee_local_ipc::endpoint_exists(&path).unwrap());
}
