use super::*;

const TRACE_SEEDS: u64 = 64;
const STEPS_PER_TRACE: usize = 38;
const ACTION_COUNT: usize = 29;

#[derive(Clone, Copy, Debug)]
enum Action {
    SetPanelInput,
    ClearPanelInput,
    SameRevisionPanelInput,
    OlderPanelInput,
    SetPanelLabel,
    ClearPanelLabel,
    SameRevisionPanelLabel,
    OlderPanelLabel,
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
    MigrateMemberToFreshOwner,
    ChangePanelId,
    ChangePresentationId,
    ChangePresentationLabel,
    ChangeWorkerId,
    ChangeTerminalId,
    DuplicateTabId,
    SwapAllMembers,
}

impl Action {
    const ALL: [Self; ACTION_COUNT] = [
        Self::SetPanelInput,
        Self::ClearPanelInput,
        Self::SameRevisionPanelInput,
        Self::OlderPanelInput,
        Self::SetPanelLabel,
        Self::ClearPanelLabel,
        Self::SameRevisionPanelLabel,
        Self::OlderPanelLabel,
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
        Self::MigrateMemberToFreshOwner,
        Self::ChangePanelId,
        Self::ChangePresentationId,
        Self::ChangePresentationLabel,
        Self::ChangeWorkerId,
        Self::ChangeTerminalId,
        Self::DuplicateTabId,
        Self::SwapAllMembers,
    ];

    const fn must_be_accepted(self) -> bool {
        matches!(
            self,
            Self::SetPanelInput
                | Self::ClearPanelInput
                | Self::SetPanelLabel
                | Self::ClearPanelLabel
                | Self::ExactSameRevision
                | Self::RevisionOnly
                | Self::RenameOne
                | Self::RenameMany
                | Self::Append
                | Self::RenameAndAppend
                | Self::ReorderExisting
                | Self::InsertBeforeExisting
                | Self::RemoveExisting
                | Self::MigrateMemberToFreshOwner
        )
    }
}

#[test]
fn generated_label_evolution_traces_match_the_independent_refinement() {
    let mut coverage = [0_usize; ACTION_COUNT];
    let mut accepted = 0_usize;
    let mut rejected = 0_usize;
    for seed in 0..TRACE_SEEDS {
        let actions = (0..STEPS_PER_TRACE)
            .map(|step| {
                let index = (usize::try_from(seed).unwrap() + step) % ACTION_COUNT;
                coverage[index] += 1;
                Action::ALL[index]
            })
            .collect::<Vec<_>>();
        match replay(seed, &actions) {
            Ok((yes, no)) => {
                accepted += yes;
                rejected += no;
            }
            Err(error) => {
                let mut reduced = actions;
                let mut index = 0;
                while index < reduced.len() {
                    let mut candidate = reduced.clone();
                    candidate.remove(index);
                    if replay(seed, &candidate).is_err() {
                        reduced = candidate;
                    } else {
                        index += 1;
                    }
                }
                panic!(
                    "seed={seed}; reduced={reduced:?}; original={error}; replay={:?}",
                    replay(seed, &reduced)
                );
            }
        }
    }
    assert!(coverage.into_iter().all(|count| count > 0));
    assert!(accepted > 0 && rejected > 0);
}

fn replay(seed: u64, actions: &[Action]) -> Result<(usize, usize), String> {
    let mut random = Random::new(seed);
    let mut current = projected(2 + random.index(5), 100 + seed * 100);
    // Disjoint monotone identity domain remains stable under trace reduction.
    let mut fresh = 800_000_u128;
    let (mut accepted, mut rejected) = (0, 0);
    for (step, action) in actions.iter().copied().enumerate() {
        let candidate = apply(action, &current, &mut random, &mut fresh);
        let expected = reference_accepts(&current, &candidate);
        let result = validate_canonical_refresh(&current, candidate.clone());
        if expected != action.must_be_accepted() || result.is_ok() != expected {
            return Err(format!(
                "step={step}; action={action:?}; expected={expected}; current={current:?}; candidate={candidate:?}; result={result:?}"
            ));
        }
        match result {
            Ok(next) => {
                accepted += 1;
                current = next;
            }
            Err(_) => rejected += 1,
        }
    }
    Ok((accepted, rejected))
}

