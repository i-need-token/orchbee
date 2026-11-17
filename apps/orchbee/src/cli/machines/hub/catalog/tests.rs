use super::*;
use crate::cli::machines::hub::lifetime::Moment;
use crate::cli::machines::hub::machine::{Kind, QUESTION, Route};
use orchbee_machine_protocol::SessionName;
use orchbee_workbench_client::application::machine_credentials::SecretKind;
use std::time::Instant;

fn id(hex: &str) -> MachineId {
    MachineId::new(hex).expect("valid hex machine id")
}

fn ssh(target: &str, session: &str) -> Kind {
    Kind::Ssh(Route {
        target: target.to_owned(),
        session: SessionName::new(session).expect("valid session name"),
    })
}

fn conditions() -> Conditions {
    Conditions {
        now: Moment {
            at: Instant::now(),
            wall: 1_000_000,
        },
        presence: false,
        closing: false,
        askers: Vec::new(),
        question: QUESTION,
    }
}

#[test]
fn reread_catalog_that_reuses_draft_id_for_another_target_drops_draft_and_forgets_answers() {
    let draft_id = id("11111111111111111111111111111111");
    let mut machines = Machines::empty();
    let Route { target, session } = match ssh("alpha@host.invalid", "default") {
        Kind::Ssh(route) => route,
        _ => unreachable!(),
    };
    machines.add_draft(Machine::draft(
        draft_id.clone(),
        Route { target, session },
        7,
    ));

    let next = Machines {
        revision: Count::new(1).expect("valid revision"),
        entries: vec![Machine::new(
            draft_id.clone(),
            ssh("beta@other.invalid", "default"),
        )],
        serials: Serials::default(),
    };

    let outcome = machines.replace(next, &conditions());
    assert!(
        machines.draft(&draft_id).is_none(),
        "colliding draft must be dropped"
    );
    assert_eq!(machines.entries.len(), 1);
    for kind in SecretKind::ALL {
        assert!(
            outcome.effects.iter().any(|effect| matches!(
                effect,
                Effect::Forget { machine, kind: forgotten }
                    if machine == &draft_id && *forgotten == kind
            )),
            "expected Effect::Forget for {kind:?}, got {:?}",
            outcome.effects
        );
    }
}

#[test]
fn constructed_catalog_and_draft_id_conflict_drops_duplicate_draft_and_forgets_answers() {
    let shared_id = id("22222222222222222222222222222222");
    let Route { target, session } = match ssh("draft@host.invalid", "work") {
        Kind::Ssh(route) => route,
        _ => unreachable!(),
    };
    let mut machines = Machines {
        revision: Count::new(1).expect("valid revision"),
        entries: vec![
            Machine::new(shared_id.clone(), ssh("saved@host.invalid", "default")),
            Machine::draft(shared_id.clone(), Route { target, session }, 9),
        ],
        serials: Serials::default(),
    };

    let next = Machines {
        revision: Count::new(2).expect("valid revision"),
        entries: vec![Machine::new(
            shared_id.clone(),
            ssh("saved@host.invalid", "default"),
        )],
        serials: Serials::default(),
    };

    let outcome = machines.replace(next, &conditions());
    assert_eq!(
        machines.entries.len(),
        1,
        "conflicting draft must not remain beside the saved Machine"
    );
    assert!(machines.draft(&shared_id).is_none());
    for kind in SecretKind::ALL {
        assert!(
            outcome.effects.iter().any(|effect| matches!(
                effect,
                Effect::Forget { machine, kind: forgotten }
                    if machine == &shared_id && *forgotten == kind
            )),
            "expected Effect::Forget for conflicting draft {kind:?}, got {:?}",
            outcome.effects
        );
    }
}
