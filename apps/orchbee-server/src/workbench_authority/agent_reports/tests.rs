//! Decision traces exercise the pure transition before live report admission.
use super::*;
fn author(source: &str, agent: &str) -> Author {
    Author::new(source, agent).unwrap()
}
fn reference(value: &str) -> Option<Reference> {
    Reference::id(value)
}
fn seq(value: u64) -> Option<Sequence> {
    Some(Sequence::new(value))
}
fn state(
    source: &str,
    agent: &str,
    status: Status,
    sequence: Option<Sequence>,
    session: Option<&str>,
) -> Event {
    Event::State {
        author: author(source, agent),
        status,
        message: Some("reported".into()),
        reference: session.and_then(reference),
        sequence,
    }
}
fn session(
    source: &str,
    agent: &str,
    id: Option<&str>,
    sequence: Option<Sequence>,
    start: Option<Start>,
) -> Event {
    Event::Session {
        author: author(source, agent),
        reference: id.and_then(reference),
        sequence,
        start,
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
fn anchored(provider: Provider, id: &str) -> Reports {
    let agent = provider.report_label();
    let source = format!("herdr:{agent}");
    let mut current = Reports::default();
    step(
        &mut current,
        process(Some(provider), Status::Unknown, false),
        1,
    );
    step(
        &mut current,
        session(&source, agent, Some(id), seq(1), Some(Start::Startup)),
        2,
    );
    step(
        &mut current,
        state(&source, agent, Status::Working, seq(2), Some(id)),
        3,
    );
    assert_eq!(current.effective().status, Status::Working);
    current
}

#[test]
fn public_native_routing_and_official_release_never_fabricate_state() {
    for provider in Provider::ALL {
        let agent = provider.report_label();
        let source = if provider == Provider::Antigravity {
            "herdr:antigravity_cli".into()
        } else {
            format!("herdr:{agent}")
        };
        if agent_report_source_policy(&source, agent) == SourcePolicy::Custom {
            continue;
        }
        let mut current = Reports::default();
        step(
            &mut current,
            process(Some(provider), Status::Idle, false),
            1,
        );
        step(
            &mut current,
            state(&source, agent, Status::Blocked, seq(5), Some("provider-id")),
            2,
        );
        if agent_report_reserves_native_state(&source, agent) {
            let effective = current.effective();
            assert_eq!(effective.status, Status::Idle);
            assert!(effective.authority_source.is_none());
            assert_eq!(effective.session.unwrap().reference.value(), "provider-id");
        } else if agent_report_source_policy(&source, agent) == SourcePolicy::IdentityOnly {
            assert!(current.authority.is_none());
            assert!(current.persisted.is_none());
            assert!(current.sequences.is_empty());
        }
        let before = current.clone();
        let released = step(
            &mut current,
            Event::Release {
                author: author(&source, agent),
                sequence: seq(u64::MAX),
            },
            3,
        );
        assert_eq!(released.disposition, Disposition::Ignored);
        assert!(!released.changed);
        assert!(!released.released);
        assert_eq!(current, before);
    }
}

#[test]
fn strict_source_watermarks_survive_clear_and_reject_unsequenced_replays() {
    let mut current = Reports::default();
    step(
        &mut current,
        state("", "Custom Agent", Status::Idle, seq(10), None),
        1,
    );
    for sequence in [None, seq(9), seq(10)] {
        let refused = step(
            &mut current,
            state("", "Custom Agent", Status::Working, sequence, None),
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
            state("", "Custom Agent", Status::Working, seq(10), None),
            6
        )
        .changed
    );
    assert!(
        step(
            &mut current,
            state("", "Custom Agent", Status::Working, seq(12), None),
            7
        )
        .changed
    );
    assert_eq!(current.effective().status, Status::Working);
    assert_eq!(current.effective().authority_source.as_deref(), Some(""));
}

#[test]
fn custom_release_preserves_live_process_and_foreign_persisted_reference() {
    let mut current = Reports::default();
    step(
        &mut current,
        process(Some(Provider::Codex), Status::Idle, false),
        1,
    );
    step(
        &mut current,
        state("custom", "codex", Status::Working, seq(1), None),
        2,
    );
    step(
        &mut current,
        session("herdr:codex", "codex", Some("session"), seq(1), None),
        3,
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
    assert_eq!(effective.session.unwrap().author.source, "herdr:codex");
    let only_identity = current.clone();
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
        current, only_identity,
        "persisted-only identity is not authority to clear"
    );
    let mut custom = Reports::default();
    step(
        &mut custom,
        state("custom", "Unrecognised", Status::Working, None, None),
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
fn ignored_session_reports_can_consume_sequence_before_later_guards() {
    let mut current = Reports::default();
    step(
        &mut current,
        process(Some(Provider::Codex), Status::Idle, false),
        1,
    );
    let ignored = step(
        &mut current,
        session("herdr:claude", "claude", Some("s"), seq(7), None),
        2,
    );
    assert_eq!(ignored.disposition, Disposition::Ignored);
    assert!(ignored.changed);
    assert_eq!(current.sequences["herdr:claude"].get(), 7);
    assert!(current.persisted.is_none());
    step(
        &mut current,
        process(Some(Provider::Claude), Status::Idle, false),
        3,
    );
    assert!(
        !step(
            &mut current,
            session("herdr:claude", "claude", Some("s"), seq(7), None),
            4
        )
        .changed
    );
    assert!(
        step(
            &mut current,
            session("herdr:claude", "claude", Some("s"), seq(8), None),
            5
        )
        .changed
    );
    let before = current.clone();
    assert!(
        !step(
            &mut current,
            session(
                "herdr:claude",
                "claude",
                None,
                seq(100),
                Some(Start::Resume)
            ),
            6
        )
        .changed
    );
    assert_eq!(current, before, "missing reference consumes no sequence");
}

#[test]
fn full_lifecycle_reports_wait_for_reference_and_matching_process_confirmation() {
    let mut current = Reports::default();
    let pending = step(
        &mut current,
        state("herdr:pi", "pi", Status::Working, seq(11), Some("old")),
        1,
    );
    assert_eq!(pending.disposition, Disposition::Deferred);
    assert!(current.effective().authority_source.is_none());
    assert_eq!(
        step(
            &mut current,
            session("herdr:pi", "pi", Some("old"), seq(10), Some(Start::Startup)),
            2
        )
        .disposition,
        Disposition::Deferred
    );
    step(
        &mut current,
        process(Some(Provider::Pi), Status::Unknown, false),
        3,
    );
    assert_eq!(
        current.effective().authority_source.as_deref(),
        Some("herdr:pi")
    );
    assert_eq!(
        current.effective().session.unwrap().reference.value(),
        "old"
    );
    assert_eq!(current.effective().status, Status::Working);
    assert!(
        !step(&mut current, process(None, Status::Idle, false), 4).changed,
        "uncertain detection cannot replace live lifecycle authority"
    );
    let exited = step(
        &mut current,
        process(Some(Provider::Pi), Status::Unknown, true),
        5,
    );
    assert_eq!(
        exited.metadata_exit,
        Some(MetadataExit {
            agent: Provider::Pi,
            observed_at: 5
        })
    );
    assert!(exited.released);
    assert_eq!(current.effective().agent, None);
    step(
        &mut current,
        state("herdr:pi", "pi", Status::Working, seq(50), Some("old")),
        6,
    );
    step(
        &mut current,
        process(Some(Provider::Pi), Status::Unknown, false),
        7,
    );
    assert!(
        current.authority.is_none(),
        "fresh process alone cannot revive the exited Session"
    );
    step(
        &mut current,
        session("herdr:pi", "pi", Some("new"), seq(20), Some(Start::New)),
        8,
    );
    assert_eq!(
        current.effective().session.unwrap().reference.value(),
        "new"
    );
    assert!(current.authority.is_none());
    step(
        &mut current,
        state("herdr:pi", "pi", Status::Working, seq(21), Some("new")),
        9,
    );
    assert_eq!(current.effective().status, Status::Working);
    assert!(
        !step(
            &mut current,
            state("herdr:pi", "pi", Status::Blocked, seq(100), Some("old")),
            10
        )
        .changed
    );
}

#[test]
fn clear_suppresses_same_session_and_reanchors_only_a_different_reference() {
    let mut current = anchored(Provider::Pi, "old");
    assert!(
        step(
            &mut current,
            Event::Clear {
                source: None,
                sequence: seq(3)
            },
            4
        )
        .changed
    );
    assert!(
        !step(
            &mut current,
            state("herdr:pi", "pi", Status::Blocked, seq(4), Some("old")),
            5
        )
        .changed
    );
    assert!(
        step(
            &mut current,
            state("herdr:pi", "pi", Status::Idle, seq(0), Some("new")),
            6
        )
        .changed
    );
    assert_eq!(
        current.effective().session.unwrap().reference.value(),
        "new"
    );
    assert!(
        !step(
            &mut current,
            state(
                "herdr:pi",
                "pi",
                Status::Blocked,
                seq(u64::MAX),
                Some("old")
            ),
            7
        )
        .changed
    );
}

#[test]
fn identity_replacement_and_lifecycle_branch_follow_different_rules() {
    let mut current = Reports::default();
    assert!(
        !step(
            &mut current,
            session("herdr:pi", "pi", Some("id"), seq(1), Some(Start::Branch)),
            1
        )
        .changed
    );
    step(
        &mut current,
        session(
            "herdr:qwen",
            "qwen",
            Some("old"),
            seq(1),
            Some(Start::Startup),
        ),
        2,
    );
    let refused = step(
        &mut current,
        session(
            "herdr:qwen",
            "qwen",
            Some("new"),
            seq(2),
            Some(Start::Branch),
        ),
        3,
    );
    assert_eq!(refused.disposition, Disposition::Ignored);
    assert!(refused.changed);
    assert_eq!(
        current.effective().session.unwrap().reference.value(),
        "old"
    );
    step(
        &mut current,
        process(Some(Provider::Qwen), Status::Idle, false),
        4,
    );
    step(
        &mut current,
        session(
            "herdr:qwen",
            "qwen",
            Some("new"),
            seq(3),
            Some(Start::Branch),
        ),
        5,
    );
    assert_eq!(
        current.effective().session.unwrap().reference.value(),
        "new"
    );
    assert!(current.authority.is_none());
}

#[test]
fn opencode_unsequenced_selection_has_an_explicit_process_gate() {
    let mut current = anchored(Provider::OpenCode, "old");
    step(
        &mut current,
        session(
            "herdr:opencode",
            "opencode",
            Some("selected"),
            None,
            Some(Start::Select),
        ),
        4,
    );
    assert_eq!(
        current.effective().session.unwrap().reference.value(),
        "selected"
    );
    assert!(current.authority.is_none());
    assert_eq!(current.sequences["herdr:opencode"].get(), 2);
    assert!(
        !step(
            &mut current,
            state(
                "herdr:opencode",
                "opencode",
                Status::Blocked,
                seq(99),
                Some("old")
            ),
            5
        )
        .changed
    );
    let mut absent = Reports::default();
    assert_eq!(
        step(
            &mut absent,
            session(
                "herdr:opencode",
                "opencode",
                Some("selected"),
                None,
                Some(Start::Select)
            ),
            1
        )
        .disposition,
        Disposition::Deferred
    );
    assert!(absent.effective().session.is_none());
    step(
        &mut absent,
        process(Some(Provider::OpenCode), Status::Unknown, false),
        2,
    );
    assert_eq!(
        absent.effective().session.unwrap().reference.value(),
        "selected"
    );
}

#[test]
fn foreign_owner_takeover_requires_the_specific_provider_and_start_evidence() {
    let mut current = Reports::default();
    step(
        &mut current,
        session("herdr:codex", "codex", Some("old"), seq(1), None),
        1,
    );
    step(
        &mut current,
        process(Some(Provider::Qwen), Status::Idle, false),
        2,
    );
    assert_eq!(
        step(
            &mut current,
            session(
                "herdr:qwen",
                "qwen",
                Some("new"),
                seq(1),
                Some(Start::Branch)
            ),
            3
        )
        .disposition,
        Disposition::Ignored
    );
    assert_eq!(current.current_session().unwrap().author.agent, "codex");
    step(
        &mut current,
        session(
            "herdr:qwen",
            "qwen",
            Some("new"),
            seq(2),
            Some(Start::Startup),
        ),
        4,
    );
    assert_eq!(current.current_session().unwrap().author.agent, "qwen");
    step(
        &mut current,
        process(Some(Provider::Grok), Status::Idle, false),
        5,
    );
    assert_eq!(
        step(
            &mut current,
            session(
                "herdr:grok",
                "grok",
                Some("grok-id"),
                seq(3),
                Some(Start::New)
            ),
            6
        )
        .disposition,
        Disposition::Ignored
    );
    assert_eq!(current.current_session().unwrap().author.agent, "qwen");
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
        state("custom", "codex", Status::Working, None, None),
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
        if next.effective().session.is_some()
            || next.detected.is_some()
            || !next.suppressed.is_empty()
            || !next.stale.is_empty()
        {
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
                0 | 1 => state(source, "Model Agent", status, sequence, Some("untrusted")),
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
fn delayed_process_clear_uses_current_owner_time_for_future_suppression() {
    let mut current = anchored(Provider::Pi, "old");
    let changed = step(
        &mut current,
        Event::Process(Process {
            observed_at: 4,
            agent: Some(Provider::Codex),
            fallback: Some(Fallback {
                status: Status::Idle,
                visible_blocker: false,
            }),
            exited: false,
        }),
        10,
    );
    assert!(changed.changed);
    assert_eq!(current.suppressed["herdr:pi"].at, 10);
    let old = step(
        &mut current,
        Event::Process(Process {
            observed_at: 8,
            agent: Some(Provider::Pi),
            fallback: Some(Fallback {
                status: Status::Unknown,
                visible_blocker: false,
            }),
            exited: false,
        }),
        11,
    );
    assert_eq!(old.disposition, Disposition::Ignored);
    assert!(!old.changed);
    assert_eq!(current.detected, Some(Provider::Codex));
}

#[test]
fn stale_session_history_limit_refuses_the_candidate_without_forgetting_old_sessions() {
    let mut current = anchored(Provider::Pi, "initial");
    for index in 1..=256 {
        let id = format!("session-{index}");
        step(
            &mut current,
            session(
                "herdr:pi",
                "pi",
                Some(&id),
                seq(index * 2 + 1),
                Some(Start::New),
            ),
            index * 2 + 100,
        );
        step(
            &mut current,
            state(
                "herdr:pi",
                "pi",
                Status::Working,
                seq(index * 2 + 2),
                Some(&id),
            ),
            index * 2 + 101,
        );
    }
    assert!(current.admitted_bytes().is_ok());
    let next = current.transition(
        session(
            "herdr:pi",
            "pi",
            Some("overflow"),
            seq(999),
            Some(Start::New),
        ),
        1000,
    );
    assert_eq!(
        next.next.admitted_bytes(),
        Err(orchbee_workbench_protocol::PanelAgentFailure::HistoryLimit)
    );
    assert_eq!(current.stale["herdr:pi"].len(), 256);
    assert!(current.admitted_bytes().is_ok());
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
        state("custom", "codex", Status::Working, seq(1), None),
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
fn managed_seed_clear_preserves_independent_authority_and_is_effect_free_until_committed() {
    let session: orchbee_workbench_protocol::PanelAgentSession = serde_json::from_str(
        r#"{"source":"herdr:grok","agent":"grok","reference":{"kind":"id","value":"saved"}}"#,
    )
    .unwrap();
    let mut reports = Reports::from_restart_session(&session).unwrap();
    reports.authority = Some(Authority {
        author: author("custom", "grok"),
        status: Status::Working,
        message: None,
        at: 1,
        reference: reference("new"),
    });
    let before = reports.clone();
    let next = reports.clear_owned_restart_session(&session).unwrap();
    assert_eq!(reports, before);
    assert!(next.persisted.is_none());
    assert_eq!(next.authority, before.authority);
    assert_eq!(next.view().session.unwrap().reference.value, "new");
}
