use super::*;
use crate::{
    WorkbenchPanelCreateBegin, WorkbenchPanelCreatePreparation, WorkbenchTabCloseBegin,
    test_support::WorkbenchFixture,
};
use orchbee_workbench_protocol::{
    PresentationBinding, SessionId, TabSetCloseChild, TabSetCloseObservation, TerminalId,
    WorkbenchAction, WorkbenchCommand, WorkbenchPanelSplitDirection,
    WorkbenchTabCloseAttemptGeneration, WorkbenchTabClosePhase, WorkbenchTabCloseStatus,
    WorkbenchTerminalGeometry,
};

fn identity(value: u128) -> String {
    format!("{value:032x}")
}

fn id(value: u128) -> WorkbenchRequestId {
    WorkbenchRequestId::parse(&identity(value)).unwrap()
}

fn worker() -> (WorkerId, NonZeroU64) {
    (WorkerId::parse(&identity(2000)).unwrap(), NonZeroU64::MIN)
}

fn binding(value: u128) -> PresentationBinding {
    PresentationBinding {
        terminal_id: TerminalId::parse(&identity(value)).unwrap(),
        worker_id: worker().0,
    }
}

struct Fixture {
    authority: WorkbenchAuthority,
    grant: WorkbenchGrant,
    tabs: Vec<WorkbenchTabId>,
}

impl Fixture {
    fn new(count: u128) -> Self {
        let mut authority =
            WorkbenchAuthority::workspace_fixture(SessionId::parse(&identity(100)).unwrap())
                .unwrap();
        for index in 0..count {
            authority.append_fixture_tab(binding(700 + index)).unwrap();
        }
        let grant =
            WorkbenchGrant::read_only(authority.session_id().clone()).with_terminal_lifecycle();
        let mut fixture = Self {
            authority,
            grant,
            tabs: Vec::new(),
        };
        fixture.tabs = fixture.current_tabs();
        fixture
    }

    fn context(&self) -> WorkbenchTerminalContextId {
        self.authority.fixture_context_id().clone()
    }

    fn current_tabs(&self) -> Vec<WorkbenchTabId> {
        let context = self.context();
        self.authority
            .snapshot()
            .terminal_contexts
            .iter()
            .find(|candidate| candidate.terminal_context_id == context)
            .unwrap()
            .tabs
            .iter()
            .map(|tab| tab.tab_id.clone())
            .collect()
    }

    fn start(&self, operation: u128, members: &[usize]) -> Action {
        Action::Start {
            operation_id: id(operation),
            terminal_context_id: self.context(),
            tab_ids: members
                .iter()
                .map(|index| self.tabs[*index].clone())
                .collect(),
            expected_revision: self.authority.snapshot().revision,
            condition: Condition::Confirmed,
        }
    }

    fn execute(&mut self, grant: &WorkbenchGrant, request: u128, action: Action) -> Outcome {
        let request = WorkbenchTabSetCloseRequest {
            protocol_version: ProtocolVersion,
            request_id: id(request),
            session_id: self.authority.session_id().clone(),
            action,
        };
        match self
            .authority
            .begin_tab_set_close(Some(grant), request, Some(worker()), true)
        {
            TabSetCloseBegin::Ready(result) => result.outcome,
            TabSetCloseBegin::Admitted(admission) => admission.admit().outcome,
        }
    }

    fn apply(&mut self, request: u128, action: Action) -> Outcome {
        let grant = self.grant.clone();
        self.execute(&grant, request, action)
    }

    fn close(&self, request: u128, member: usize) -> WorkbenchCommand {
        WorkbenchCommand {
            protocol_version: ProtocolVersion,
            request_id: id(request),
            session_id: self.authority.session_id().clone(),
            expected_revision: self.authority.snapshot().revision,
            action: WorkbenchAction::TabClose {
                terminal_context_id: self.context(),
                tab_id: self.tabs[member].clone(),
            },
        }
    }

    /// Whether the set's token admits `command` as member `index`'s own Close.
    fn token_admits(&mut self, command: WorkbenchCommand, operation: u128, index: usize) -> bool {
        let grant = self.grant.clone();
        matches!(
            self.authority
                .begin_tab_set_member_close(Some(&grant), command, &id(operation), index),
            WorkbenchTabCloseBegin::Admitted(_)
        )
    }

