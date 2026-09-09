use super::super::{Action, Outcome};
use super::fixture::{Effect, Fixture, client_id};
use orchbee_client_server_protocol::{
    ClientMessage, ControlInput, ControllerId, Geometry, ServerMessage,
    TerminalControlAction as Control, TerminalControlOutcome as ControlOutcome,
    TerminalControlRequest, TerminalViewerRequest, ViewerId,
};
use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU64;

mod oracle;
mod replay;
use oracle::run;
use replay::{decode, encode, shrink};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
    Attach,
    Acquire,
    Control,
    Poll,
    Feedback,
    Detach,
    Drop,
    Dispose,
    ReplaceWorker,
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct Step {
    kind: Kind,
    session: usize,
    viewer: usize,
    target: usize,
    grant: usize,
    value: u64,
    alternate: bool,
}
impl Step {
    fn new(kind: Kind) -> Self {
        Self {
            kind,
            session: 0,
            viewer: 0,
            target: 0,
            grant: 0,
            value: 1,
            alternate: false,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct Failure {
    fingerprint: (&'static str, String),
    detail: String,
}
type Coverage = BTreeSet<&'static str>;
const REQUIRED: &[&str] = &[
    "two-live",
    "duplicate",
    "full-duplicate",
    "local-capacity",
    "worker-capacity",
    "wrong-scope",
    "stale-viewer",
    "wrong-grant",
    "equal-sequence",
    "gap",
    "stale-grant",
    "equal-frame",
    "bad-feedback",
    "detach-sibling",
    "absent-detach",
    "disposed-sibling",
    "drop-multiple",
    "worker-loss",
    "worker-generation-control",
    "worker-generation-poll",
    "rejected-candidate",
];

#[test]
fn generated_sessions_match_independent_routing_oracle_with_shrinking_and_replay() {
    let mut coverage = Coverage::new();
    let mut interleaved = 0;
    for seed in 1..=64 {
        let trace = generate(seed);
        let serialized = encode(&trace);
        let decoded = decode(&serialized).expect("valid serialized trace");
        assert_eq!(encode(&decoded), serialized, "seed={seed}");
        let first = run(&decoded);
        let observed = match first {
            Ok(c) => c,
            Err(failure) => {
                if failure.fingerprint.0 == "fixture" || failure.fingerprint.1 == "fixture:barrier"
                {
                    panic!(
                        "routing-v2 fixture failure; xorshift64 seed={seed} steps=128; original={serialized}; failure={failure:?}"
                    );
                }
                let reduced = shrink(decoded, &failure.fingerprint, |steps| {
                    run(steps).err().map(|error| error.fingerprint)
                });
                panic!(
                    "routing-v2 xorshift64 seed={seed} steps=128; original={serialized}; failure={failure:?}; reduced={}; reduced_failure={:?}",
                    encode(&reduced),
                    run(&reduced)
                );
            }
        };
        // A second fresh fixture establishes replay beyond codec round-trip.
        assert_eq!(
            run(&trace),
            Ok(observed.clone()),
            "seed={seed}; replay={serialized}"
        );
        if observed.contains("interleaved") {
            interleaved += 1;
        }
        coverage.extend(observed);
    }
    for class in REQUIRED {
        assert!(
            coverage.contains(class),
            "missing generated coverage {class}; reached={coverage:?}"
        );
    }
    assert!(
        interleaved >= 8,
        "two-live interleaving seeds={interleaved}"
    );
}

fn random(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}
fn generate(seed: u64) -> Vec<Step> {
    let mut rng = seed;
    let mut trace = Vec::new();
    let mut slots: Vec<(usize, usize, usize)> = Vec::new();
    let mut grants = Vec::new();
    let owner = (random(&mut rng) % 2) as usize;
    let mut targets: Vec<_> = (0..10).collect();
    for i in (1..10).rev() {
        let j = (random(&mut rng) % (i + 1) as u64) as usize;
        targets.swap(i, j);
    }
    // Generate a capacity frontier with randomized targets and Session distribution.
    for (i, target) in targets.iter().take(8).enumerate() {
        let s = if seed.is_multiple_of(2) { owner } else { i % 2 };
        let step = Step {
            kind: Kind::Attach,
            session: s,
            viewer: i,
            target: *target,
            alternate: random(&mut rng).is_multiple_of(2),
            ..Step::new(Kind::Attach)
        };
        trace.push(step);
        slots.push((i, s, *target));
    }
    trace.push(Step {
        kind: Kind::Attach,
        session: owner,
        viewer: 8,
        target: targets[0],
        alternate: true,
        ..Step::new(Kind::Attach)
    });
    trace.push(Step {
        kind: Kind::Attach,
        session: 1 - owner,
        viewer: 9,
        target: targets[8],
        ..Step::new(Kind::Attach)
    });
    // Correlated generated commands exercise each independent live grant and frame route.
    for &(viewer, session, target) in &slots {
        let grant = 1000 + viewer;
        grants.push(grant);
        trace.push(Step {
            kind: Kind::Acquire,
            session,
            viewer,
            target,
            grant,
            ..Step::new(Kind::Acquire)
        });
        trace.push(Step {
            kind: Kind::Control,
            session,
            viewer,
            target,
            grant,
            value: 1,
            alternate: random(&mut rng).is_multiple_of(2),
        });
        trace.push(Step {
            kind: Kind::Poll,
            session,
            viewer,
            target,
            ..Step::new(Kind::Poll)
        });
    }
    let (first, first_session, first_target) = slots[0];
    let (second, second_session, second_target) = slots[1];
    trace.push(Step {
        kind: Kind::Feedback,
        session: first_session,
        viewer: first,
        value: 2,
        ..Step::new(Kind::Feedback)
    });
    trace.push(Step {
        kind: Kind::Feedback,
        session: second_session,
        viewer: second,
        value: 1,
        alternate: random(&mut rng).is_multiple_of(2),
        ..Step::new(Kind::Feedback)
    });
    trace.push(Step {
        kind: Kind::Control,
        session: first_session,
        viewer: first,
        grant: 1000 + first,
        value: 4,
        ..Step::new(Kind::Control)
    });
    trace.push(Step {
        kind: Kind::Control,
        session: 1 - first_session,
        viewer: first,
        grant: 1000 + first,
        ..Step::new(Kind::Control)
    });
    trace.push(Step {
        kind: Kind::Dispose,
        target: first_target,
        ..Step::new(Kind::Dispose)
    });
    trace.push(Step {
        kind: Kind::Detach,
        session: second_session,
        viewer: second,
        ..Step::new(Kind::Detach)
    });
    trace.push(Step {
        kind: Kind::Detach,
        session: second_session,
        viewer: second,
        ..Step::new(Kind::Detach)
    });
    trace.push(Step {
        kind: Kind::Attach,
        session: second_session,
        viewer: 40,
        target: second_target,
        alternate: true,
        ..Step::new(Kind::Attach)
    });
    slots.push((40, second_session, second_target));
    trace.push(Step {
        kind: Kind::Acquire,
        session: second_session,
        viewer: 40,
        grant: 1040,
        ..Step::new(Kind::Acquire)
    });
    grants.push(1040);
    trace.push(Step {
        kind: Kind::Control,
        session: second_session,
        viewer: 40,
        grant: 1000 + second,
        ..Step::new(Kind::Control)
    });
    for index in trace.len()..120 {
        let roll = random(&mut rng) % 100;
        let (viewer, session, target) = slots[(random(&mut rng) % slots.len() as u64) as usize];
        let mut step = Step {
            session,
            viewer,
            target,
            grant: grants.last().copied().unwrap_or(999),
            value: 1 + random(&mut rng) % 5,
            alternate: random(&mut rng).is_multiple_of(2),
            ..Step::new(Kind::Poll)
        };
        step.kind = match roll {
            0..=12 => Kind::Attach,
            13..=28 => Kind::Acquire,
            29..=48 => Kind::Control,
            49..=66 => Kind::Poll,
            67..=81 => Kind::Feedback,
            82..=93 => Kind::Detach,
            _ => Kind::Drop,
        };
        if random(&mut rng).is_multiple_of(5) {
            step.session = 1 - step.session;
        }
        match step.kind {
            Kind::Attach => {
                step.viewer = index;
                step.target = targets[(random(&mut rng) % 10) as usize];
                slots.push((index, step.session, step.target));
            }
            Kind::Acquire => {
                step.grant = index;
                grants.push(index);
            }
            Kind::Control if !grants.is_empty() => {
                step.grant = grants[(random(&mut rng) % grants.len() as u64) as usize];
            }
            _ => {}
        }
        trace.push(step);
    }
    // Guarantee a live, unDisposed old-generation route at the actual replacement cut.
    trace.push(Step {
        kind: Kind::Drop,
        session: 0,
        ..Step::new(Kind::Drop)
    });
    trace.push(Step {
        kind: Kind::Drop,
        session: 1,
        ..Step::new(Kind::Drop)
    });
    trace.push(Step {
        kind: Kind::Attach,
        session: 0,
        viewer: 122,
        target: targets[9],
        ..Step::new(Kind::Attach)
    });
    trace.push(Step {
        kind: Kind::Acquire,
        session: 0,
        viewer: 122,
        grant: 1123,
        ..Step::new(Kind::Acquire)
    });
    trace.push(Step::new(Kind::ReplaceWorker));
    trace.push(Step {
        kind: Kind::Control,
        session: 0,
        viewer: 122,
        grant: 1123,
        value: 2,
        ..Step::new(Kind::Control)
    });
    trace.push(Step {
        kind: Kind::Poll,
        session: 0,
        viewer: 122,
        ..Step::new(Kind::Poll)
    });
    trace.push(Step {
        kind: Kind::Detach,
        session: 0,
        viewer: 122,
        ..Step::new(Kind::Detach)
    });
    trace
}

#[test]
fn semantic_shrinker_preserves_failure_fingerprint_and_parseable_replay() {
    let mut bad = Step::new(Kind::Control);
    bad.value = 9;
    let trace = vec![
        Step::new(Kind::Attach),
        Step::new(Kind::Poll),
        bad,
        Step::new(Kind::Detach),
    ];
    let failure = ("control", "route".to_owned());
    let classify = |steps: &[Step]| {
        steps
            .iter()
            .any(|step| step.kind == Kind::Control && step.value >= 1)
            .then(|| failure.clone())
    };
    let reduced = shrink(trace.clone(), &failure, classify);
    assert!(reduced.len() < trace.len());
    assert_eq!(reduced.len(), 1);
    assert_eq!(reduced[0].value, 1);
    assert_eq!(decode(&encode(&reduced)).unwrap(), reduced);
    let unchanged = shrink(trace.clone(), &failure, |_| {
        Some(("poll", "unrelated".to_owned()))
    });
    assert_eq!(unchanged, trace);
}

#[test]
fn explicit_detach_compound_worker_cleanup_replays_server_registry_release_afterward() {
    // Minimized oracle discovery: OBCN Detach includes Host Release before Host Detach;
    // subsequent OBCN Release settles the Server grant. Only Session Drop orders the
    // two distinct OBCN messages Release before Detach.
    let trace = decode(
        r#"{"version":2,"steps":[[0,1,1,0,0,1,false],[1,1,1,0,0,1,false],[5,1,1,0,0,1,false]]}"#,
    )
    .unwrap();
    assert!(run(&trace).is_ok());
}
#[test]
fn worker_generation_coverage_requires_a_live_old_route_not_just_an_environment_event() {
    let replacement = Step::new(Kind::ReplaceWorker);
    assert!(
        !run(std::slice::from_ref(&replacement))
            .unwrap()
            .contains("worker-loss")
    );
    let attach = Step::new(Kind::Attach);
    let dispose = Step::new(Kind::Dispose);
    let poll = Step::new(Kind::Poll);
    assert!(
        !run(&[attach.clone(), dispose, replacement.clone(), poll.clone()])
            .unwrap()
            .contains("worker-loss")
    );
    assert!(
        run(&[attach, replacement, poll])
            .unwrap()
            .contains("worker-generation-poll")
    );
}
#[test]
fn sibling_interleaving_requires_distinct_viewers_in_one_continuous_session_window() {
    let attach = Step::new(Kind::Attach);
    let sibling = Step {
        viewer: 1,
        target: 1,
        ..attach.clone()
    };
    let poll = Step::new(Kind::Poll);
    let mut trace = vec![attach.clone(), sibling.clone(), poll.clone(), poll.clone()];
    assert!(!run(&trace).unwrap().contains("interleaved"));
    trace.push(Step {
        viewer: 1,
        ..poll.clone()
    });
    assert!(run(&trace).unwrap().contains("interleaved"));

    let detach = Step {
        viewer: 1,
        ..Step::new(Kind::Detach)
    };
    let replacement = Step {
        viewer: 2,
        target: 2,
        ..attach.clone()
    };
    let trace = vec![
        attach.clone(),
        sibling.clone(),
        poll.clone(),
        detach,
        replacement.clone(),
        Step {
            viewer: 2,
            ..poll.clone()
        },
    ];
    assert!(
        !run(&trace).unwrap().contains("interleaved"),
        "a one-viewer interval ends the earlier window"
    );

    let next_sibling = Step {
        viewer: 3,
        target: 3,
        ..attach.clone()
    };
    let trace = vec![
        attach.clone(),
        sibling.clone(),
        poll.clone(),
        Step::new(Kind::Drop),
        replacement,
        next_sibling,
        Step {
            viewer: 2,
            ..poll.clone()
        },
    ];
    assert!(
        !run(&trace).unwrap().contains("interleaved"),
        "Session Drop ends the earlier window"
    );

    let trace = vec![
        attach.clone(),
        sibling,
        Step {
            session: 1,
            viewer: 2,
            target: 2,
            ..attach.clone()
        },
        Step {
            session: 1,
            viewer: 3,
            target: 3,
            ..attach
        },
        poll.clone(),
        Step {
            session: 1,
            viewer: 2,
            ..poll
        },
    ];
    assert!(
        !run(&trace).unwrap().contains("interleaved"),
        "different Sessions cannot share a window"
    );
}
#[test]
fn stale_grant_coverage_requires_same_target_replacement_with_a_new_grant() {
    let attach = Step::new(Kind::Attach);
    let acquire = Step::new(Kind::Acquire);
    let control = Step {
        viewer: 1,
        ..Step::new(Kind::Control)
    };
    let sibling = Step {
        viewer: 1,
        target: 1,
        ..attach.clone()
    };
    let new_grant = Step {
        viewer: 1,
        grant: 1,
        ..acquire.clone()
    };
    let trace = vec![
        attach.clone(),
        acquire.clone(),
        sibling.clone(),
        new_grant.clone(),
        control.clone(),
    ];
    let coverage = run(&trace).unwrap();
    assert!(coverage.contains("wrong-grant"));
    assert!(
        !coverage.contains("stale-grant"),
        "a live sibling grant is not a retired replacement grant"
    );

    let detach = Step::new(Kind::Detach);
    let trace = vec![
        attach.clone(),
        acquire.clone(),
        detach.clone(),
        sibling,
        new_grant.clone(),
        control.clone(),
    ];
    assert!(
        !run(&trace).unwrap().contains("stale-grant"),
        "a retired grant from another target is not a replacement grant"
    );
    let replacement = Step {
        viewer: 1,
        ..attach.clone()
    };
    let mut trace = vec![attach, acquire, detach, replacement, control.clone()];
    assert!(
        !run(&trace).unwrap().contains("stale-grant"),
        "replacement must have acquired its new grant"
    );
    trace.pop();
    trace.push(new_grant);
    trace.push(control);
    assert!(run(&trace).unwrap().contains("stale-grant"));
}

#[test]
fn coverage_distinguishes_unknown_ids_and_live_same_session_siblings() {
    let attach = Step::new(Kind::Attach);
    let acquire = Step::new(Kind::Acquire);
    let control = Step::new(Kind::Control);
    let poll = Step::new(Kind::Poll);
    assert!(
        !run(std::slice::from_ref(&poll))
            .unwrap()
            .contains("stale-viewer")
    );
    assert!(
        run(&[attach.clone(), Step::new(Kind::Detach), poll.clone()])
            .unwrap()
            .contains("stale-viewer")
    );

    for other_session in 0..2 {
        let sibling = Step {
            session: other_session,
            viewer: 1,
            target: 1,
            ..attach.clone()
        };
        let sibling_acquire = Step {
            session: other_session,
            viewer: 1,
            grant: 1,
            ..acquire.clone()
        };
        let sibling_control = Step {
            session: other_session,
            viewer: 1,
            grant: 1,
            ..control.clone()
        };
        let sibling_poll = Step {
            session: other_session,
            viewer: 1,
            ..poll.clone()
        };
        let sibling_feedback = Step {
            session: other_session,
            viewer: 1,
            ..Step::new(Kind::Feedback)
        };
        let prefix = vec![
            attach.clone(),
            acquire.clone(),
            control.clone(),
            poll.clone(),
            sibling,
            sibling_acquire,
            sibling_poll,
        ];
        let mut live = prefix.clone();
        live.push(sibling_control.clone());
        live.push(sibling_feedback.clone());
        let coverage = run(&live).unwrap();
        assert_eq!(coverage.contains("equal-sequence"), other_session == 0);
        assert_eq!(coverage.contains("equal-frame"), other_session == 0);

        let mut disposed = prefix;
        disposed.push(Step::new(Kind::Dispose));
        disposed.push(sibling_control);
        disposed.push(sibling_feedback);
        disposed.push(Step::new(Kind::Drop));
        let coverage = run(&disposed).unwrap();
        assert!(
            !coverage.contains("equal-sequence"),
            "disposed grant cannot provide live equal-sequence evidence"
        );
        assert!(
            !coverage.contains("equal-frame"),
            "disposed projection cannot provide live equal-frame evidence"
        );
        assert!(
            !coverage.contains("drop-multiple"),
            "one live and one disposed entry are not two live cleanups"
        );
        assert_eq!(coverage.contains("disposed-sibling"), other_session == 0);
    }
}
#[test]
fn alias_and_feedback_coverage_require_the_declared_boundary() {
    let attach = Step::new(Kind::Attach);
    let same_form = Step {
        viewer: 1,
        ..attach.clone()
    };
    assert!(
        !run(&[attach.clone(), same_form.clone()])
            .unwrap()
            .contains("duplicate")
    );
    let alias = Step {
        alternate: true,
        ..same_form
    };
    assert!(run(&[attach.clone(), alias]).unwrap().contains("duplicate"));

    let feedback = Step {
        value: 2,
        ..Step::new(Kind::Feedback)
    };
    assert!(
        !run(&[attach.clone(), feedback.clone()])
            .unwrap()
            .contains("bad-feedback")
    );
    assert!(
        run(&[attach, Step::new(Kind::Poll), feedback])
            .unwrap()
            .contains("bad-feedback")
    );
}