fn apply(
    action: Action,
    current: &NativeContextTabs,
    random: &mut Random,
    fresh: &mut u128,
) -> NativeContextTabs {
    if matches!(action, Action::ExactSameRevision) {
        return current.clone();
    }

    let mut candidate = successor(current, 0);
    let first = random.index(current.tabs.len());
    match action {
        Action::SetPanelInput | Action::SameRevisionPanelInput | Action::OlderPanelInput => {
            candidate.tabs[first].panels[0].right_click =
                match current.tabs[first].panels[0].right_click {
                    orchbee_workbench_protocol::PanelRightClickTarget::Orchbee => {
                        orchbee_workbench_protocol::PanelRightClickTarget::Panel
                    }
                    orchbee_workbench_protocol::PanelRightClickTarget::Panel => {
                        orchbee_workbench_protocol::PanelRightClickTarget::Orchbee
                    }
                };
            if matches!(action, Action::SameRevisionPanelInput) {
                candidate.revision = current.revision;
            }
            if matches!(action, Action::OlderPanelInput) {
                candidate.revision = WorkbenchRevision::new(current.revision.value() - 1);
            }
        }
        Action::ClearPanelInput => {
            candidate.tabs[first].panels[0].right_click =
                orchbee_workbench_protocol::PanelRightClickTarget::Orchbee
        }

        Action::SetPanelLabel | Action::SameRevisionPanelLabel | Action::OlderPanelLabel => {
            let value = format!("custom-{}", random.next());
            candidate.tabs[first].panels[0].custom_label =
                Some(orchbee_workbench_protocol::PanelCustomLabel::parse(&value).unwrap());
            if matches!(action, Action::SameRevisionPanelLabel) {
                candidate.revision = current.revision;
            }
            if matches!(action, Action::OlderPanelLabel) {
                candidate.revision = WorkbenchRevision::new(current.revision.value() - 1);
            }
        }
        Action::ClearPanelLabel => candidate.tabs[first].panels[0].custom_label = None,
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
            candidate.location =
                NativeContextLocation::Standalone(context_id(200_000 + u128::from(random.next())));
        }
        Action::RemoveExisting => candidate.tabs.truncate((current.tabs.len() - 1).max(1)),
        Action::ReorderExisting => candidate.tabs.rotate_left(1),
        Action::InsertBeforeExisting => {
            let mut source = successor(current, 1);
            let inserted = source.tabs.pop().unwrap();
            candidate.tabs.insert(first, inserted);
        }
        Action::MigrateMemberToFreshOwner => {
            // The one authorized delta: a legal member-owner migration —
            // the Tab gains a fresh identity (the original id vanishes,
            // exempting the floor) while every member tuple stays exact.
            // The fresh id comes from a deterministic domain that provably
            // never collides with a live Tab, not from a lucky random draw.
            let owner = tab_id(fresh_id(fresh));
            assert!(
                candidate.tabs.iter().all(|tab| tab.tab_id != owner),
                "fresh owner {owner:?} must not collide with a live Tab"
            );
            candidate.tabs[first].tab_id = owner;
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
        Action::DuplicateTabId => {
            // Minimal duplicate-Tab counterexample: a twin keeps the
            // owner's identity while every member identity is fresh, so
            // the (a) uniqueness rule alone must reject.
            let mut twin = candidate.tabs[first].clone();
            replace_members_with_fresh(&mut twin, fresh);
            candidate.tabs.push(twin);
        }
        Action::SwapAllMembers => {
            // Minimal surviving-Tab floor violation: the Tab keeps its
            // identity but every member is replaced, so no original member
            // remains under the same Tab and the (c) floor must reject.
            replace_members_with_fresh(&mut candidate.tabs[first], fresh);
        }
    }
    candidate
}

/// A deterministic fresh identity from the counter domain — guaranteed
/// disjoint from seed, appended and previously minted identities.
fn fresh_id(fresh: &mut u128) -> u128 {
    *fresh += 1;
    *fresh
}

/// Replaces every member identity of `tab` — the tab-level primary member
/// and each leaf panel member — with fresh deterministic identities while
/// keeping the owning Tab untouched.
fn replace_members_with_fresh(tab: &mut NativeSimpleTab, fresh: &mut u128) {
    tab.panel_id = panel_id(fresh_id(fresh));
    tab.presentation_id = presentation_id(fresh_id(fresh));
    tab.presentation_label = generated_label(u64::try_from(*fresh).unwrap_or(u64::MAX));
    tab.binding.worker_id = worker_id(fresh_id(fresh));
    tab.binding.terminal_id = terminal_id(fresh_id(fresh));
    for panel in tab.panels.iter_mut() {
        panel.panel_id = panel_id(fresh_id(fresh));
        panel.presentation_id = presentation_id(fresh_id(fresh));
        panel.presentation_label = generated_label(u64::try_from(*fresh).unwrap_or(u64::MAX));
        panel.binding.worker_id = worker_id(fresh_id(fresh));
        panel.binding.terminal_id = terminal_id(fresh_id(fresh));
    }
}