    /// Splits member `index`'s only Panel; the new terminal stays unsettled.
    fn split(&mut self, index: usize) {
        let membership = self.authority.fixture_presentation_membership().unwrap();
        let source = membership
            .members_for_tab(&self.tabs[index])
            .next()
            .unwrap()
            .clone();
        let command = WorkbenchCommand {
            protocol_version: ProtocolVersion,
            request_id: id(900),
            session_id: self.authority.session_id().clone(),
            expected_revision: self.authority.snapshot().revision,
            action: WorkbenchAction::PanelSplit {
                right_click: Default::default(),
                terminal_context_id: self.context(),
                tab_id: source.tab_id().clone(),
                panel_id: source.panel_id().clone(),
                direction: WorkbenchPanelSplitDirection::Right,
                label: WorkbenchLabel::parse("second").unwrap(),
                geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
                launch: None,
                profile: None,
                options: Default::default(),
            },
        };
        let grant = WorkbenchGrant::manage_navigation_and_terminal_create(
            self.authority.session_id().clone(),
        );
        let WorkbenchPanelCreateBegin::Admitted(admission) = self
            .authority
            .begin_terminal_panel_create(Some(&grant), command)
        else {
            panic!("split admission")
        };
        let WorkbenchPanelCreatePreparation::Prepared(prepared) =
            admission.finish(binding(800), None)
        else {
            panic!("split preparation")
        };
        let _pending = prepared.commit_into_pending();
    }
}

fn child(
    request: u128,
    generation: u64,
    status: Option<WorkbenchTabCloseStatus>,
) -> TabSetCloseChild {
    TabSetCloseChild {
        request_id: id(request),
        observation: status.map(|status| TabSetCloseObservation {
            attempt_generation: WorkbenchTabCloseAttemptGeneration::new(generation).unwrap(),
            status,
        }),
    }
}

fn closing() -> Option<WorkbenchTabCloseStatus> {
    Some(WorkbenchTabCloseStatus::Closing {
        phase: WorkbenchTabClosePhase::Queued,
    })
}

fn closed() -> Option<WorkbenchTabCloseStatus> {
    Some(WorkbenchTabCloseStatus::Closed {
        revision: WorkbenchRevision::new(9),
    })
}

fn rejected(outcome: &Outcome, expected: Rejection) -> bool {
    matches!(outcome, Outcome::Rejected { reason, .. } if *reason == expected)
}

#[test]
fn a_set_admits_only_a_current_ordered_scope_that_no_other_work_owns() {
    let mut fixture = Fixture::new(3);
    let revision = fixture.authority.snapshot().revision;
    let mut stale = fixture.start(10, &[0]);
    let Action::Start {
        expected_revision, ..
    } = &mut stale
    else {
        unreachable!()
    };
    *expected_revision = WorkbenchRevision::new(revision.value() - 1);
    assert!(rejected(
        &fixture.apply(1, stale),
        Rejection::RevisionMismatch
    ));
    let mut foreign = fixture.start(10, &[0]);
    let Action::Start {
        terminal_context_id,
        ..
    } = &mut foreign
    else {
        unreachable!()
    };
    *terminal_context_id = WorkbenchTerminalContextId::parse(&identity(4242)).unwrap();
    assert!(rejected(
        &fixture.apply(2, foreign),
        Rejection::TerminalContextUnavailable
    ));
    let mut missing = fixture.start(10, &[0]);
    let Action::Start { tab_ids, .. } = &mut missing else {
        unreachable!()
    };
    tab_ids.push(WorkbenchTabId::parse(&identity(4243)).unwrap());
    assert!(rejected(
        &fixture.apply(3, missing),
        Rejection::TabUnavailable
    ));
    assert!(rejected(
        &fixture.apply(4, fixture.start(10, &[1, 0])),
        Rejection::TabOrderMismatch
    ));
    let viewer = WorkbenchGrant::read_only(fixture.authority.session_id().clone());
    assert_eq!(
        fixture.execute(&viewer, 5, fixture.start(10, &[0])),
        Outcome::Denied {},
        "only a terminal lifecycle grant starts a set"
    );

    assert!(matches!(
        fixture.apply(6, fixture.start(10, &[0, 1])),
        Outcome::Available {
            status: Status::Pending {},
            ..
        }
    ));
    assert_eq!(
        fixture.apply(7, fixture.start(11, &[1, 2])),
        Outcome::Unavailable {
            reason: Unavailable::Busy
        },
        "a pending set owns its members"
    );
    let committed = fixture
        .authority
        .prepare_fixture_tab(binding(750))
        .unwrap()
        .commit();
    drop(committed);
    fixture.tabs = fixture.current_tabs();
    assert_eq!(
        fixture.apply(8, fixture.start(11, &[2, 3])),
        Outcome::Unavailable {
            reason: Unavailable::Busy
        },
        "an unsettled create still owes its Tab a terminal"
    );
    assert!(
        matches!(
            fixture.apply(9, fixture.start(11, &[2])),
            Outcome::Available { .. }
        ),
        "busy is decided per member"
    );
    assert_eq!(
        fixture.current_tabs(),
        fixture.tabs,
        "admission changes no Tab"
    );
}

