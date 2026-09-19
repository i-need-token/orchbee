use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Action {
    Create,
    Rename(u8, u8),
    Move(u8, u8),
}

#[derive(Debug)]
struct Failure {
    step: usize,
    claim: &'static str,
}

#[test]
fn generated_reorder_histories_refine_identity_keyed_membership_and_provenance() {
    let replay_input = std::env::var("ORCHBEE_NATIVE_REORDER_REPLAY").ok();
    for phase in 0..3 {
        for seed in [0x41c6_4e6d_u64, 0x9e37_79b9, 0xd1b5_4a32, 0x94d0_49bb] {
            let actions = replay_input
                .as_ref()
                .map_or_else(|| generate(seed), |input| decode(input));
            assert_eq!(decode(&encode(&actions)), actions);
            match replay(phase, &actions) {
                Ok(coverage) => {
                    if replay_input.is_none() {
                        assert!(
                            coverage.iter().all(|count| *count > 0),
                            "unreached class: {coverage:?}"
                        );
                    }
                    eprintln!(
                        "native-reorder seed={seed:#x} phase={phase} steps={} coverage={coverage:?}",
                        actions.len()
                    );
                }
                Err(failure) => {
                    let reduced = shrink(phase, actions, failure.claim);
                    panic!(
                        "seed={seed:#x} phase={phase} step={} claim={} replay={}",
                        failure.step,
                        failure.claim,
                        encode(&reduced)
                    );
                }
            }
        }
    }
}

fn generate(mut random: u64) -> Vec<Action> {
    let mut actions = vec![
        Action::Create,
        Action::Rename(0, 7),
        Action::Move(0, 6),
        Action::Move(6, 0),
        Action::Move(0, 0),
    ];
    for _ in 0..96 {
        random = random
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let first = (random >> 32) as u8;
        let second = (random >> 48) as u8;
        actions.push(match random % 3 {
            0 => Action::Create,
            1 => Action::Rename(first, second),
            _ => Action::Move(first, second),
        });
    }
    actions
}