fn successor(current: &NativeContextTabs, appended: usize) -> NativeContextTabs {
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

fn rename(candidate: &mut NativeContextTabs, index: usize, value: u64) {
    candidate.tabs[index].tab_label = generated_label(value);
}

fn generated_label(value: u64) -> WorkbenchLabel {
    label(&format!("  rename-{value}-雪  "))
}

/// The independent set/relation oracle for the canonical refresh contract.
/// (a) envelope and Tab domain: same Site/context, nondecreasing revision,
/// same-revision projection equality, globally unique Tab ids within the
/// existing bound. (b) member identity domains: Panel, Presentation and
/// Terminal identities each stay injective across the candidate's leaf
/// panels, and any overlap with a current member on any of the three
/// domains requires the complete immutable tuple — the owning Tab is the
/// one authorized delta. (c) surviving-Tab floor: a Tab identity still
/// present in the candidate must still own at least one of its original
/// members under that same identity; a vanished Tab is exempt. The oracle
/// quantifies over the same member tuples the validator enumerates but
/// expresses every rule relationally, not as the production three-map loop.
fn reference_accepts(current: &NativeContextTabs, candidate: &NativeContextTabs) -> bool {
    // (a) Envelope: same Site/context and a nondecreasing revision; the
    // same revision requires the complete projection — Tabs, navigation
    // and projects — exact.
    if candidate.site_id != current.site_id
        || candidate.location != current.location
        || candidate.revision < current.revision
    {
        return false;
    }
    if candidate.revision == current.revision {
        return candidate == current;
    }
    // (a) Tab domain: ids globally unique within the existing bound.
    if candidate.tabs.len() > MAXIMUM_TABS_PER_CONTEXT {
        return false;
    }
    let candidate_tab_ids: std::collections::BTreeSet<&WorkbenchTabId> =
        candidate.tabs.iter().map(|tab| &tab.tab_id).collect();
    if candidate_tab_ids.len() != candidate.tabs.len() {
        return false;
    }
    // (b) Each member-identity domain stays injective across the
    // candidate's leaf panels — the same relation the structural-id rule
    // enforces. The tab-level primary member shares its leaf's identity
    // and must not be double-counted here.
    let candidate_panels: Vec<&NativePanel> = candidate
        .tabs
        .iter()
        .flat_map(|tab| tab.panels.iter())
        .collect();
    let unique_panels: std::collections::BTreeSet<&WorkbenchPanelId> = candidate_panels
        .iter()
        .map(|panel| &panel.panel_id)
        .collect();
    let unique_presentations: std::collections::BTreeSet<&TerminalPresentationId> =
        candidate_panels
            .iter()
            .map(|panel| &panel.presentation_id)
            .collect();
    let unique_terminals: std::collections::BTreeSet<&TerminalId> = candidate_panels
        .iter()
        .map(|panel| &panel.binding.terminal_id)
        .collect();
    if unique_panels.len() != candidate_panels.len()
        || unique_presentations.len() != candidate_panels.len()
        || unique_terminals.len() != candidate_panels.len()
    {
        return false;
    }
    // (b) Overlap on any of the three identity domains requires the
    // complete immutable tuple; the owning Tab is free.
    let old_members: Vec<MemberTuple<'_>> = current.tabs.iter().flat_map(member_tuples).collect();
    let new_members: Vec<MemberTuple<'_>> = candidate.tabs.iter().flat_map(member_tuples).collect();
    for member in &new_members {
        for old in &old_members {
            let overlaps =
                member.1 == old.1 || member.2 == old.2 || member.4.terminal_id == old.4.terminal_id;
            if overlaps
                && (member.1 != old.1
                    || member.2 != old.2
                    || member.3 != old.3
                    || member.4 != old.4)
            {
                return false;
            }
        }
    }
    // (c) Surviving-Tab floor: a Tab identity still present must still own
    // at least one of its original members under that same identity.
    for tab in &current.tabs {
        if !candidate_tab_ids.contains(&tab.tab_id) {
            continue;
        }
        let retains = member_tuples(tab).any(|old| {
            new_members.iter().any(|member| {
                member.0 == old.0
                    && (member.1 == old.1
                        || member.2 == old.2
                        || member.4.terminal_id == old.4.terminal_id)
            })
        });
        if !retains {
            return false;
        }
    }
    true
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
