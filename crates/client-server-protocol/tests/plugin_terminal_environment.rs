use orchbee_client_server_protocol::*;
use serde_json::json;
fn environment() -> PluginTerminalEnvironment {
    PluginTerminalEnvironment::new(
        PLUGIN_TERMINAL_ENVIRONMENT_NAMES[..7]
            .iter()
            .map(|name| ((*name).into(), "literal=中文 $ value".into()))
            .collect(),
    )
    .unwrap()
}
#[test]
fn plugin_terminal_environment_roundtrips_and_closed_schema_requires_an_explicit_program() {
    let base = TerminalCreateRequest::new(
        RequestId::parse(&"1".repeat(32)).unwrap(),
        None,
        None,
        StartMode::Deferred,
        Some(LaunchSelection::new("C:/plugin.exe".into(), vec!["literal & arg".into()]).unwrap()),
    );
    let legacy = encode_client_frame(&ClientMessage::TerminalCreate(base.clone())).unwrap();
    assert_eq!(
        legacy,
        encode_client_frame(&ClientMessage::TerminalCreate(
            base.clone().with_plugin_environment(None).unwrap()
        ))
        .unwrap()
    );
    let request = base.with_plugin_environment(Some(environment())).unwrap();
    let message = ClientMessage::TerminalCreate(request);
    let encoded = encode_client_frame(&message).unwrap();
    assert_eq!(decode_client_frame(&encoded).unwrap(), message);
    let document: serde_json::Value = serde_json::from_slice(&encoded[8..]).unwrap();
    let schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../../schemas/client-server/v2/client-server.schema.json"
    ))
    .unwrap();
    let validator = jsonschema::draft202012::new(&schema).unwrap();
    assert!(validator.is_valid(&document), "{document}");
    for field in ["launch", "pluginEnvironment"] {
        let mut bad = document.clone();
        bad[field] = json!(null);
        assert!(!validator.is_valid(&bad));
        let payload = serde_json::to_vec(&bad).unwrap();
        let mut frame = encoded[..8].to_vec();
        frame[6..8].copy_from_slice(&(payload.len() as u16).to_be_bytes());
        frame.extend(payload);
        assert!(decode_client_frame(&frame).is_err());
    }
    let no_program = TerminalCreateRequest::new(
        RequestId::parse(&"2".repeat(32)).unwrap(),
        None,
        None,
        StartMode::Deferred,
        None,
    );
    assert!(
        no_program
            .with_plugin_environment(Some(environment()))
            .is_err()
    );
}
