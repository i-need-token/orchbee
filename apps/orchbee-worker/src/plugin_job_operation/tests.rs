use super::*;
use orchbee_capability_node_protocol::{PluginCommandIntent, PluginJobAction};
use std::path::Path;

#[test]
fn plugin_route_uses_owned_session_and_rejects_invalid_paths_before_execution() {
    let intent = PluginCommandIntent {
        plugin_id: "tools".into(),
        command_id: "hello".into(),
        directory: "C:/plugins/tools".into(),
        argv: vec!["cmd.exe".into()],
        environment: vec![
            ("ORCHBEE_RUNTIME_BASE".into(), "C:/untrusted".into()),
            ("ORCHBEE_RUNTIME_SCOPE".into(), "other".into()),
            ("ORCHBEE_PLUGIN_ID".into(), "tools".into()),
        ],
    };
    let PluginJobAction::Start(routed) = route(
        PluginJobAction::Start(intent.clone()),
        Some(Path::new("C:/owned-runtime/default")),
    )
    .unwrap() else {
        panic!()
    };
    // Inherited routing is replaced by the owned Session and tagged with this build's environment.
    let value = |name: &str| {
        let values: Vec<_> = routed
            .environment
            .iter()
            .filter(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
            .collect();
        assert!(values.len() <= 1, "{name} is routed once");
        values.first().copied()
    };
    assert_eq!(value("ORCHBEE_PLUGIN_ID"), Some("tools"));
    assert_eq!(value("ORCHBEE_RUNTIME_BASE"), Some("C:/owned-runtime"));
    assert_eq!(value("ORCHBEE_RUNTIME_SCOPE"), Some("default"));
    assert_eq!(
        value(orchbee_local_runtime::ENVIRONMENT_VARIABLE),
        Some(orchbee_local_runtime::environment_stem())
    );
    assert!(
        route(
            PluginJobAction::Start(intent.clone()),
            Some(Path::new("relative/default"))
        )
        .is_err()
    );
    let mut relative = intent;
    relative.directory = "relative".into();
    assert!(route(PluginJobAction::Start(relative), None).is_err());
}
