//! Decision traces exercise the pure transition before live report admission.
use super::*;
fn author(source: &str, agent: &str) -> Author {
    Author::new(source, agent).unwrap()
}
fn seq(value: u64) -> Option<Sequence> {
    Some(Sequence::new(value))
}
fn state(source: &str, agent: &str, status: Status, sequence: Option<Sequence>) -> Event {
    Event::State {
        author: author(source, agent),
        status,
        message: Some("reported".into()),
        sequence,
    }
}
fn process(agent: Option<Provider>, status: Status, exited: bool) -> Event {
    Event::Process(Process {
        observed_at: u64::MAX,
        agent,
        fallback: Some(Fallback {
            status,
            visible_blocker: false,
        }),
        exited,
    })
}
fn step(current: &mut Reports, mut event: Event, at: u64) -> Transition {
    if let Event::Process(process) = &mut event
        && process.observed_at == u64::MAX
    {
        process.observed_at = at;
    }
    let before = current.clone();
    let transition = current.transition(event, at);
    assert_eq!(*current, before, "preparation never changes its input");
    assert_eq!(transition.changed, transition.next != before);
    *current = transition.next.clone();
    transition
}

#[test]
fn strict_source_watermarks_survive_clear_and_reject_unsequenced_replays() {
    let mut current = Reports::default();
    step(
        &mut current,
        state("", "Custom Agent", Status::Idle, seq(10)),
        1,
    );
    for sequence in [None, seq(9), seq(10)] {
        let refused = step(
            &mut current,
            state("", "Custom Agent", Status::Working, sequence),
            2,
        );
        assert_eq!(refused.disposition, Disposition::Ignored);
        assert!(!refused.changed);
    }
    assert!(
        !step(
            &mut current,
            Event::Clear {
                source: Some("foreign".into()),
                sequence: seq(99)
            },
            3
        )
        .changed
    );
    assert!(
        !step(
            &mut current,
            Event::Clear {
                source: None,
                sequence: None
            },
            4
        )
        .changed
    );
    assert!(
        step(
            &mut current,
            Event::Clear {
                source: None,
                sequence: seq(11)
            },
            5
        )
        .changed
    );
    assert_eq!(current.effective().agent, None);
    assert_eq!(current.sequences[""].get(), 11);
    assert!(
        !step(
            &mut current,
            state("", "Custom Agent", Status::Working, seq(10)),
            6
        )
        .changed
    );
    assert!(
        step(
            &mut current,
            state("", "Custom Agent", Status::Working, seq(12)),
            7
        )
        .changed
    );
    assert_eq!(current.effective().status, Status::Working);
    assert_eq!(current.effective().authority_source.as_deref(), Some(""));
}

#[test]
fn custom_release_preserves_live_process() {
    let mut current = Reports::default();
    step(
        &mut current,
        process(Some(Provider::Codex), Status::Idle, false),
        1,
    );
    step(
        &mut current,
        state("custom", "codex", Status::Working, seq(1)),
        2,
    );
    assert!(
        !step(
            &mut current,
            Event::Release {
                author: author("foreign", "codex"),
                sequence: seq(2)
            },
            4
        )
        .changed
    );
    let released = step(
        &mut current,
        Event::Release {
            author: author("custom", "codex"),
            sequence: seq(2),
        },
        5,
    );
    assert_eq!(released.disposition, Disposition::Applied);
    assert!(!released.released);
    let effective = current.effective();
    assert_eq!(effective.agent.as_deref(), Some("codex"));
    assert_eq!(effective.status, Status::Idle);
    assert!(effective.authority_source.is_none());
    let process_only = current.clone();
    assert!(
        !step(
            &mut current,
            Event::Clear {
                source: None,
                sequence: seq(50)
            },
            6
        )
        .changed
    );
    assert_eq!(
        current, process_only,
        "process-only presence is not authority to clear"
    );
    let mut custom = Reports::default();
    step(
        &mut custom,
        state("custom", "Unrecognised", Status::Working, None),
        1,
    );
    assert!(
        step(
            &mut custom,
            Event::Release {
                author: author("custom", "Unrecognised"),
                sequence: None
            },
            2
        )
        .released
    );
    assert_eq!(custom.effective().agent, None);
    assert_eq!(custom.effective().status, Status::Unknown);
}

