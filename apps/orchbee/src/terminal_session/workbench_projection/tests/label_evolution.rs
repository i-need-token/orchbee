use super::*;

const TRACE_SEEDS: u64 = 64;
const STEPS_PER_TRACE: usize = 38;
const ACTION_COUNT: usize = 19;

#[derive(Clone, Copy, Debug)]
enum Action {
    ExactSameRevision,
    RevisionOnly,
    RenameOne,
    RenameMany,
    Append,
    RenameAndAppend,
    SameRevisionRename,
    OlderRevision,
    ChangeSite,
    ChangeContext,
    RemoveExisting,
    ReorderExisting,
    InsertBeforeExisting,
    ChangeTabId,
    ChangePanelId,
    ChangePresentationId,
    ChangePresentationLabel,
    ChangeWorkerId,
    ChangeTerminalId,
}

impl Action {
    const ALL: [Self; ACTION_COUNT] = [
        Self::ExactSameRevision,
        Self::RevisionOnly,
        Self::RenameOne,
        Self::RenameMany,
        Self::Append,
        Self::RenameAndAppend,
        Self::SameRevisionRename,
        Self::OlderRevision,
        Self::ChangeSite,
        Self::ChangeContext,
        Self::RemoveExisting,
        Self::ReorderExisting,
        Self::InsertBeforeExisting,
        Self::ChangeTabId,
        Self::ChangePanelId,
        Self::ChangePresentationId,
        Self::ChangePresentationLabel,
        Self::ChangeWorkerId,
        Self::ChangeTerminalId,
    ];

    const fn must_be_accepted(self) -> bool {
        matches!(
            self,
            Self::ExactSameRevision
                | Self::RevisionOnly
                | Self::RenameOne
                | Self::RenameMany
                | Self::Append
                | Self::RenameAndAppend
                | Self::ReorderExisting
                | Self::InsertBeforeExisting
                | Self::RemoveExisting
        )
    }
}

#[test]
fn generated_label_evolution_traces_match_the_independent_refinement() {
    let mut coverage = [0_usize; ACTION_COUNT];
    let mut accepted = 0_usize;
    let mut rejected = 0_usize;

    for seed in 0..TRACE_SEEDS {
        let mut random = Random::new(seed);
        let mut current = projected(2 + random.index(5), 100 + seed * 100);

        for step in 0..STEPS_PER_TRACE {
            let action_index = (usize::try_from(seed).unwrap() + step) % ACTION_COUNT;
            let action = Action::ALL[action_index];
            coverage[action_index] += 1;
            let candidate = apply(action, &current, &mut random);
            let expected = reference_accepts(&current, &candidate);
            assert_eq!(
                expected,
                action.must_be_accepted(),
                "generated action failed to realize its class: seed={seed}; step={step}; action={action:?}"
            );
            let result = validate_canonical_refresh(&current, candidate.clone());

            assert_eq!(
                result.is_ok(),
                expected,
                "seed={seed}; step={step}; action={action:?}; current={current:?}; candidate={candidate:?}"
            );
            if expected {
                accepted += 1;
                current = result.unwrap();
            } else {
                rejected += 1;
            }
        }
    }

    assert!(coverage.into_iter().all(|count| count > 0));
    assert!(accepted > 0);
    assert!(rejected > 0);
}

