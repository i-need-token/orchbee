use super::super::*;

/// Windows withdrew the Agent observation channel: a Session start binds no pipe at the observation
/// name, so nothing of this Server's sits where an Agent's hook would look.
#[test]
fn a_windows_session_binds_no_observation_pipe() {
    // This scope derives only pipe names; it creates no filesystem resources.
    let scope =
        std::env::temp_dir().join(format!("orchbee-observation-native-{}", std::process::id()));
    let path =
        orchbee_local_runtime::ModeCheckedLocalSessionRendezvous::observation_endpoint_path(&scope);
    let bound = bind_for_session(&scope).unwrap();
    assert!(bound.is_none());
    assert!(!orchbee_local_ipc::endpoint_exists(&path).unwrap());
}
