use super::*;
use orchbee_capability_node_protocol::{HostCreationId, HostEpoch, HostTerminalId};
use orchbee_workbench_protocol::{
    SiteId, TerminalId, WorkbenchPanelId, WorkbenchProtocolDocument, WorkbenchRequestId, WorkerId,
    parse_snapshot, serialize_document,
};
use serde_json::{Value, json};
use std::num::NonZeroU64;
mod explanation;
fn binding(n: u128) -> PresentationBinding {
    PresentationBinding {
        terminal_id: TerminalId::parse(&format!("{n:032x}")).unwrap(),
        worker_id: WorkerId::parse(&format!("{:032x}", 99)).unwrap(),
    }
}
fn physical(n: u128, generation: u64) -> TerminalAgentBinding {
    TerminalAgentBinding {
        binding: binding(n),
        worker_generation: WorkerGeneration::new(NonZeroU64::new(generation).unwrap()),
        identity: HostTerminalIdentity::new(
            HostCreationId::from_bytes([1; 16]).unwrap(),
            HostEpoch::from_bytes([2; 16]).unwrap(),
            HostTerminalId::from_bytes([3; 16]).unwrap(),
        ),
    }
}
fn tree() -> Value {
    json!({"kind":"workbench.snapshot","protocolVersion":1,"revision":"9","siteId":format!("{:032x}",80),"navigation":{"workspaceOrder":[],"projects":[],"directories":[],"workspaces":[],"standaloneTerminalContextId":format!("{:032x}",1)},"terminalContexts":[{"terminalContextId":format!("{:032x}",1),"groups":[],"tabs":[{"tabId":format!("{:032x}",2),"label":"tab","pinned":false,"groupId":null,"root":{"kind":"panel","panelId":format!("{:032x}",3),"primaryPresentation":{"customLabel":null,"presentationId":format!("{:032x}",4),"label":"main","binding":binding(1)},"childPresentations":[{"customLabel":null,"presentationId":format!("{:032x}",5),"label":"child","binding":binding(2)}]}}]}]})
}
fn install(a: &mut WorkbenchAuthority, doc: &Value) {
    let snap = parse_snapshot(&serde_json::to_vec(doc).unwrap()).unwrap();
    a.projection = crate::workbench_projection::WorkbenchProjectionStore::load(
        &serialize_document(&WorkbenchProtocolDocument::Snapshot(snap)).unwrap(),
    )
    .unwrap();
}
fn owner() -> WorkbenchAuthority {
    let mut a = WorkbenchAuthority::new(SiteId::parse(&format!("{:032x}", 80)).unwrap()).unwrap();
    install(&mut a, &tree());
    a
}
fn observation(presentation: u128, after: Option<u64>) -> Action {
    Action::Observe {
        binding: serde_json::from_value(json!({"presentationId":format!("{presentation:032x}"),
            "generation":"01".repeat(16), "hostEpoch":"02".repeat(16), "hostTerminalId":"03".repeat(16)})).unwrap(),
        after_sequence: after.map(WorkbenchRevision::new),
    }
}

