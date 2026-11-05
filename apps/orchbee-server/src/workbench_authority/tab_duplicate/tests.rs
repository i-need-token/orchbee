use super::*;
use crate::{test_support::WorkbenchFixture, workbench_authority::WorkbenchGrant};
use orchbee_workbench_protocol::{
    PresentationBinding, SessionId, TabDuplicateCopy, TerminalId, WorkbenchTerminalFailureReason,
    WorkbenchTerminalOutcome, WorkerId,
};

fn identity(value: u128) -> String {
    format!("{value:032x}")
}

fn id(value: u128) -> WorkbenchRequestId {
    WorkbenchRequestId::parse(&identity(value)).unwrap()
}

fn tab(value: u128) -> WorkbenchTabId {
    WorkbenchTabId::parse(&identity(value)).unwrap()
}

fn geometry() -> WorkbenchTerminalGeometry {
    WorkbenchTerminalGeometry::new(80, 24, 0, 0).unwrap()
}

fn started() -> Option<WorkbenchTerminalOutcome> {
    Some(WorkbenchTerminalOutcome::Started {
        geometry: geometry(),
    })
}

fn copy(
    tab_id: u128,
    revision: u64,
    terminal: Option<WorkbenchTerminalOutcome>,
) -> Option<TabDuplicateCopy> {
    Some(TabDuplicateCopy {
        tab_id: tab(tab_id),
        revision: WorkbenchRevision::new(revision),
        terminal,
    })
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
            authority
                .append_fixture_tab(PresentationBinding {
                    terminal_id: TerminalId::parse(&identity(700 + index)).unwrap(),
                    worker_id: WorkerId::parse(&identity(2000)).unwrap(),
                })
                .unwrap();
        }
        let grant =
            WorkbenchGrant::manage_navigation_and_terminal_create(authority.session_id().clone());
        let context = authority.fixture_context_id().clone();
        let tabs = authority
            .snapshot()
            .terminal_contexts
            .iter()
            .find(|candidate| candidate.terminal_context_id == context)
            .unwrap()
            .tabs
            .iter()
            .map(|tab| tab.tab_id.clone())
            .collect();
        Self {
            authority,
            grant,
            tabs,
        }
    }

    fn start(&self, operation: u128, members: &[usize]) -> Action {
        Action::Start {
            operation_id: id(operation),
            terminal_context_id: self.authority.fixture_context_id().clone(),
            tab_ids: members
                .iter()
                .map(|index| self.tabs[*index].clone())
                .collect(),
            expected_revision: self.authority.snapshot().revision,
            geometry: geometry(),
        }
    }

    fn execute(&mut self, grant: &WorkbenchGrant, action: Action) -> Outcome {
        let request = WorkbenchTabDuplicateRequest {
            protocol_version: ProtocolVersion,
            request_id: id(1),
            session_id: self.authority.session_id().clone(),
            action,
        };
        self.authority
            .tab_duplicate(Some(grant), request, true)
            .outcome
    }

    fn apply(&mut self, action: Action) -> Outcome {
        let grant = self.grant.clone();
        self.execute(&grant, action)
    }

    fn read(&mut self, operation: u128) -> Outcome {
        self.apply(Action::Read {
            operation_id: id(operation),
        })
    }
}

fn rejected(outcome: &Outcome, expected: Rejection) -> bool {
    matches!(outcome, Outcome::Rejected { reason, .. } if *reason == expected)
}

#[test]
fn a_start_admits_only_a_current_ordered_scope_and_a_refusal_retains_nothing() {
    let mut fixture = Fixture::new(3);
    let read_only = WorkbenchGrant::read_only(fixture.authority.session_id().clone());
    let start = fixture.start(10, &[0, 2]);
    assert_eq!(fixture.execute(&read_only, start), Outcome::Denied {});

    let Action::Start {
        operation_id,
        terminal_context_id,
        tab_ids,
        expected_revision,
        geometry,
    } = fixture.start(11, &[0, 2])
    else {
        unreachable!()
    };
    let stale = fixture.apply(Action::Start {
        operation_id: operation_id.clone(),
        terminal_context_id: terminal_context_id.clone(),
        tab_ids: tab_ids.clone(),
        expected_revision: WorkbenchRevision::new(expected_revision.value() + 1),
        geometry,
    });
    assert!(rejected(&stale, Rejection::RevisionMismatch));
    let unknown = fixture.apply(Action::Start {
        operation_id: operation_id.clone(),
        terminal_context_id: terminal_context_id.clone(),
        tab_ids: vec![tab_ids[0].clone(), tab(9999)],
        expected_revision,
        geometry,
    });
    assert!(rejected(&unknown, Rejection::TabUnavailable));
    let reversed = fixture.apply(Action::Start {
        operation_id,
        terminal_context_id,
        tab_ids: tab_ids.into_iter().rev().collect(),
        expected_revision,
        geometry,
    });
    assert!(rejected(&reversed, Rejection::TabOrderMismatch));
    assert_eq!(fixture.read(11), Outcome::NotFound {});

    let admitted = fixture.apply(fixture.start(11, &[0, 2]));
    assert!(matches!(
        &admitted,
        Outcome::Available { status: Status::Pending {}, members, .. }
            if members.iter().all(|member| member.copy.is_none())
    ));
    // A read-only Client still observes the operation.
    assert_eq!(
        fixture.execute(
            &read_only,
            Action::Read {
                operation_id: id(11)
            }
        ),
        admitted
    );
}

