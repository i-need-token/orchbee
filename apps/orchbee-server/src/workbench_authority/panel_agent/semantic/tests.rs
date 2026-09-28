use super::*;
use orchbee_workbench_protocol::PanelAgentState as State;

fn view(state: State) -> PanelAgentView {
    PanelAgentView {
        agent: Some("grok".into()),
        state,
        ..Default::default()
    }
}
#[test]
fn semantic_history_retains_transients_without_message_or_reference_noise() {
    let mut h = SemanticHistory::default();
    let initial = PanelAgentView::default();
    let working = view(State::Working);
    h.advance(&initial, &working).unwrap();
    let mut message = working.clone();
    message.message = Some("quiet metadata change".into());
    h.advance(&working, &message).unwrap();
    h.advance(&message, &view(State::Idle)).unwrap();
    assert_eq!(h.sequence().value(), 2);
    let changes = h.since(Some(WorkbenchRevision::new(0))).unwrap();
    assert_eq!(
        changes.iter().map(|c| c.state).collect::<Vec<_>>(),
        [State::Working, State::Idle]
    );
    assert!(h.since(None).unwrap().is_empty());
    assert!(h.since(Some(h.sequence())).unwrap().is_empty());
    assert_eq!(
        h.since(Some(WorkbenchRevision::new(3))),
        Err(Failure::InvalidRequest)
    );
}
#[test]
fn semantic_history_has_exact_eviction_floor_and_checked_overflow() {
    let mut h = SemanticHistory::default();
    let mut before = PanelAgentView::default();
    for n in 0..130 {
        let after = view(if n % 2 == 0 {
            State::Working
        } else {
            State::Idle
        });
        h.advance(&before, &after).unwrap();
        before = after;
    }
    assert_eq!(h.floor, 2);
    assert_eq!(
        h.since(Some(WorkbenchRevision::new(1))),
        Err(Failure::HistoryLimit)
    );
    assert_eq!(h.since(Some(WorkbenchRevision::new(2))).unwrap().len(), 128);
    h.sequence = u64::MAX;
    let bytes = h.bytes;
    assert_eq!(
        h.advance(&before, &view(State::Blocked)),
        Err(Failure::RevisionExhausted)
    );
    assert_eq!(h.bytes, bytes);
    h.advance(&before, &before).unwrap();
}
#[test]
fn semantic_history_byte_bound_evicts_whole_changes_and_rejects_oversized_candidate() {
    let mut h = SemanticHistory::default();
    let mut before = PanelAgentView::default();
    for n in 0..12 {
        let after = PanelAgentView {
            agent: Some(format!("{n}{}", "a".repeat(8192))),
            ..Default::default()
        };
        h.advance(&before, &after).unwrap();
        before = after;
        assert!(h.bytes <= MAXIMUM_HISTORY_BYTES);
    }
    assert!(h.floor > 0);
    let sequence = h.sequence;
    let bytes = h.bytes;
    let after = PanelAgentView {
        agent: Some("x".repeat(MAXIMUM_HISTORY_BYTES)),
        ..Default::default()
    };
    assert_eq!(h.advance(&before, &after), Err(Failure::ResourceLimit));
    assert_eq!((h.sequence, h.bytes), (sequence, bytes));
}

fn trace_failure(trace: &[u8]) -> Option<&'static str> {
    let mut history = SemanticHistory::default();
    let mut before = PanelAgentView::default();
    let mut expected = Vec::new();
    for event in trace {
        let after = PanelAgentView {
            agent: (event & 4 != 0).then(|| "grok".into()),
            state: [State::Unknown, State::Idle, State::Working, State::Blocked]
                [usize::from(event & 3)],
            message: Some(event.to_string()),
            ..Default::default()
        };
        if before.agent != after.agent || before.state != after.state {
            expected.push((after.agent.clone(), after.state));
        }
        if history.advance(&before, &after).is_err() {
            return Some("admitted transition refused");
        }
        if history.sequence != expected.len() as u64 {
            return Some("message churn or missing transition");
        }
        let floor = expected
            .len()
            .saturating_sub(MAXIMUM_TERMINAL_AGENT_CHANGES);
        let Ok(changes) = history.since(Some(WorkbenchRevision::new(floor as u64))) else {
            return Some("retained cursor refused");
        };
        if changes
            .iter()
            .map(|c| (c.agent.clone(), c.state))
            .collect::<Vec<_>>()
            != expected[floor..]
        {
            return Some("transient lost or reordered");
        }
        if floor > 0
            && history.since(Some(WorkbenchRevision::new(floor as u64 - 1)))
                != Err(Failure::HistoryLimit)
        {
            return Some("gap silently accepted");
        }
        before = after;
    }
    None
}
#[test]
fn generated_semantic_traces_preserve_current_state_and_replay_every_retained_transition() {
    for seed in 1_u64..=32 {
        let mut random = seed;
        let mut trace: Vec<u8> = (0..300)
            .map(|_| {
                random ^= random << 13;
                random ^= random >> 7;
                random ^= random << 17;
                (random & 255) as u8
            })
            .collect();
        if trace_failure(&trace).is_some() {
            let mut index = 0;
            while index < trace.len() {
                let mut reduced = trace.clone();
                reduced.remove(index);
                if trace_failure(&reduced).is_some() {
                    trace = reduced;
                } else {
                    index += 1;
                }
            }
            panic!(
                "seed {seed}, failure {:?}, replay {trace:?}",
                trace_failure(&trace)
            );
        }
    }
}