#[test]
fn delayed_exit_preserves_newer_custom_authority_and_newer_blocker_remains_visible() {
    let mut current = Reports::default();
    step(
        &mut current,
        process(Some(Provider::Codex), Status::Idle, false),
        10,
    );
    step(
        &mut current,
        state("custom", "codex", Status::Working, None),
        30,
    );
    let delayed = step(
        &mut current,
        Event::Process(Process {
            observed_at: 20,
            agent: Some(Provider::Codex),
            fallback: Some(Fallback {
                status: Status::Unknown,
                visible_blocker: false,
            }),
            exited: true,
        }),
        35,
    );
    assert!(!delayed.released);
    assert_eq!(current.effective().status, Status::Working);
    assert_eq!(
        delayed.metadata_exit,
        Some(MetadataExit {
            agent: Provider::Codex,
            observed_at: 20
        })
    );
    step(
        &mut current,
        Event::Process(Process {
            observed_at: u64::MAX,
            agent: Some(Provider::Codex),
            fallback: Some(Fallback {
                status: Status::Blocked,
                visible_blocker: true,
            }),
            exited: false,
        }),
        40,
    );
    assert_eq!(current.effective().status, Status::Blocked);
    assert_eq!(
        current.effective().authority_source.as_deref(),
        Some("custom")
    );
    assert_eq!(current.authority.as_ref().unwrap().status, Status::Working);
}

fn custom_trace_failure(trace: &[(u64, Event)]) -> Option<&'static str> {
    let mut current = Reports::default();
    for (at, event) in trace {
        let before = current.clone();
        let transition = current.transition(event.clone(), *at);
        if current != before || transition.changed != (transition.next != before) {
            return Some("preparation changed its input or misreported its candidate");
        }
        let next = transition.next;
        for (source, sequence) in &before.sequences {
            if next
                .sequences
                .get(source)
                .is_none_or(|next| next < sequence)
            {
                return Some("custom source watermark regressed without a process exit");
            }
        }
        if let Event::State {
            author, sequence, ..
        } = event
            && before
                .sequences
                .get(&author.source)
                .is_some_and(|last| sequence.is_none_or(|sequence| sequence <= *last))
            && next != before
        {
            return Some("stale custom report changed state");
        }
        if let Event::Clear {
            source: Some(source),
            ..
        } = event
            && before
                .authority
                .as_ref()
                .is_none_or(|a| &a.author.source != source)
            && next != before
        {
            return Some("clear changed a foreign authority");
        }
        if next.detected.is_some() {
            return Some("custom-only trace manufactured provider or process facts");
        }
        current = next;
    }
    None
}