#[test]
fn a_repeated_start_observes_its_operation_and_a_changed_one_conflicts() {
    let mut fixture = Fixture::new(3);
    let start = fixture.start(20, &[0, 1]);
    fixture.apply(start.clone());
    let mut plan = fixture.authority.claim_tab_duplicate().unwrap();
    plan.members[0].copy = copy(50, 9, None);
    assert!(
        fixture
            .authority
            .publish_tab_duplicate(&plan, Status::Pending {})
    );
    // A lost reply is answered by sending the same Start again: it observes, never copies again.
    assert_eq!(fixture.apply(start.clone()), fixture.read(20));
    assert!(matches!(
        fixture.read(20),
        Outcome::Available { members, .. } if members[0].copy == copy(50, 9, None)
    ));
    let Action::Start {
        operation_id,
        terminal_context_id,
        tab_ids,
        expected_revision,
        ..
    } = start
    else {
        unreachable!()
    };
    let other_members = Action::Start {
        operation_id: operation_id.clone(),
        terminal_context_id: terminal_context_id.clone(),
        tab_ids: tab_ids[..1].to_vec(),
        expected_revision,
        geometry: geometry(),
    };
    assert_eq!(fixture.apply(other_members), Outcome::Conflict {});
    let other_geometry = Action::Start {
        operation_id,
        terminal_context_id,
        tab_ids,
        expected_revision,
        geometry: WorkbenchTerminalGeometry::new(100, 30, 0, 0).unwrap(),
    };
    assert_eq!(fixture.apply(other_geometry), Outcome::Conflict {});
}

#[test]
fn progress_only_moves_forward_and_duplicated_needs_every_copy_started() {
    let mut fixture = Fixture::new(3);
    fixture.apply(fixture.start(30, &[0, 1]));
    let unclaimed = TabDuplicatePlan {
        operation_id: id(30),
        terminal_context_id: fixture.authority.fixture_context_id().clone(),
        geometry: geometry(),
        members: vec![
            TabDuplicateMember {
                tab_id: fixture.tabs[0].clone(),
                copy: None,
            },
            TabDuplicateMember {
                tab_id: fixture.tabs[1].clone(),
                copy: None,
            },
        ],
    };
    assert!(
        !fixture
            .authority
            .publish_tab_duplicate(&unclaimed, Status::Pending {})
    );
    let plan = fixture.authority.claim_tab_duplicate().unwrap();
    assert_eq!(plan, unclaimed);
    assert_eq!(fixture.authority.claim_tab_duplicate(), None);

    let publish = |fixture: &mut Fixture, first, second, status| {
        let mut next = plan.clone();
        next.members[0].copy = first;
        next.members[1].copy = second;
        fixture.authority.publish_tab_duplicate(&next, status)
    };
    // A copy after one whose terminal has not started is not a truthful order.
    assert!(!publish(
        &mut fixture,
        copy(50, 9, None),
        copy(51, 10, None),
        Status::Pending {}
    ));
    assert!(publish(
        &mut fixture,
        copy(50, 9, None),
        None,
        Status::Pending {}
    ));
    assert!(!publish(&mut fixture, None, None, Status::Pending {}));
    assert!(!publish(
        &mut fixture,
        copy(52, 9, None),
        None,
        Status::Pending {}
    ));
    assert!(publish(
        &mut fixture,
        copy(50, 10, started()),
        None,
        Status::Pending {}
    ));
    let failed = Some(WorkbenchTerminalOutcome::Failed {
        reason: WorkbenchTerminalFailureReason::TerminalCreateFailed,
    });
    assert!(!publish(
        &mut fixture,
        copy(50, 11, failed),
        None,
        Status::Pending {}
    ));
    assert!(!publish(
        &mut fixture,
        copy(50, 10, started()),
        None,
        Status::Duplicated {}
    ));
    assert!(publish(
        &mut fixture,
        copy(50, 10, started()),
        copy(51, 12, started()),
        Status::Duplicated {}
    ));
    // An ended operation is final.
    assert!(!publish(
        &mut fixture,
        copy(50, 10, started()),
        copy(51, 12, started()),
        Status::Pending {}
    ));
    assert!(matches!(
        fixture.read(30),
        Outcome::Available {
            status: Status::Duplicated {},
            ..
        }
    ));
}

#[test]
fn pending_operations_are_retained_while_settled_ones_give_way() {
    let mut fixture = Fixture::new(2);
    for operation in 0..MAXIMUM_OPERATIONS {
        let outcome = fixture.apply(fixture.start(1000 + operation as u128, &[0]));
        assert!(matches!(outcome, Outcome::Available { .. }));
    }
    assert_eq!(
        fixture.apply(fixture.start(5000, &[0])),
        Outcome::Unavailable {
            reason: Unavailable::Capacity
        }
    );
    let oldest = fixture.authority.claim_tab_duplicate().unwrap();
    fixture
        .authority
        .abandon_tab_duplicate(&oldest.operation_id);
    assert!(matches!(
        fixture.apply(fixture.start(5000, &[0])),
        Outcome::Available { .. }
    ));
    assert_eq!(fixture.read(1000), Outcome::NotFound {});
    assert!(matches!(fixture.read(1001), Outcome::Available { .. }));
}