fn replay(phase: u8, actions: &[Action]) -> Result<[usize; 7], Failure> {
    let mut canonical = snapshot(6, 1, None);
    let mut state = ready(6);
    let mut history = [
        CreateTerminalProvenance::Started,
        CreateTerminalProvenance::Pending,
        CreateTerminalProvenance::Unavailable(
            WorkbenchTerminalUnavailableReason::TerminalRuntimeUnavailable,
        ),
        CreateTerminalProvenance::Failed(WorkbenchTerminalFailureReason::TerminalCreateFailed),
        CreateTerminalProvenance::OutcomeUnknown,
        CreateTerminalProvenance::BindingOnly,
    ];
    if phase == 1 {
        history.swap(0, 1);
    }
    state.provenance = history.to_vec();
    let known: Vec<_> = canonical.terminal_contexts[0]
        .tabs
        .iter()
        .zip(history)
        .map(|(tab, provenance)| (tab.tab_id.clone(), provenance))
        .collect();
    if phase > 0 {
        let selected = state.active().unwrap().clone();
        state.phase = if phase == 1 {
            SessionPhase::SelectedWithoutAttachment(selected)
        } else {
            SessionPhase::RetryableAttachUnavailable(selected)
        };
        if let Some(record) = state.focus_record_mut() {
            record.attached = false;
            record.projected = false;
            record.staged_frame = None;
            record.accepted_frame = None;
            record.mode = None;
            record.last_accepted_geometry = None;
        }
    }
    let active_id = canonical.terminal_contexts[0].tabs[0].tab_id.clone();
    let desired = state.desired().unwrap().clone();
    let attached = state.attached_key().cloned();
    let projection = state.projection_key().cloned();
    let mode = state.controller_mode().cloned();
    let frame = state.focus_accepted_frame().clone();
    let stable_phase = state.phase_kind();
    let mut coverage = [0_usize; 7];
    for (step, action) in actions.iter().enumerate() {
        let fail = |claim| Failure { step, claim };
        let old_active = state.active().unwrap().index();
        let tabs = &mut canonical.terminal_contexts[0].tabs;
        let mut changed = true;
        match *action {
            Action::Create => {
                if tabs.len() == orchbee_workbench_protocol::MAXIMUM_TABS_PER_CONTEXT {
                    changed = false;
                } else {
                    tabs.push(tab(tabs.len()));
                    coverage[0] += 1;
                }
            }
            Action::Rename(index, value) => {
                let index = usize::from(index) % tabs.len();
                tabs[index].label = format!("rename-{step}-{value}");
                coverage[1] += 1;
            }
            Action::Move(from, to) => {
                let from = usize::from(from) % tabs.len();
                let to = usize::from(to) % tabs.len();
                changed = from != to;
                let moved = tabs.remove(from);
                tabs.insert(to, moved);
                coverage[2] += 1;
                coverage[match from.cmp(&to) {
                    std::cmp::Ordering::Greater => 3,
                    std::cmp::Ordering::Less => 4,
                    std::cmp::Ordering::Equal => 5,
                }] += 1;
            }
        }
        if changed {
            canonical.revision = WorkbenchRevision::new(canonical.revision.value() + 1);
        }
        let generation = u64::try_from(step).unwrap() + 1;
        if !state.start_pending_refresh(generation) {
            return Err(fail("refresh admission"));
        }
        if finish_refresh(
            &mut state,
            generation,
            WorkbenchSnapshotOutcome::Available {
                snapshot: canonical.clone(),
            },
            None,
        ) != RefreshDecision::Continue
        {
            return Err(fail("strict projection rejected supported history"));
        }
        let expected_index = canonical.terminal_contexts[0]
            .tabs
            .iter()
            .position(|tab| tab.tab_id == active_id)
            .unwrap();
        coverage[6] += usize::from(old_active != expected_index);
        if state.active().unwrap().index() != expected_index
            || state.desired().unwrap() != &desired
            || state.attached_key() != attached.as_ref()
            || state.projection_key() != projection.as_ref()
            || state.controller_mode() != mode.as_ref()
            || state.focus_accepted_frame() != frame
            || state.phase_kind() != stable_phase
            || !state.invariant_holds()
        {
            return Err(fail(
                "active attachment or historical Ready identity changed",
            ));
        }
        if state.cache().revision() != canonical.revision
            || state.cache().tabs().len() != canonical.terminal_contexts[0].tabs.len()
        {
            return Err(fail("canonical revision or membership length mismatch"));
        }
        for (index, expected) in canonical.terminal_contexts[0].tabs.iter().enumerate() {
            let expected_provenance = known
                .iter()
                .find(|(id, _)| *id == expected.tab_id)
                .map_or(&CreateTerminalProvenance::BindingOnly, |(_, provenance)| {
                    provenance
                });
            if state.tab(index).unwrap().tab_id() != &expected.tab_id
                || state.tab(index).unwrap().tab_label().as_str() != expected.label
                || state.provenance(index) != Some(expected_provenance)
            {
                return Err(fail("ID-keyed provenance or canonical order mismatch"));
            }
        }
    }
    Ok(coverage)
}

fn shrink(phase: u8, mut actions: Vec<Action>, claim: &str) -> Vec<Action> {
    let fails = |candidate: &[Action]| {
        replay(phase, candidate).is_err_and(|failure| failure.claim == claim)
    };
    let mut width = actions.len();
    while width > 0 {
        let mut start = 0;
        while start + width <= actions.len() {
            let mut candidate = actions.clone();
            candidate.drain(start..start + width);
            if fails(&candidate) {
                actions = candidate;
            } else {
                start += 1;
            }
        }
        width /= 2;
    }
    for index in 0..actions.len() {
        let simplified = match actions[index] {
            Action::Create => Action::Create,
            Action::Rename(_, _) => Action::Rename(0, 0),
            Action::Move(_, _) => Action::Move(0, 1),
        };
        let mut candidate = actions.clone();
        candidate[index] = simplified;
        if fails(&candidate) {
            actions = candidate;
        }
    }
    actions
}

fn encode(actions: &[Action]) -> String {
    actions
        .iter()
        .map(|action| match action {
            Action::Create => "c".to_owned(),
            Action::Rename(index, value) => format!("r:{index}:{value}"),
            Action::Move(from, to) => format!("m:{from}:{to}"),
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn decode(input: &str) -> Vec<Action> {
    if input.is_empty() {
        return Vec::new();
    }
    input
        .split(',')
        .map(|token| {
            let fields: Vec<_> = token.split(':').collect();
            match fields.as_slice() {
                ["c"] => Action::Create,
                ["r", index, value] => {
                    Action::Rename(index.parse().unwrap(), value.parse().unwrap())
                }
                ["m", from, to] => Action::Move(from.parse().unwrap(), to.parse().unwrap()),
                _ => panic!("invalid reorder replay action: {token}"),
            }
        })
        .collect()
}