fn apply(
    action: Action,
    current: &NativeStandaloneTabs,
    random: &mut Random,
) -> NativeStandaloneTabs {
    if matches!(action, Action::ExactSameRevision) {
        return current.clone();
    }

    let mut candidate = successor(current, 0);
    let first = random.index(current.tabs.len());
    match action {
        Action::ExactSameRevision | Action::RevisionOnly => {}
        Action::RenameOne => rename(&mut candidate, first, random.next()),
        Action::RenameMany => {
            rename(&mut candidate, first, random.next());
            rename(
                &mut candidate,
                (first + 1) % current.tabs.len(),
                random.next(),
            );
        }
        Action::Append => candidate = successor(current, 1 + random.index(3)),
        Action::RenameAndAppend => {
            candidate = successor(current, 1 + random.index(3));
            rename(&mut candidate, first, random.next());
        }
        Action::SameRevisionRename => {
            candidate.revision = current.revision;
            rename(&mut candidate, first, random.next());
        }
        Action::OlderRevision => {
            candidate.revision = WorkbenchRevision::new(current.revision.value() - 1);
        }
        Action::ChangeSite => candidate.site_id = site_id(100_000 + u128::from(random.next())),
        Action::ChangeContext => {
            candidate.terminal_context_id = context_id(200_000 + u128::from(random.next()));
        }
        Action::RemoveExisting => candidate.tabs.truncate(current.tabs.len() - 1),
        Action::ReorderExisting => candidate.tabs.rotate_left(1),
        Action::InsertBeforeExisting => {
            let mut source = successor(current, 1);
            let inserted = source.tabs.pop().unwrap();
            candidate.tabs.insert(first, inserted);
        }
        Action::ChangeTabId => {
            candidate.tabs[first].tab_id = tab_id(300_000 + u128::from(random.next()));
        }
        Action::ChangePanelId => {
            candidate.tabs[first].panel_id = panel_id(400_000 + u128::from(random.next()));
        }
        Action::ChangePresentationId => {
            candidate.tabs[first].presentation_id =
                presentation_id(500_000 + u128::from(random.next()));
        }
        Action::ChangePresentationLabel => {
            candidate.tabs[first].presentation_label = generated_label(random.next());
        }
        Action::ChangeWorkerId => {
            candidate.tabs[first].binding.worker_id =
                worker_id(600_000 + u128::from(random.next()));
        }
        Action::ChangeTerminalId => {
            candidate.tabs[first].binding.terminal_id =
                terminal_id(700_000 + u128::from(random.next()));
        }
    }
    candidate
}

fn successor(current: &NativeStandaloneTabs, appended: usize) -> NativeStandaloneTabs {
    let revision = current.revision.value() + 1;
    let mut candidate = current.clone();
    candidate.revision = WorkbenchRevision::new(revision);
    if appended > 0 {
        for offset in 0..appended {
            candidate
                .tabs
                .push(project_tab(&tab(usize::try_from(revision).unwrap() * 4 + offset)).unwrap());
        }
    }
    candidate
}

fn rename(candidate: &mut NativeStandaloneTabs, index: usize, value: u64) {
    candidate.tabs[index].tab_label = generated_label(value);
}

fn generated_label(value: u64) -> WorkbenchLabel {
    label(&format!("  rename-{value}-雪  "))
}

fn reference_accepts(current: &NativeStandaloneTabs, candidate: &NativeStandaloneTabs) -> bool {
    if candidate.site_id != current.site_id
        || candidate.terminal_context_id != current.terminal_context_id
        || candidate.revision < current.revision
    {
        return false;
    }
    if candidate.revision == current.revision {
        return candidate == current;
    }
    let old_identities = identity_map(current);
    let candidate_identities = identity_map(candidate);
    candidate.tabs.len() <= MAXIMUM_TABS_PER_CONTEXT
        && candidate_identities.len() == candidate.tabs.len() * 4
        && candidate_identities.iter().all(|(identity, member)| {
            old_identities
                .get(identity)
                .is_none_or(|old| reference_member_is_stable((old, member)))
        })
}

fn identity_map(
    tabs: &NativeStandaloneTabs,
) -> std::collections::BTreeMap<(u8, &str), &NativeSimpleTab> {
    tabs.tabs
        .iter()
        .flat_map(|tab| {
            [
                ((0, tab.tab_id.as_str()), tab),
                ((1, tab.panel_id.as_str()), tab),
                ((2, tab.presentation_id.as_str()), tab),
                ((3, tab.binding.terminal_id.as_str()), tab),
            ]
        })
        .collect()
}

fn reference_member_is_stable((current, candidate): (&NativeSimpleTab, &NativeSimpleTab)) -> bool {
    current.tab_id == candidate.tab_id
        && current.panel_id == candidate.panel_id
        && current.presentation_id == candidate.presentation_id
        && current.presentation_label == candidate.presentation_label
        && current.binding == candidate.binding
}

struct Random(u64);

impl Random {
    fn new(seed: u64) -> Self {
        Self(seed.wrapping_add(0x9e37_79b9_7f4a_7c15))
    }

    fn next(&mut self) -> u64 {
        let mut value = self.0;
        value ^= value << 13;
        value ^= value >> 7;
        value ^= value << 17;
        self.0 = value;
        value
    }

    fn index(&mut self, length: usize) -> usize {
        usize::try_from(self.next() % u64::try_from(length).unwrap()).unwrap()
    }
}