#[test]
fn compact_states_cover_primary_child_unavailable_and_replaced_physical_generations() {
    let mut a = owner();
    let facts = [physical(1, 1), physical(2, 1)];
    invoke(&mut a, state("custom", "1"), &facts);
    let action = Action::States {
        presentation_ids: [4, 5]
            .into_iter()
            .map(|id| {
                orchbee_workbench_protocol::TerminalPresentationId::parse(&format!("{id:032x}"))
                    .unwrap()
            })
            .collect(),
    };
    let Outcome::States { presentations, .. } = invoke(&mut a, action.clone(), &facts) else {
        panic!("states")
    };
    assert_eq!(
        presentations[0].snapshot.as_ref().unwrap().agent.as_deref(),
        Some("Team Agent")
    );
    assert!(presentations[1].snapshot.as_ref().unwrap().agent.is_none());
    assert_eq!(
        presentations[0].snapshot.as_ref().unwrap().sequence.value(),
        1
    );
    let Outcome::States { presentations, .. } = invoke(&mut a, action.clone(), &facts[..1]) else {
        panic!("states")
    };
    assert!(presentations[1].snapshot.is_none());
    let mut changed = physical(1, 2);
    changed.identity = HostTerminalIdentity::new(
        HostCreationId::from_bytes([4; 16]).unwrap(),
        HostEpoch::from_bytes([5; 16]).unwrap(),
        HostTerminalId::from_bytes([6; 16]).unwrap(),
    );
    let Outcome::States { presentations, .. } = invoke(&mut a, action, &[changed]) else {
        panic!("states")
    };
    let current = presentations[0].snapshot.as_ref().unwrap();
    assert_eq!(current.binding.generation.as_str(), "04".repeat(16));
    assert_eq!(current.sequence.value(), 0);
    assert!(current.agent.is_none());
    assert_eq!(
        invoke(
            &mut a,
            Action::States {
                presentation_ids: vec![
                    orchbee_workbench_protocol::TerminalPresentationId::parse(&"7".repeat(32))
                        .unwrap()
                ]
            },
            &facts
        ),
        Outcome::Failed {
            reason: Failure::NotFound
        }
    );
}
#[test]
fn physical_observation_is_contiguous_and_isolates_child_membership() {
    let mut a = owner();
    let facts = [physical(1, 1), physical(2, 1)];
    invoke(&mut a, state("custom", "1"), &facts);
    invoke(
        &mut a,
        report(
            json!({"kind":"state","source":"custom","agent":"Team Agent","state":"working","message":"changed","sequence":"2"}),
        ),
        &facts,
    );
    invoke(
        &mut a,
        report(
            json!({"kind":"state","source":"custom","agent":"Team Agent","state":"idle","sequence":"3"}),
        ),
        &facts,
    );
    let Outcome::Observed { observation, .. } = invoke(&mut a, observation(4, Some(0)), &facts)
    else {
        panic!("observation");
    };
    assert_eq!(observation.sequence.value(), 2);
    assert_eq!(
        observation
            .changes
            .iter()
            .map(|c| c.state)
            .collect::<Vec<_>>(),
        [
            orchbee_workbench_protocol::PanelAgentState::Working,
            orchbee_workbench_protocol::PanelAgentState::Idle
        ]
    );
    let Outcome::Observed { observation, .. } =
        invoke(&mut a, self::observation(5, Some(0)), &facts)
    else {
        panic!("child");
    };
    assert_eq!(observation.sequence.value(), 0);
    assert!(observation.view.agent.is_none());
    assert_eq!(
        invoke(&mut a, self::observation(4, Some(3)), &facts),
        Outcome::Failed {
            reason: Failure::InvalidRequest
        }
    );
}
#[test]
fn physical_observation_rejects_replacement_and_does_not_borrow_missing_proof() {
    let mut a = owner();
    let facts = [physical(1, 1)];
    invoke(&mut a, state("custom", "1"), &facts);
    assert_eq!(
        invoke(&mut a, observation(4, None), &[]),
        Outcome::Failed {
            reason: Failure::Unavailable
        }
    );
    let mut replaced = physical(1, 2);
    replaced.identity = HostTerminalIdentity::new(
        HostCreationId::from_bytes([4; 16]).unwrap(),
        replaced.identity.host_epoch(),
        replaced.identity.terminal_id(),
    );
    assert_eq!(
        invoke(&mut a, observation(4, None), &[replaced.clone()]),
        Outcome::Failed {
            reason: Failure::BindingChanged
        }
    );
    let mut action = observation(4, Some(0));
    if let Action::Observe { binding, .. } = &mut action {
        binding.generation =
            orchbee_agent_observation_protocol::AgentAttachmentGeneration::parse(&"04".repeat(16))
                .unwrap();
    }
    let Outcome::Observed { observation, .. } = invoke(&mut a, action, &[replaced]) else {
        panic!("new binding");
    };
    assert_eq!(observation.sequence.value(), 0);
    assert!(observation.view.agent.is_none());
}
fn panel() -> WorkbenchPanelId {
    WorkbenchPanelId::parse(&format!("{:032x}", 3)).unwrap()
}
fn report(value: Value) -> Action {
    Action::Report {
        panel_id: panel(),
        expected_binding: binding(1),
        report: Box::new(serde_json::from_value(value).unwrap()),
    }
}
fn state(source: &str, seq: &str) -> Action {
    report(
        json!({"kind":"state","source":source,"agent":" Team Agent ","state":"working","message":"hello","sequence":seq}),
    )
}
fn invoke(a: &mut WorkbenchAuthority, action: Action, facts: &[TerminalAgentBinding]) -> Outcome {
    let request = PanelAgentRequest {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{:032x}", 7)).unwrap(),
        site_id: a.site_id().clone(),
        action,
    };
    let result = a.panel_agent(request.clone(), facts, Instant::now(), |p| {
        std::path::Path::new(p).is_absolute()
    });
    assert!(result.answers(&request));
    result.outcome
}
#[test]
fn reports_read_clear_and_release_preserve_sequence_and_canonical_navigation() {
    let mut a = owner();
    let facts = [physical(1, 1)];
    let before = a.snapshot().clone();
    assert!(matches!(
        invoke(&mut a, state("custom", "1"), &[]),
        Outcome::Failed {
            reason: Failure::Unavailable
        }
    ));
    assert!(matches!(
        invoke(&mut a, state("custom", "1"), &facts),
        Outcome::Reported { changed: true, .. }
    ));
    assert!(matches!(
        invoke(&mut a, state("custom", "1"), &facts),
        Outcome::Reported {
            changed: false,
            disposition: orchbee_workbench_protocol::PanelAgentDisposition::Ignored,
            ..
        }
    ));
    let Outcome::Read { panels, .. } = invoke(
        &mut a,
        Action::Read {
            panel_ids: vec![panel()],
        },
        &facts,
    ) else {
        panic!("read")
    };
    assert_eq!(
        panels[0].report.as_ref().unwrap().agent.as_deref(),
        Some("Team Agent")
    );
    assert!(matches!(
        invoke(
            &mut a,
            report(json!({"kind":"clear","sequence":"2"})),
            &facts
        ),
        Outcome::Reported { changed: true, .. }
    ));
    assert!(matches!(
        invoke(&mut a, state("custom", "1"), &facts),
        Outcome::Reported { changed: false, .. }
    ));
    invoke(&mut a, state("custom", "3"), &facts);
    let Outcome::Reported { view, .. } = invoke(
        &mut a,
        report(json!({"kind":"release","source":"custom","agent":"Team Agent","sequence":"4"})),
        &facts,
    ) else {
        panic!("release")
    };
    assert!(view.agent.is_none());
    assert_eq!(a.snapshot(), &before);
}
#[test]
fn physical_generation_changes_retire_reports_but_missing_facts_do_not_invent_exit() {
    let mut a = owner();
    invoke(&mut a, state("custom", "9"), &[physical(1, 1)]);
    let read = || Action::Read {
        panel_ids: vec![panel()],
    };
    let Outcome::Read { panels, .. } = invoke(&mut a, read(), &[]) else {
        panic!("read")
    };
    assert!(panels[0].report.as_ref().unwrap().agent.is_some());
    let Outcome::Read { panels, .. } = invoke(&mut a, read(), &[physical(1, 2)]) else {
        panic!("read")
    };
    assert!(panels[0].report.as_ref().unwrap().agent.is_none());
    assert!(matches!(
        invoke(&mut a, state("custom", "1"), &[physical(1, 2)]),
        Outcome::Reported { changed: true, .. }
    ));
    let before = a.panel_agent.revision;
    assert!(matches!(
        invoke(&mut a, read(), &[physical(1, 2), physical(1, 3)]),
        Outcome::Failed {
            reason: Failure::BindingChanged
        }
    ));
    assert_eq!(a.panel_agent.revision, before);
}
#[test]
fn source_and_revision_failures_do_not_consume_the_rejected_report() {
    let mut a = owner();
    let facts = [physical(1, 1)];
    for n in 0..128 {
        assert!(matches!(
            invoke(&mut a, state(&format!("source:{n}"), "1"), &facts),
            Outcome::Reported { .. }
        ));
    }
    let before = serde_json::to_value(&a.panel_agent.resources[&key(&binding(1))].reports).unwrap();
    assert!(matches!(
        invoke(&mut a, state("overflow", "9"), &facts),
        Outcome::Failed {
            reason: Failure::SourceLimit
        }
    ));
    assert_eq!(
        serde_json::to_value(&a.panel_agent.resources[&key(&binding(1))].reports).unwrap(),
        before
    );
    a.panel_agent.revision = u64::MAX;
    assert!(matches!(
        invoke(&mut a, state("source:0", "2"), &facts),
        Outcome::Failed {
            reason: Failure::RevisionExhausted
        }
    ));
    assert_eq!(
        serde_json::to_value(&a.panel_agent.resources[&key(&binding(1))].reports).unwrap(),
        before
    );
}
#[test]
fn session_ignore_can_commit_a_watermark_while_official_release_is_a_complete_noop() {
    let mut a = owner();
    let facts = [physical(1, 1)];
    invoke(
        &mut a,
        report(
            json!({"kind":"session","source":"herdr:codex","agent":"codex","agentSessionId":"provider-id","sequence":"1"}),
        ),
        &facts,
    );
    assert!(matches!(
        invoke(
            &mut a,
            report(
                json!({"kind":"session","source":"herdr:claude","agent":"claude","agentSessionId":"foreign-id","sequence":"9"})
            ),
            &facts
        ),
        Outcome::Reported {
            changed: true,
            disposition: orchbee_workbench_protocol::PanelAgentDisposition::Ignored,
            ..
        }
    ));
    let before = a.panel_agent.revision;
    assert!(matches!(
        invoke(
            &mut a,
            report(
                json!({"kind":"release","source":"herdr:codex","agent":"codex","sequence":"18446744073709551615"})
            ),
            &facts
        ),
        Outcome::Reported { changed: false, .. }
    ));
    assert_eq!(a.panel_agent.revision, before);
}

