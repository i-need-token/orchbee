use super::*;
use crate::cli::update::manifest::Connections;
use orchbee_connection_handshake::{
    Build, CapabilityName, Extensions, ProcessState, ProductVersion,
};

fn component(id: &str) -> Component {
    Component {
        id: id.to_owned(),
        version: "2.0.0".to_owned(),
        security_floor: None,
        recommended_floor: None,
        connections: Connections::default(),
    }
}

fn query(stopped: bool) -> QueryResult {
    let entry = |role, version: &str, pid, terminals| ProcessEntry {
        role,
        product_version: ProductVersion::new(version).unwrap(),
        build: Build::default(),
        environment: orchbee_local_runtime::environment_name(),
        pid: NonZeroU32::new(pid).unwrap(),
        state: ProcessState::Ready,
        terminals,
        connections: Vec::new(),
    };
    let mut extensions = Extensions::new();
    if stopped {
        extensions
            .insert(
                CapabilityName::new("moves-stopped").unwrap(),
                json!({"step": "restore", "reason": "other"}),
            )
            .unwrap();
    }
    QueryResult::new(
        vec![
            entry(ProcessRole::Server, "2.0.0", 7, 0),
            entry(ProcessRole::Worker, "2.0.0", 8, 0),
            entry(ProcessRole::TerminalHost, "1.9.0", 10, 24),
            entry(ProcessRole::TerminalHost, "1.8.0", 11, 0),
            entry(ProcessRole::TerminalHost, "2.0.0", 12, 3),
        ],
        Some(true),
        extensions,
    )
    .unwrap()
}

/// A Host older than the Worker that still runs terminals after a failed move stopped the Worker's
/// moves recommends the restart whose Worker moves them again; without the stop, or without
/// terminals, it asks for nothing.
#[test]
fn terminals_left_behind_by_stopped_moves_recommend_a_restart() {
    let components = ["client", "server", "worker", "terminal-host"].map(component);
    let tiers = |restart: &Restart| {
        restart
            .hosts
            .iter()
            .map(|host| (host.pid.get(), host.tier))
            .collect::<Vec<_>>()
    };
    let stopped = evaluate(&components, &query(true));
    assert_eq!(stopped.tier, Tier::Recommended);
    assert_eq!(
        tiers(&stopped),
        [(10, Tier::Recommended), (11, Tier::None), (12, Tier::None)]
    );
    let moving = evaluate(&components, &query(false));
    assert_eq!(moving.tier, Tier::None);
    assert_eq!(
        tiers(&moving),
        [(10, Tier::None), (11, Tier::None), (12, Tier::None)]
    );
}
