use super::*;
use orchbee_capability_node_protocol::{HostCreationId, HostEpoch, HostTerminalId};
use orchbee_workbench_protocol::{
    SiteId, TerminalId, WorkbenchPanelId, WorkbenchProtocolDocument, WorkbenchRequestId, WorkerId,
    parse_snapshot, serialize_document,
};
use serde_json::{Value, json};
use std::num::NonZeroU64;
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
    json!({"kind":"workbench.snapshot","protocolVersion":1,"revision":"9","siteId":format!("{:032x}",80),"navigation":{"workspaceOrder":[],"projects":[],"standaloneTerminalContextId":format!("{:032x}",1)},"terminalContexts":[{"terminalContextId":format!("{:032x}",1),"groups":[],"tabs":[{"tabId":format!("{:032x}",2),"label":"tab","pinned":false,"groupId":null,"root":{"kind":"panel","panelId":format!("{:032x}",3),"primaryPresentation":{"presentationId":format!("{:032x}",4),"label":"main","binding":binding(1)},"childPresentations":[{"presentationId":format!("{:032x}",5),"label":"child","binding":binding(2)}]}}]}]})
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
}

#[test]
fn clearing_an_unsequenced_custom_report_reclaims_the_empty_resource() {
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
    assert!(a.panel_agent.resources.is_empty());
    assert_eq!(a.panel_agent.bytes, 0);
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