fn metadata(
    a: &mut WorkbenchAuthority,
    source: &str,
    agent: Option<&str>,
    title: &str,
    sequence: &str,
    now: Instant,
) {
    let value = json!({"protocolVersion":1,"requestId":"a".repeat(32),"siteId":a.site_id(),"action":{"kind":"report","panelId":panel(),"expectedBinding":binding(1),"source":source,"patch":{"agent":agent,"title":title,"tokens":{"progress":"kept"}},"sequence":sequence}});
    let request = serde_json::from_value(value).unwrap();
    assert!(matches!(
        a.panel_metadata(request, now).outcome,
        orchbee_workbench_protocol::PanelMetadataOutcome::Reported { .. }
    ));
}
fn metadata_read(
    a: &mut WorkbenchAuthority,
    now: Instant,
) -> orchbee_workbench_protocol::PanelMetadataRow {
    let request=serde_json::from_value(json!({"protocolVersion":1,"requestId":"a".repeat(32),"siteId":a.site_id(),"action":{"kind":"read","panelIds":[panel()]}})).unwrap();
    let orchbee_workbench_protocol::PanelMetadataOutcome::Read { mut panels, .. } =
        a.panel_metadata(request, now).outcome
    else {
        panic!("metadata read")
    };
    panels.remove(0)
}
#[test]
fn process_exit_commits_metadata_cleanup_and_blocks_late_reports_without_dropping_tokens() {
    use orchbee_agent_observation_protocol::AgentReportProvider as Provider;
    use orchbee_workbench_protocol::MetadataTokenKey;
    let mut a = owner();
    let fact = physical(1, 1);
    let facts = [fact.clone()];
    let now = Instant::now();
    metadata(&mut a, "herdr:codex", None, "official", "9", now);
    metadata(&mut a, "custom-old", Some("codex"), "old", "9", now);
    metadata(
        &mut a,
        "custom-new",
        Some("codex"),
        "new",
        "9",
        now + std::time::Duration::from_millis(20),
    );
    let observation = TerminalAgentProcessObservation {
        observed_at: now + std::time::Duration::from_millis(10),
        agent: Some(Provider::Codex),
        fallback: None,
        exited: true,
    };
    let result = a
        .observe_terminal_agent(
            &fact,
            &facts,
            observation,
            now + std::time::Duration::from_millis(30),
        )
        .unwrap();
    assert!(result.changed);
    let row = metadata_read(&mut a, now + std::time::Duration::from_millis(31));
    assert_eq!(row.presentations.len(), 1);
    assert_eq!(row.presentations[0].source.as_str(), "custom-new");
    assert_eq!(
        row.tokens.as_map()[&MetadataTokenKey::parse("progress").unwrap()].as_str(),
        "kept"
    );
    metadata(
        &mut a,
        "herdr:codex",
        None,
        "late",
        "10",
        now + std::time::Duration::from_millis(32),
    );
    assert_eq!(
        metadata_read(&mut a, now + std::time::Duration::from_millis(33)),
        row
    );
    a.observe_terminal_agent(
        &fact,
        &facts,
        TerminalAgentProcessObservation {
            observed_at: now + std::time::Duration::from_millis(40),
            exited: false,
            ..observation
        },
        now + std::time::Duration::from_millis(40),
    )
    .unwrap();
    metadata(
        &mut a,
        "herdr:codex",
        None,
        "fresh",
        "1",
        now + std::time::Duration::from_millis(41),
    );
    assert_eq!(
        metadata_read(&mut a, now + std::time::Duration::from_millis(42))
            .presentations
            .len(),
        2
    );
}
#[test]
fn failed_process_commit_preserves_metadata_and_report_state_and_clock_keeps_sample_time() {
    use orchbee_agent_observation_protocol::AgentReportProvider as Provider;
    let mut a = owner();
    let fact = physical(1, 1);
    let facts = [fact.clone()];
    let now = Instant::now();
    metadata(&mut a, "herdr:codex", None, "keep", "1", now);
    let before = metadata_read(&mut a, now);
    a.panel_agent.revision = u64::MAX;
    let observation = TerminalAgentProcessObservation {
        observed_at: now,
        agent: Some(Provider::Codex),
        fallback: Some(TerminalAgentScreenObservation {
            state: orchbee_workbench_protocol::PanelAgentState::Unknown,
            visible_blocker: false,
        }),
        exited: true,
    };
    assert_eq!(
        a.observe_terminal_agent(&fact, &facts, observation, now),
        Err(Failure::RevisionExhausted)
    );
    assert_eq!(metadata_read(&mut a, now), before);
    assert!(a.panel_agent.resources.is_empty());
    a.panel_agent.revision = 0;
    let later = now + std::time::Duration::from_millis(30);
    a.observe_terminal_agent(&fact, &facts, observation, later)
        .unwrap();
    let state = serde_json::to_value(&a.panel_agent.resources[&key(&binding(1))].reports).unwrap();
    assert_eq!(state["observed_at"], json!((1_u64 << 63) - 30_000_000));
    assert_eq!(a.panel_agent.time(now).unwrap(), (1_u64 << 63) - 30_000_000);
}

