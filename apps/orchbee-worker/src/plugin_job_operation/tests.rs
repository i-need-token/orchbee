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
    assert_eq!(
        routed.environment,
        vec![
            ("ORCHBEE_PLUGIN_ID".into(), "tools".into()),
            ("ORCHBEE_RUNTIME_BASE".into(), "C:/owned-runtime".into()),
            ("ORCHBEE_RUNTIME_SCOPE".into(), "default".into()),
        ]
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
