use orchbee_capability_node_protocol::*;
use std::num::NonZeroU64;
fn launch(selection: Option<LaunchSelection>) -> AuthorizedTerminalLaunch {
    let n = NonZeroU64::MIN;
    AuthorizedTerminalLaunch::new(
        TerminalOperationAuthorization::new(
            TerminalId::from_bytes([1; 16]).unwrap(),
            OperationId::from_bytes([2; 16]).unwrap(),
            WorkerId::from_bytes([3; 16]).unwrap(),
            WorkerGeneration::new(n),
            TerminalFence::new(n),
            TerminalOperationLease::new(
                LeaseId::from_bytes([4; 16]).unwrap(),
                LeaseExpiresAtUnixMilliseconds::new(n),
            ),
            TERMINAL_LAUNCH_CAPABILITY_VERSION,
        ),
        None,
        TerminalLaunchMode::Deferred,
        selection,
    )
}
fn environment() -> PluginTerminalEnvironment {
    PluginTerminalEnvironment::new(
        PLUGIN_TERMINAL_ENVIRONMENT_NAMES[..7]
            .iter()
            .map(|name| ((*name).into(), "literal 中文".into()))
            .collect(),
    )
    .unwrap()
}
#[test]
fn plugin_terminal_kind_is_distinct_and_bounded() {
    let selection = LaunchSelection::new("C:/plugin.exe".into(), vec!["a&b".into()]).unwrap();
    let plain = launch(Some(selection.clone()));
    assert!(
        launch(None)
            .with_plugin_environment(Some(environment()))
            .is_err()
    );
    let empty = encode_message(&CapabilityNodeMessage::AuthorizedTerminalLaunch(launch(
        None,
    )))
    .unwrap();
    let message = CapabilityNodeMessage::AuthorizedTerminalLaunch(
        plain
            .with_environment(
                LaunchEnvironment::new(vec![("MARKER".into(), "value".into())]).unwrap(),
            )
            .with_plugin_environment(Some(environment()))
            .unwrap(),
    );
    let encoded = encode_message(&message).unwrap();
    let start = empty.len() - 3;
    assert_eq!(encoded[start], 3);
    assert_eq!(decode_message(&encoded).unwrap(), message);
    for end in [8, empty.len(), encoded.len() - 1] {
        assert!(decode_message(&encoded[..end]).is_err());
    }
    let mut bad = encoded.clone();
    bad[start] = 9;
    assert!(decode_message(&bad).is_err());
    let at = encoded
        .windows(b"ORCHBEE_PLUGIN_ID".len())
        .position(|bytes| bytes == b"ORCHBEE_PLUGIN_ID")
        .unwrap();
    let mut bad = encoded;
    bad[at] = b'X';
    assert!(decode_message(&bad).is_err());
    let huge = launch(Some(
        LaunchSelection::new("C:/plugin.exe".into(), vec!["x".repeat(30 * 1024 - 100)]).unwrap(),
    ));
    let mut entries = environment().entries().to_vec();
    entries[5].1 = "x".repeat(20 * 1024);
    let huge = huge
        .with_plugin_environment(Some(PluginTerminalEnvironment::new(entries).unwrap()))
        .unwrap();
    assert!(encode_message(&CapabilityNodeMessage::AuthorizedTerminalLaunch(huge)).is_err());
}