#[test]
fn moving_a_terminal_to_a_child_retains_reports_and_last_membership_removal_prunes_them() {
    let mut a = owner();
    invoke(&mut a, state("custom", "1"), &[physical(1, 1)]);
    let mut doc = tree();
    doc["terminalContexts"][0]["tabs"][0]["root"]["primaryPresentation"]["binding"] =
        json!(binding(2));
    doc["terminalContexts"][0]["tabs"][0]["root"]["childPresentations"][0]["binding"] =
        json!(binding(1));
    install(&mut a, &doc);
    let Outcome::Read { panels, .. } = invoke(
        &mut a,
        Action::Read {
            panel_ids: vec![panel()],
        },
        &[physical(1, 1), physical(2, 1)],
    ) else {
        panic!("read")
    };
    assert!(panels[0].report.as_ref().unwrap().agent.is_none());
    assert!(a.panel_agent.resources.contains_key(&key(&binding(1))));
    assert!(matches!(
        invoke(
            &mut a,
            state("custom", "2"),
            &[physical(1, 1), physical(2, 1)]
        ),
        Outcome::Failed {
            reason: Failure::BindingChanged
        }
    ));
    doc["terminalContexts"][0]["tabs"][0]["root"]["childPresentations"] = json!([]);
    install(&mut a, &doc);
    invoke(
        &mut a,
        Action::Read { panel_ids: vec![] },
        &[physical(2, 1)],
    );
    assert!(a.panel_agent.resources.is_empty());
    assert_eq!(a.panel_agent.bytes, 0);
}
#[test]
fn noops_remain_available_at_full_capacity_and_byte_rejection_preserves_the_candidate_sequence() {
    let mut a = owner();
    let facts = [physical(1, 1)];
    invoke(&mut a, state("custom", "1"), &facts);
    let before = serde_json::to_value(&a.panel_agent.resources[&key(&binding(1))].reports).unwrap();
    let semantic_before = invoke(&mut a, observation(4, Some(0)), &facts);
    a.panel_agent.bytes = MAXIMUM_BYTES;
    assert!(matches!(
        invoke(
            &mut a,
            report(json!({"kind":"release","source":"herdr:codex","agent":"codex","sequence":"9"})),
            &facts
        ),
        Outcome::Reported { changed: false, .. }
    ));
    assert!(matches!(
        invoke(&mut a, state("new-source", "2"), &facts),
        Outcome::Failed {
            reason: Failure::ResourceLimit
        }
    ));
    assert_eq!(
        serde_json::to_value(&a.panel_agent.resources[&key(&binding(1))].reports).unwrap(),
        before
    );
    assert_eq!(
        invoke(&mut a, observation(4, Some(0)), &facts),
        semantic_before
    );
}

