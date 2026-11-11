use super::windows::{DEADLINE, probe, resolve_commands};
use orchbee_capability_node_protocol::AgentProbeResult::{Found, Missing, Unavailable};
use std::{cell::Cell, ffi::OsStr, sync::atomic::AtomicBool, time::Instant};

#[test]
fn probe_keeps_partial_results_and_stops_without_guessing() {
    let commands = ["codex", "pi", "amp", "mcode", "gemini"].map(String::from);
    let mut observed = Vec::new();
    let mut results = vec![Unavailable; commands.len()];
    resolve_commands(
        &commands,
        &mut results,
        || false,
        |command| {
            observed.push(command.to_owned());
            match command {
                "codex" | "amp" | "mcode" => Found,
                "pi" => Unavailable,
                _ => Missing,
            }
        },
    );
    // A failed query neither discards an earlier answer nor skips a later command.
    assert_eq!(observed, commands);
    assert_eq!(results, [Found, Unavailable, Found, Found, Missing]);
    let probes = Cell::new(0);
    let mut results = vec![Unavailable; commands.len()];
    resolve_commands(
        &commands,
        &mut results,
        || probes.get() == 2,
        |_| {
            probes.set(probes.get() + 1);
            Missing
        },
    );
    assert_eq!(probes.get(), 2);
    assert_eq!(
        results,
        [Missing, Missing, Unavailable, Unavailable, Unavailable]
    );
    // A cancelled or expired probe queries nothing and answers nothing.
    for (cancelled, deadline) in [(true, Instant::now() + DEADLINE), (false, Instant::now())] {
        let mut results = vec![Unavailable; commands.len()];
        probe(
            &commands,
            &mut results,
            OsStr::new(""),
            &AtomicBool::new(cancelled),
            deadline,
        );
        assert_eq!(results, [Unavailable; 5]);
    }
}