#[test]
fn generated_custom_traces_preserve_watermarks_without_inventing_process_facts() {
    fn random(seed: &mut u64) -> u64 {
        *seed ^= *seed << 13;
        *seed ^= *seed >> 7;
        *seed ^= *seed << 17;
        *seed
    }
    for seed in 1_u64..=256 {
        let mut generator = seed;
        let mut trace = Vec::new();
        for at in 0_u64..64 {
            let source =
                ["custom-a", "custom-b", "custom-c"][(random(&mut generator) % 3) as usize];
            let sequence = match random(&mut generator) % 8 {
                0 => None,
                1 => seq(0),
                2 if at > 56 => seq(u64::MAX),
                _ => seq(random(&mut generator) % (at + 8)),
            };
            let status = [
                Status::Idle,
                Status::Working,
                Status::Blocked,
                Status::Unknown,
            ][(random(&mut generator) % 4) as usize];
            let event = match random(&mut generator) % 5 {
                0 | 1 => state(source, "Model Agent", status, sequence),
                2 => Event::Clear {
                    source: Some(source.into()),
                    sequence,
                },
                3 => Event::Clear {
                    source: None,
                    sequence,
                },
                _ => Event::Release {
                    author: author(source, "Model Agent"),
                    sequence,
                },
            };
            trace.push((at, event));
        }
        if custom_trace_failure(&trace).is_some() {
            // Preserve original ticks while reducing the sequence; the printed trace is replayable.
            let mut index = 0;
            while index < trace.len() {
                let mut reduced = trace.clone();
                reduced.remove(index);
                if custom_trace_failure(&reduced).is_some() {
                    trace = reduced;
                    index = 0;
                } else {
                    index += 1;
                }
            }
            panic!(
                "seed {seed}: {}; minimized trace {}",
                custom_trace_failure(&trace).unwrap(),
                serde_json::to_string(&trace).unwrap()
            );
        }
    }
}

#[test]
fn presence_preserves_screen_age_and_replacement_clears_it() {
    let mut current = Reports::default();
    let observation = |agent, fallback, exited, at| {
        Event::Process(Process {
            observed_at: at,
            agent: Some(agent),
            fallback,
            exited,
        })
    };
    let blocker = Some(Fallback {
        status: Status::Blocked,
        visible_blocker: true,
    });
    step(
        &mut current,
        observation(Provider::Codex, blocker, false, 10),
        10,
    );
    step(
        &mut current,
        state("custom", "codex", Status::Working, seq(1)),
        20,
    );
    let present = step(
        &mut current,
        observation(Provider::Codex, None, false, 30),
        30,
    );
    assert!(!present.changed);
    assert_eq!(current.effective().status, Status::Working);
    assert_eq!(current.observed_at, Some(10));
    step(
        &mut current,
        observation(Provider::Codex, blocker, false, 40),
        40,
    );
    assert_eq!(current.effective().status, Status::Blocked);
    step(&mut current, observation(Provider::Pi, None, false, 50), 50);
    assert_eq!(current.effective().status, Status::Unknown);
    assert!(!current.visible_blocker);
    assert_eq!(current.observed_at, None);
    step(
        &mut current,
        observation(Provider::Pi, blocker, false, 60),
        60,
    );
    step(&mut current, observation(Provider::Pi, None, true, 70), 70);
    assert_eq!(current.fallback, Status::Unknown);
    assert_eq!(current.observed_at, None);
    step(&mut current, observation(Provider::Pi, None, false, 80), 80);
    assert_eq!(current.effective().status, Status::Unknown);
}

#[test]
fn input_during_work_preserves_done_until_input_after_completion() {
    for transient_unknown in [false, true] {
        let mut current = Reports::default();
        let agent = Provider::lookup("codex").unwrap();
        step(
            &mut current,
            process(Some(agent), Status::Working, false),
            1,
        );
        assert!(
            !current.acknowledge_input(),
            "typing during Working does not consume completion"
        );
        step(
            &mut current,
            process(Some(agent), Status::Working, false),
            2,
        );
        if transient_unknown {
            step(
                &mut current,
                process(Some(agent), Status::Unknown, false),
                3,
            );
        }
        step(&mut current, process(Some(agent), Status::Idle, false), 4);
        assert_eq!(current.view().display_state(), Status::Done);
        // Further observations and merely viewing the terminal leave the Server fact intact.
        step(&mut current, process(Some(agent), Status::Idle, false), 5);
        assert_eq!(current.view().display_state(), Status::Done);
        assert!(current.acknowledge_input());
        assert_eq!(current.view().display_state(), Status::Idle);
        assert!(!current.acknowledge_input());
        step(
            &mut current,
            process(Some(agent), Status::Working, false),
            6,
        );
        step(&mut current, process(Some(agent), Status::Idle, false), 7);
        assert_eq!(current.view().display_state(), Status::Done);
    }
}