#[test]
fn clearing_a_report_retains_semantic_history_until_physical_membership_is_removed() {
    let mut a = owner();
    let facts = [physical(1, 1)];
    invoke(
        &mut a,
        report(json!({"kind":"state","source":"custom","agent":"Agent","state":"working"})),
        &facts,
    );
    assert_eq!(a.panel_agent.resources.len(), 1);
    assert!(matches!(
        invoke(&mut a, report(json!({"kind":"clear"})), &facts),
        Outcome::Reported { changed: true, .. }
    ));
    assert_eq!(a.panel_agent.resources.len(), 1);
    let Outcome::Observed { observation, .. } = invoke(&mut a, observation(4, Some(0)), &facts)
    else {
        panic!("history");
    };
    assert_eq!(observation.changes.len(), 2);
    assert!(observation.view.agent.is_none());
    assert_eq!(
        observation.view.state,
        orchbee_workbench_protocol::PanelAgentState::Unknown
    );
    assert!(a.panel_agent.bytes > 0);
}

#[test]
fn per_binding_explicit_input_stamp_ignores_process_writes_and_other_bindings() {
    let mut owner = owner();
    let fact = physical(1, 1);
    let facts = [fact.clone(), physical(2, 1)];
    let before = owner.terminal_agent_input_revision(&fact);
    assert_eq!(before, 0);
    invoke(&mut owner, state("custom", "1"), &facts);
    let stamp = owner.terminal_agent_input_revision(&fact);
    assert!(stamp > before);
    assert_eq!(owner.terminal_agent_input_revision(&facts[1]), 0);
    assert_eq!(owner.terminal_agent_input_revision(&physical(1, 2)), 0);
    let now = Instant::now();
    owner
        .observe_terminal_agent(
            &fact,
            &facts,
            TerminalAgentProcessObservation {
                observed_at: now,
                agent: Some(orchbee_agent_observation_protocol::AgentReportProvider::Codex),
                fallback: None,
                exited: false,
            },
            now,
        )
        .unwrap();
    assert_eq!(owner.terminal_agent_input_revision(&fact), stamp);
    invoke(&mut owner, state("custom", "1"), &facts);
    assert_eq!(owner.terminal_agent_input_revision(&fact), stamp);
    invoke(&mut owner, state("custom", "2"), &facts);
    assert!(owner.terminal_agent_input_revision(&fact) > stamp);
}

