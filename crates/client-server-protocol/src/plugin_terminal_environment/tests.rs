use super::*;
fn entries() -> Vec<(String, String)> {
    PLUGIN_TERMINAL_ENVIRONMENT_NAMES[..7]
        .iter()
        .map(|name| ((*name).into(), "private-value".into()))
        .collect()
}
#[test]
fn plugin_display_environment_refuses_runtime_authority_and_partial_or_unbounded_metadata() {
    let accepted = PluginTerminalEnvironment::new(entries()).unwrap();
    assert!(!format!("{accepted:?}").contains("private-value"));
    for name in [
        "ORCHBEE_RUNTIME_BASE",
        "ORCHBEE_RUNTIME_SCOPE",
        "ORCHBEE_WORKER_ID",
        "ORCHBEE_TERMINAL_ID",
        "PATH",
        "orchbee_plugin_id",
    ] {
        let mut bad = entries();
        bad[0].0 = name.into();
        assert!(PluginTerminalEnvironment::new(bad).is_err(), "{name}");
    }
    for value in ["", "nul\0value"] {
        let mut bad = entries();
        bad[0].1 = value.into();
        assert!(PluginTerminalEnvironment::new(bad).is_err());
    }
    for index in 0..7 {
        let mut bad = entries();
        bad.remove(index);
        assert!(PluginTerminalEnvironment::new(bad).is_err());
    }
    let mut bad = entries();
    bad.push(bad[0].clone());
    assert!(PluginTerminalEnvironment::new(bad).is_err());
    let mut boundary = entries();
    boundary[5].1.clear();
    let used: usize = boundary
        .iter()
        .map(|(name, value)| name.len() + value.len())
        .sum();
    boundary[5].1 = "x".repeat(MAXIMUM_PLUGIN_TERMINAL_ENVIRONMENT_BYTES - used);
    assert!(PluginTerminalEnvironment::new(boundary.clone()).is_ok());
    boundary[5].1.push('x');
    assert!(PluginTerminalEnvironment::new(boundary).is_err());
    assert!(
        crate::LaunchEnvironment::new(entries()).is_err(),
        "caller overlay stays restricted"
    );
}