#[test]
fn error_episode_keeps_errored_turn_red_until_a_new_turn() {
    use Status::{Blocked, Done, Error, Idle, Unknown, Working};
    use orchbee_workbench_protocol::AgentCompletion as Completion;
    let report = |status, n| state("custom", "codex", status, seq(n));
    let observed = |status| process(Some(Provider::Codex), status, false);
    let cases = vec![
        (
            "working, error, idle",
            vec![
                (report(Working, 1), Working, Completion::None),
                (report(Error, 2), Error, Completion::None),
                (report(Idle, 3), Error, Completion::None),
            ],
        ),
        (
            "working, error, done",
            vec![
                (report(Working, 1), Working, Completion::None),
                (report(Error, 2), Error, Completion::None),
                (report(Done, 3), Error, Completion::None),
            ],
        ),
        (
            "working, error, working, idle",
            vec![
                (report(Working, 1), Working, Completion::None),
                (report(Error, 2), Error, Completion::None),
                (report(Working, 3), Working, Completion::None),
                (report(Idle, 4), Idle, Completion::AwaitingInput),
            ],
        ),
        (
            "working, idle",
            vec![
                (report(Working, 1), Working, Completion::None),
                (report(Idle, 2), Idle, Completion::AwaitingInput),
            ],
        ),
        (
            "error, idle without working",
            vec![
                (report(Error, 1), Error, Completion::None),
                (report(Idle, 2), Error, Completion::None),
            ],
        ),
        (
            "working, error, blocked",
            vec![
                (report(Working, 1), Working, Completion::None),
                (report(Error, 2), Error, Completion::None),
                (report(Blocked, 3), Blocked, Completion::None),
            ],
        ),
        (
            "working, error, idle, process exit, next agent idle",
            vec![
                (report(Working, 1), Working, Completion::None),
                (report(Error, 2), Error, Completion::None),
                (report(Idle, 3), Error, Completion::None),
                (
                    process(Some(Provider::Codex), Unknown, true),
                    Unknown,
                    Completion::None,
                ),
                (
                    process(Some(Provider::Pi), Idle, false),
                    Idle,
                    Completion::None,
                ),
            ],
        ),
        (
            "working, error and idle via fallback",
            vec![
                (observed(Working), Working, Completion::None),
                (observed(Error), Error, Completion::None),
                (observed(Idle), Error, Completion::None),
            ],
        ),
        (
            "fallback working hidden by an explicit error ends the episode",
            vec![
                (report(Working, 1), Working, Completion::None),
                (report(Error, 2), Error, Completion::None),
                (observed(Working), Error, Completion::None),
                (observed(Idle), Error, Completion::None),
                (
                    Event::Clear {
                        source: None,
                        sequence: seq(3),
                    },
                    Idle,
                    Completion::AwaitingInput,
                ),
            ],
        ),
        (
            "error cleared while a later idle was hidden",
            vec![
                (report(Working, 1), Working, Completion::None),
                (report(Error, 2), Error, Completion::None),
                (observed(Idle), Error, Completion::None),
                (
                    Event::Clear {
                        source: None,
                        sequence: seq(3),
                    },
                    Idle,
                    Completion::None,
                ),
            ],
        ),
        (
            "error then another agent idle",
            vec![
                (report(Working, 1), Working, Completion::None),
                (report(Error, 2), Error, Completion::None),
                (state("custom", "pi", Idle, seq(3)), Idle, Completion::None),
            ],
        ),
    ];
    for (name, steps) in cases {
        let mut current = Reports::default();
        for (at, (event, status, completion)) in (1_u64..).zip(steps) {
            step(&mut current, event, at);
            let view = current.view();
            assert_eq!(
                (view.state, view.completion),
                (status, completion),
                "{name}, tick {at}"
            );
        }
    }
}