#[test]
fn managed_clear_is_prepared_atomic_exact_and_preserves_independent_reports() {
    let mut a = owner();
    let physical = physical(1, 1);
    let session: orchbee_workbench_protocol::PanelAgentSession = serde_json::from_value(
        json!({"source":"herdr:grok","agent":"grok","reference":{"kind":"id","value":"saved"}}),
    )
    .unwrap();
    a.panel_agent
        .seed_sessions(vec![(physical.clone(), session.clone())], Instant::now())
        .unwrap();
    let revision = a.panel_agent.revision;
    drop(a.prepare_managed_agent_clear(&physical, &session).unwrap());
    assert_eq!(a.terminal_restart_session(&physical), Some(session.clone()));
    assert_eq!(a.panel_agent.revision, revision);
    a.panel_agent.revision = u64::MAX;
    assert!(matches!(
        a.prepare_managed_agent_clear(&physical, &session),
        Err(Failure::RevisionExhausted)
    ));
    assert_eq!(a.terminal_restart_session(&physical), Some(session.clone()));
    a.panel_agent.revision = revision;
    let mut other = session.clone();
    other.reference.value = "later".into();
    a.prepare_managed_agent_clear(&physical, &other)
        .unwrap()
        .commit();
    assert_eq!(a.terminal_restart_session(&physical), Some(session.clone()));
    let mut stale = physical.clone();
    stale.worker_generation = WorkerGeneration::new(NonZeroU64::new(2).unwrap());
    a.prepare_managed_agent_clear(&stale, &session)
        .unwrap()
        .commit();
    assert_eq!(a.terminal_restart_session(&physical), Some(session.clone()));
    a.prepare_managed_agent_clear(&physical, &session)
        .unwrap()
        .commit();
    assert_eq!(a.panel_agent.checkpoint_session(&physical), Some(None));
    assert_eq!(a.panel_agent.revision, revision + 1);
    // A newer persisted provider reference is not owned by the cleared name.
    let mut a = owner();
    a.panel_agent
        .seed_sessions(vec![(physical.clone(), other.clone())], Instant::now())
        .unwrap();
    a.prepare_managed_agent_clear(&physical, &session)
        .unwrap()
        .commit();
    assert_eq!(a.terminal_restart_session(&physical), Some(other));
}