#[test]
fn a_repeated_start_observes_its_set_and_a_changed_one_conflicts() {
    let mut fixture = Fixture::new(2);
    let start = fixture.start(10, &[0, 1]);
    let admitted = fixture.apply(1, start.clone());
    assert!(matches!(admitted, Outcome::Available { .. }));
    assert_eq!(fixture.apply(2, start.clone()), admitted);
    assert_eq!(
        fixture.apply(
            3,
            Action::Read {
                operation_id: id(10)
            }
        ),
        admitted
    );
    for change in 0..3 {
        let mut changed = start.clone();
        let Action::Start {
            tab_ids,
            condition,
            expected_revision,
            ..
        } = &mut changed
        else {
            unreachable!()
        };
        match change {
            0 => {
                tab_ids.pop();
            }
            1 => *condition = Condition::IfIdle,
            _ => *expected_revision = WorkbenchRevision::new(expected_revision.value() + 1),
        }
        assert_eq!(fixture.apply(4, changed), Outcome::Conflict {}, "{change}");
    }
    assert_eq!(
        fixture.apply(
            5,
            Action::Read {
                operation_id: id(11)
            }
        ),
        Outcome::NotFound {}
    );
}

#[test]
fn set_progress_only_moves_forward_and_each_child_needs_the_next_unchanged_member() {
    let mut fixture = Fixture::new(3);
    assert!(matches!(
        fixture.apply(1, fixture.start(10, &[0, 1, 2])),
        Outcome::Available { .. }
    ));
    let first = fixture.close(20, 0);
    assert!(
        !fixture.token_admits(first.clone(), 10, 0),
        "only the claimed set submits"
    );
    let mut plan = fixture.authority.claim_tab_set_close().unwrap();
    assert!(fixture.authority.claim_tab_set_close().is_none());
    assert!(!fixture.token_admits(fixture.close(21, 1), 10, 1));
    assert!(!fixture.token_admits(first.clone(), 10, 1));
    assert!(!fixture.token_admits(first.clone(), 11, 0));
    assert!(fixture.token_admits(first, 10, 0));

    let publish = |fixture: &mut Fixture, plan: &TabSetClosePlan, status: Status| {
        fixture.authority.publish_tab_set_close(plan, status)
    };
    let mut early = plan.clone();
    early.members[1].member.close = Some(child(31, 1, None));
    assert!(
        !publish(&mut fixture, &early, Status::Pending {}),
        "no member is submitted before an earlier one is done"
    );
    plan.members[0].member.close = Some(child(30, 1, None));
    assert!(publish(&mut fixture, &plan, Status::Pending {}));
    let mut regressed = plan.clone();
    regressed.members[0].member.close = None;
    assert!(!publish(&mut fixture, &regressed, Status::Pending {}));
    regressed.members[0].member.close = Some(child(32, 1, None));
    assert!(!publish(&mut fixture, &regressed, Status::Pending {}));
    plan.members[0].member.close = Some(child(30, 2, closing()));
    assert!(publish(&mut fixture, &plan, Status::Pending {}));
    regressed.members[0].member.close = Some(child(30, 1, closing()));
    assert!(!publish(&mut fixture, &regressed, Status::Pending {}));
    assert!(
        !publish(&mut fixture, &plan, Status::Closed {}),
        "Closed needs every member done"
    );
    plan.members[0].member.close = Some(child(30, 2, closed()));
    plan.members[1].member.absent = true;
    assert!(publish(&mut fixture, &plan, Status::Pending {}));
    regressed = plan.clone();
    regressed.members[1].member.absent = false;
    assert!(!publish(&mut fixture, &regressed, Status::Pending {}));
    regressed = plan.clone();
    regressed.members[0].member.close = Some(child(30, 2, closing()));
    assert!(!publish(&mut fixture, &regressed, Status::Pending {}));

    assert_eq!(
        fixture.authority.tab_set_close_member_state(&id(10), 2),
        Some(TabSetCloseMemberState::Unchanged)
    );
    fixture.split(2);
    assert_eq!(
        fixture.authority.tab_set_close_member_state(&id(10), 2),
        Some(TabSetCloseMemberState::Changed),
        "a member whose terminals changed is no longer the one the person chose"
    );
    assert!(!fixture.token_admits(fixture.close(22, 2), 10, 2));
    assert!(publish(
        &mut fixture,
        &plan,
        Status::Failed {
            reason: Failure::SourceChanged
        }
    ));
    assert!(
        !publish(&mut fixture, &plan, Status::Pending {}),
        "a settled set is final"
    );
}
