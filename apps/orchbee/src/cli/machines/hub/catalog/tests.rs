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
        preferred: Vec::new(),
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
        preferred: Vec::new(),
    };

    let next = Machines {
        revision: Count::new(2).expect("valid revision"),
        entries: vec![Machine::new(
            shared_id.clone(),
            ssh("saved@host.invalid", "default"),
        )],
        serials: Serials::default(),
        preferred: Vec::new(),
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

#[test]
fn presence_attempts_four_machines_at_once_those_with_workspaces_first() {
    use crate::cli::machines::hub::machine::End;
    let ids: Vec<MachineId> = (1..=6_u32).map(|n| id(&format!("{n:032x}"))).collect();
    let mut machines = Machines {
        revision: Count::new(1).expect("valid revision"),
        entries: ids
            .iter()
            .map(|id| Machine::new(id.clone(), ssh("dev@host.invalid", "default")))
            .collect(),
        serials: Serials::default(),
        preferred: vec![ids[5].clone()],
    };
    let present = Conditions {
        presence: true,
        ..conditions()
    };
    let started = |outcome: &Outcome, machines: &Machines| -> Vec<(u64, MachineId)> {
        outcome
            .effects
            .iter()
            .filter_map(|effect| match effect {
                Effect::Start { carrier, .. } => {
                    Some((*carrier, machines.owner(*carrier)?.clone()))
                }
                _ => None,
            })
            .collect()
    };
    let outcome = machines.each(&present, Machine::attach);
    let first = started(&outcome, &machines);
    assert_eq!(
        first.iter().map(|(_, id)| id).collect::<Vec<_>>(),
        [&ids[5], &ids[0], &ids[1], &ids[2]]
    );
    // An attempt that ends frees its slot for the next queued Machine.
    let (carrier, ended) = first[1].clone();
    let end = End::Exited {
        code: Some(1),
        stderr: Vec::new(),
    };
    let outcome = machines.update(&ended, &present, |machine, cx| {
        machine.ended(carrier, &end, cx)
    });
    assert_eq!(
        started(&outcome, &machines)
            .into_iter()
            .map(|(_, id)| id)
            .collect::<Vec<_>>(),
        [ids[3].clone()]
    );
}