#[test]
fn presentation_reports_read_exact_primary_child_and_unbound_in_requested_order() {
    let mut a = owner();
    let facts = [physical(1, 1), physical(2, 1)];
    invoke(&mut a, state("custom", "1"), &facts);
    let Action::Report { report, .. } = report(
        json!({"kind":"state","source":"custom","agent":"child-agent","state":"blocked","message":"child approval","sequence":"2"}),
    ) else {
        panic!("report")
    };
    let exact = Action::ReportPresentation {
        panel_id: panel(),
        presentation_id: orchbee_workbench_protocol::TerminalPresentationId::parse(&format!(
            "{:032x}",
            5
        ))
        .unwrap(),
        expected_binding: binding(2),
        report,
    };
    assert!(matches!(
        invoke(&mut a, exact.clone(), &facts),
        Outcome::PresentationReported { changed: true, .. }
    ));
    assert!(matches!(
        invoke(&mut a, exact.clone(), &facts),
        Outcome::PresentationReported { changed: false, .. }
    ));
    for wrong_owner in [false, true] {
        let mut rejected = exact.clone();
        if let Action::ReportPresentation {
            panel_id,
            expected_binding,
            ..
        } = &mut rejected
        {
            if wrong_owner {
                *panel_id = WorkbenchPanelId::parse(&format!("{:032x}", 99)).unwrap();
            } else {
                *expected_binding = binding(1);
            }
        }
        assert_eq!(
            invoke(&mut a, rejected, &facts),
            Outcome::Failed {
                reason: if wrong_owner {
                    Failure::NotFound
                } else {
                    Failure::BindingChanged
                }
            }
        );
    }
    let before = a.snapshot().clone();
    let id = |n: u128| {
        orchbee_workbench_protocol::TerminalPresentationId::parse(&format!("{n:032x}")).unwrap()
    };
    let read = |ids: &[u128]| Action::ReadPresentations {
        presentation_ids: ids.iter().copied().map(id).collect(),
    };
    let Outcome::PresentationsRead {
        presentations,
        report_revision,
        ..
    } = invoke(&mut a, read(&[5, 4]), &facts)
    else {
        panic!("read")
    };
    assert_eq!(
        presentations
            .iter()
            .map(|r| r.presentation_id.clone())
            .collect::<Vec<_>>(),
        vec![id(5), id(4)]
    );
    assert!(presentations.iter().all(|r| r.panel.panel_id == panel()));
    assert_eq!(presentations[0].panel.binding, Some(binding(2)));
    assert_eq!(
        presentations[0]
            .panel
            .report
            .as_ref()
            .unwrap()
            .message
            .as_deref(),
        Some("child approval")
    );
    assert_eq!(presentations[1].panel.binding, Some(binding(1)));
    assert_eq!(
        presentations[1]
            .panel
            .report
            .as_ref()
            .unwrap()
            .agent
            .as_deref(),
        Some("Team Agent")
    );
    assert!(
        matches!(invoke(&mut a, read(&[]), &facts), Outcome::PresentationsRead { report_revision: r, presentations, .. } if r == report_revision && presentations.is_empty())
    );
    assert_eq!(
        invoke(&mut a, read(&[4, 99]), &facts),
        Outcome::Failed {
            reason: Failure::NotFound
        }
    );
    assert_eq!(a.snapshot(), &before);
    let mut unbound = tree();
    unbound["terminalContexts"][0]["tabs"][0]["root"]["childPresentations"][0]["binding"] =
        Value::Null;
    install(&mut a, &unbound);
    assert_eq!(
        invoke(&mut a, exact, &facts[..1]),
        Outcome::Failed {
            reason: Failure::Unbound
        }
    );
    let Outcome::PresentationsRead { presentations, .. } =
        invoke(&mut a, read(&[5, 4]), &facts[..1])
    else {
        panic!("unbound")
    };
    assert!(presentations[0].panel.binding.is_none() && presentations[0].panel.report.is_none());
    assert_eq!(
        presentations[1]
            .panel
            .report
            .as_ref()
            .unwrap()
            .agent
            .as_deref(),
        Some("Team Agent")
    );
}
