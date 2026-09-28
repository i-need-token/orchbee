use super::*;
use orchbee_workbench_protocol::{
    MetadataState, MetadataTokenPatch, SiteId, TerminalId, WorkbenchProtocolDocument,
    WorkbenchRequestId, WorkerId, parse_snapshot, serialize_document,
};
use serde_json::{Value, json};
fn binding(n: u128) -> PresentationBinding {
    PresentationBinding {
        terminal_id: TerminalId::parse(&format!("{n:032x}")).unwrap(),
        worker_id: WorkerId::parse(&format!("{:032x}", 99)).unwrap(),
    }
}
fn source(n: u32) -> MetadataSource {
    MetadataSource::parse(&format!("source:{n}")).unwrap()
}
fn text(s: &str) -> MetadataTokenValue {
    MetadataTokenValue::parse(s).unwrap()
}
fn title(s: &str) -> Patch {
    Patch {
        title: Some(text(s)),
        ..Patch::default()
    }
}
fn panel(n: u128) -> WorkbenchPanelId {
    WorkbenchPanelId::parse(&format!("{n:032x}")).unwrap()
}
fn token(k: &str, v: Option<&str>) -> Patch {
    Patch {
        tokens: Some(
            MetadataTokenPatch::new(BTreeMap::from([(
                MetadataTokenKey::parse(k).unwrap(),
                v.map(text),
            )]))
            .unwrap(),
        ),
        ..Patch::default()
    }
}
fn applied(
    r: &mut PanelMetadata,
    s: u32,
    p: Patch,
    seq: Option<u64>,
    ttl: Option<u32>,
    now: Instant,
) {
    assert_eq!(
        r.report(
            &key(&binding(1)),
            &source(s),
            &p,
            seq.map(WorkbenchRevision::new),
            ttl,
            now
        ),
        Ok(Disposition::Applied)
    );
    assert_eq!(
        r.bytes,
        r.resources.values().map(Resource::bytes).sum::<usize>()
    );
}
fn row(r: &PanelMetadata) -> PanelMetadataRow {
    r.row(panel(1), Some(binding(1)))
}
fn effective(r: &PanelMetadata) -> orchbee_workbench_protocol::PanelMetadataEffective {
    row(r).presentation(None, None)
}
#[test]
fn source_replacement_and_selective_clear_preserve_field_precedence_and_guards() {
    let now = Instant::now();
    let mut r = PanelMetadata::default();
    applied(
        &mut r,
        1,
        Patch {
            title: Some(text("first")),
            display_agent: Some(text("provider")),
            state_labels: BTreeMap::from([(MetadataState::Working, text("busy"))]),
            ..Patch::default()
        },
        None,
        None,
        now,
    );
    applied(&mut r, 2, title("second"), None, None, now);
    applied(
        &mut r,
        1,
        Patch {
            clear_display_agent: true,
            ..Patch::default()
        },
        None,
        None,
        now,
    );
    assert_eq!(effective(&r).title, Some(text("second")));
    assert_eq!(effective(&r).display_agent, None);
    assert_eq!(
        effective(&r).state_labels[&MetadataState::Working],
        text("busy")
    );
    applied(
        &mut r,
        2,
        Patch {
            clear_title: true,
            ..Patch::default()
        },
        None,
        None,
        now,
    );
    assert_eq!(effective(&r).title, Some(text("first")));
    // Set without clear replaces the source, including fields and optional guards.
    applied(
        &mut r,
        1,
        Patch {
            display_agent: Some(text("new")),
            agent: Some(text("codex")),
            applies_to_source: Some(source(3)),
            ..Patch::default()
        },
        None,
        None,
        now,
    );
    assert_eq!(effective(&r), Default::default());
    assert_eq!(
        row(&r).presentation(Some("codex"), None),
        Default::default()
    );
    assert_eq!(
        row(&r).presentation(Some("wrong"), Some("source:3")),
        Default::default()
    );
    let selected = row(&r).presentation(Some("codex"), Some("source:3"));
    assert_eq!(selected.display_agent, Some(text("new")));
    assert!(selected.title.is_none() && selected.state_labels.is_empty());
    applied(&mut r, 1, title("unguarded"), None, None, now);
    assert_eq!(effective(&r).title, Some(text("unguarded")));
}
#[test]
fn expiry_keeps_sequence_fences_and_token_deadlines_independent() {
    let now = Instant::now();
    let mut r = PanelMetadata::default();
    let live = BTreeSet::from([key(&binding(1))]);
    let mut report = title("short");
    report.tokens = token("k", Some("value")).tokens;
    applied(&mut r, 1, report.clone(), Some(10), Some(100), now);
    // Clear-only preserves this source's expiry, while another token has its own deadline.
    applied(
        &mut r,
        1,
        Patch {
            clear_display_agent: true,
            ..Patch::default()
        },
        Some(11),
        None,
        now + Duration::from_millis(20),
    );
    applied(
        &mut r,
        2,
        token("other", Some("long")),
        None,
        Some(200),
        now,
    );
    r.maintain(&live, now + Duration::from_millis(100)).unwrap();
    assert!(effective(&r).title.is_none());
    assert_eq!(row(&r).tokens.as_map().len(), 1);
    let before = r.revision;
    assert_eq!(
        r.report(
            &key(&binding(1)),
            &source(1),
            &report,
            Some(WorkbenchRevision::new(11)),
            Some(1000),
            now + Duration::from_millis(100)
        ),
        Ok(Disposition::Stale)
    );
    assert_eq!(r.revision, before);
    assert!(effective(&r).title.is_none());
    applied(
        &mut r,
        1,
        title("unsequenced"),
        None,
        None,
        now + Duration::from_millis(101),
    );
    assert_eq!(
        r.resources[&key(&binding(1))].sources[&source(1)].sequence,
        Some(WorkbenchRevision::new(11))
    );
    r.maintain(&live, now + Duration::from_secs(3)).unwrap();
    assert_eq!(effective(&r).title, Some(text("unsequenced")));
    assert!(row(&r).tokens.as_map().is_empty());
    // An explicit TTL on clear refreshes retained fields; setting no TTL makes them permanent.
    applied(
        &mut r,
        1,
        Patch {
            clear_display_agent: true,
            ..Patch::default()
        },
        Some(12),
        Some(100),
        now + Duration::from_secs(4),
    );
    r.maintain(
        &live,
        now + Duration::from_secs(4) + Duration::from_millis(99),
    )
    .unwrap();
    assert!(effective(&r).title.is_some());
    r.maintain(
        &live,
        now + Duration::from_secs(4) + Duration::from_millis(100),
    )
    .unwrap();
    assert!(effective(&r).title.is_none());
}
#[test]
fn budget_failures_never_commit_partial_fields_or_consume_a_sequence() {
    let now = Instant::now();
    let mut r = PanelMetadata::default();
    for i in 0..32 {
        applied(
            &mut r,
            0,
            token(&format!("k{i}"), Some("v")),
            None,
            None,
            now,
        );
    }
    let mut report = title("must not appear");
    report.tokens = token("overflow", Some("v")).tokens;
    let before = r.resources.clone();
    let revision = r.revision;
    let bytes = r.bytes;
    assert_eq!(
        r.report(
            &key(&binding(1)),
            &source(9),
            &report,
            Some(WorkbenchRevision::new(12)),
            None,
            now
        ),
        Err(Failure::TokenLimit)
    );
    assert_eq!(r.resources, before);
    assert_eq!((r.revision, r.bytes), (revision, bytes));
    for i in 0..32 {
        applied(&mut r, i, title("v"), Some(1), None, now);
    }
    let before = r.resources.clone();
    let revision = r.revision;
    assert_eq!(
        r.report(
            &key(&binding(1)),
            &source(33),
            &title("overflow"),
            Some(WorkbenchRevision::new(1)),
            None,
            now
        ),
        Err(Failure::SourceLimit)
    );
    assert_eq!(r.resources, before);
    assert_eq!(r.revision, revision);
    r.revision = u64::MAX;
    assert_eq!(
        r.report(
            &key(&binding(1)),
            &source(0),
            &title("blocked"),
            Some(WorkbenchRevision::new(2)),
            None,
            now
        ),
        Err(Failure::RevisionExhausted)
    );
    assert_eq!(r.resources, before);
    assert_eq!(
        r.report(
            &key(&binding(1)),
            &source(0),
            &title("old"),
            Some(WorkbenchRevision::new(1)),
            None,
            now
        ),
        Ok(Disposition::Stale)
    );
    assert_eq!(
        r.maintain(&BTreeSet::new(), now),
        Err(Failure::RevisionExhausted)
    );
    assert_eq!(r.resources, before);
}
#[test]
fn terminal_pruning_releases_payload_and_sequence_with_atomic_capacity_checks() {
    let now = Instant::now();
    let mut r = PanelMetadata::default();
    applied(&mut r, 1, title("保留 😀"), Some(1), None, now);
    let saved = r.resources[&key(&binding(1))].clone();
    r.bytes = MAXIMUM_BYTES;
    let before = r.resources.clone();
    assert_eq!(
        r.report(&key(&binding(2)), &source(2), &title("v"), None, None, now),
        Err(Failure::ResourceLimit)
    );
    assert_eq!(r.resources, before);
    r.bytes = saved.bytes();
    for i in 2..=MAXIMUM_RESOURCES {
        r.resources.insert(key(&binding(i as u128)), saved.clone());
    }
    r.bytes = r.resources.values().map(Resource::bytes).sum();
    assert_eq!(
        r.report(
            &key(&binding(5000)),
            &source(1),
            &title("v"),
            None,
            None,
            now
        ),
        Err(Failure::ResourceLimit)
    );
    r.maintain(&BTreeSet::from([key(&binding(1))]), now)
        .unwrap();
    assert_eq!(r.resources.len(), 1);
    assert_eq!(r.bytes, saved.bytes());
    r.maintain(&BTreeSet::new(), now).unwrap();
    assert!(r.resources.is_empty());
    assert_eq!(r.bytes, 0);
}
#[test]
fn state_labels_merge_per_field_and_clear_falls_back_without_reordering_other_fields() {
    let now = Instant::now();
    let mut r = PanelMetadata::default();
    applied(
        &mut r,
        1,
        Patch {
            state_labels: BTreeMap::from([
                (MetadataState::Working, text("one")),
                (MetadataState::Done, text("done")),
            ]),
            ..Patch::default()
        },
        None,
        None,
        now,
    );
    applied(
        &mut r,
        2,
        Patch {
            state_labels: BTreeMap::from([(MetadataState::Working, text("two"))]),
            ..Patch::default()
        },
        None,
        None,
        now,
    );
    applied(
        &mut r,
        1,
        Patch {
            clear_title: true,
            display_agent: Some(text("agent")),
            ..Patch::default()
        },
        None,
        None,
        now,
    );
    assert_eq!(
        effective(&r).state_labels[&MetadataState::Working],
        text("two")
    );
    assert_eq!(
        effective(&r).state_labels[&MetadataState::Done],
        text("done")
    );
    applied(
        &mut r,
        2,
        Patch {
            clear_state_labels: true,
            ..Patch::default()
        },
        None,
        None,
        now,
    );
    assert_eq!(
        effective(&r).state_labels[&MetadataState::Working],
        text("one")
    );
    assert_eq!(effective(&r).display_agent, Some(text("agent")));
}
fn tree() -> Value {
    json!({"kind":"workbench.snapshot","protocolVersion":1,"revision":"9","siteId":format!("{:032x}",80),"navigation":{"workspaceOrder":[],"projects":[],"standaloneTerminalContextId":format!("{:032x}",1)},"terminalContexts":[{"terminalContextId":format!("{:032x}",1),"groups":[],"tabs":[{"tabId":format!("{:032x}",2),"label":"tab","pinned":false,"groupId":null,"root":{"kind":"panel","panelId":format!("{:032x}",3),"primaryPresentation":{"presentationId":format!("{:032x}",4),"label":"main","binding":binding(1)},"childPresentations":[{"presentationId":format!("{:032x}",5),"label":"child","binding":binding(2)}]}}]}]})
}
fn install(authority: &mut WorkbenchAuthority, doc: &Value) {
    let snapshot = parse_snapshot(&serde_json::to_vec(doc).unwrap()).unwrap();
    authority.projection = crate::workbench_projection::WorkbenchProjectionStore::load(
        &serialize_document(&WorkbenchProtocolDocument::Snapshot(snapshot)).unwrap(),
    )
    .unwrap();
}
fn invoke(authority: &mut WorkbenchAuthority, action: Action) -> PanelMetadataResult {
    let request = PanelMetadataRequest {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{:032x}", 7)).unwrap(),
        site_id: authority.site_id().clone(),
        action,
    };
    let result = authority.panel_metadata(request.clone(), Instant::now());
    assert!(result.answers(&request));
    result.validate().unwrap();
    result
}
#[test]
fn authority_resolves_primary_fences_swaps_and_keeps_metadata_while_child_remains() {
    let mut authority =
        WorkbenchAuthority::new(SiteId::parse(&format!("{:032x}", 80)).unwrap()).unwrap();
    let mut doc = tree();
    install(&mut authority, &doc);
    let report = |b| Action::Report {
        panel_id: panel(3),
        expected_binding: binding(b),
        source: source(1),
        patch: Box::new(title("terminal one")),
        sequence: Some(WorkbenchRevision::new(1)),
        ttl_milliseconds: None,
    };
    let before = authority.snapshot().clone();
    assert!(matches!(
        invoke(&mut authority, report(2)).outcome,
        Outcome::Failed {
            reason: Failure::BindingChanged
        }
    ));
    assert!(authority.panel_metadata.resources.is_empty());
    assert!(matches!(
        invoke(&mut authority, report(1)).outcome,
        Outcome::Reported { .. }
    ));
    assert_eq!(authority.snapshot(), &before);
    // A primary/child swap keeps both logical terminals live, but changes the resolved target.
    doc["terminalContexts"][0]["tabs"][0]["root"]["primaryPresentation"]["binding"] =
        json!(binding(2));
    doc["terminalContexts"][0]["tabs"][0]["root"]["childPresentations"][0]["binding"] =
        json!(binding(1));
    doc["revision"] = json!("10");
    install(&mut authority, &doc);
    assert!(matches!(
        invoke(&mut authority, report(1)).outcome,
        Outcome::Failed {
            reason: Failure::BindingChanged
        }
    ));
    let Outcome::Read { panels, .. } = invoke(
        &mut authority,
        Action::Read {
            panel_ids: vec![panel(3)],
        },
    )
    .outcome
    else {
        panic!()
    };
    assert!(panels[0].presentations.is_empty());
    assert_eq!(authority.panel_metadata.resources.len(), 1);
    doc["terminalContexts"][0]["tabs"][0]["root"]["primaryPresentation"]["binding"] =
        json!(binding(1));
    doc["terminalContexts"][0]["tabs"][0]["root"]["childPresentations"] = json!([]);
    doc["revision"] = json!("11");
    install(&mut authority, &doc);
    let Outcome::Read { panels, .. } = invoke(
        &mut authority,
        Action::Read {
            panel_ids: vec![panel(3)],
        },
    )
    .outcome
    else {
        panic!()
    };
    assert_eq!(
        panels[0].presentation(None, None).title,
        Some(text("terminal one"))
    );
    doc["terminalContexts"][0]["tabs"][0]["root"]["primaryPresentation"]["binding"] = Value::Null;
    doc["revision"] = json!("12");
    install(&mut authority, &doc);
    let Outcome::Read { panels, .. } = invoke(
        &mut authority,
        Action::Read {
            panel_ids: vec![panel(3)],
        },
    )
    .outcome
    else {
        panic!()
    };
    assert!(panels[0].binding.is_none());
    assert!(authority.panel_metadata.resources.is_empty());
    assert!(matches!(
        invoke(&mut authority, report(1)).outcome,
        Outcome::Failed {
            reason: Failure::Unbound
        }
    ));
    assert!(matches!(
        invoke(
            &mut authority,
            Action::Read {
                panel_ids: vec![panel(999)]
            }
        )
        .outcome,
        Outcome::Failed {
            reason: Failure::NotFound
        }
    ));
}

#[test]
fn metadata_revision_failure_prevents_the_companion_agent_exit_commit() {
    use crate::{TerminalAgentBinding, TerminalAgentProcessObservation};
    use orchbee_agent_observation_protocol::AgentReportProvider;
    use orchbee_capability_node_protocol::{
        HostCreationId, HostEpoch, HostTerminalId, HostTerminalIdentity, WorkerGeneration,
    };
    use orchbee_workbench_protocol::{PanelAgentAction, PanelAgentRequest};
    let mut authority =
        WorkbenchAuthority::new(SiteId::parse(&format!("{:032x}", 80)).unwrap()).unwrap();
    install(&mut authority, &tree());
    invoke(
        &mut authority,
        Action::Report {
            panel_id: panel(3),
            expected_binding: binding(1),
            source: MetadataSource::parse("herdr:codex").unwrap(),
            patch: Box::new(title("keep")),
            sequence: Some(WorkbenchRevision::new(1)),
            ttl_milliseconds: None,
        },
    );
    let before = authority.panel_metadata.resources.clone();
    authority.panel_metadata.revision = u64::MAX;
    let fact = TerminalAgentBinding {
        binding: binding(1),
        worker_generation: WorkerGeneration::new(std::num::NonZeroU64::MIN),
        identity: HostTerminalIdentity::new(
            HostCreationId::from_bytes([1; 16]).unwrap(),
            HostEpoch::from_bytes([2; 16]).unwrap(),
            HostTerminalId::from_bytes([3; 16]).unwrap(),
        ),
    };
    let now = Instant::now();
    assert_eq!(
        authority.observe_terminal_agent(
            &fact,
            std::slice::from_ref(&fact),
            TerminalAgentProcessObservation {
                observed_at: now,
                agent: Some(AgentReportProvider::Codex),
                fallback: None,
                exited: true
            },
            now
        ),
        Err(orchbee_workbench_protocol::PanelAgentFailure::RevisionExhausted)
    );
    assert_eq!(authority.panel_metadata.resources, before);
    let result = authority.panel_agent(
        PanelAgentRequest {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(&"a".repeat(32)).unwrap(),
            site_id: authority.site_id().clone(),
            action: PanelAgentAction::Read {
                panel_ids: vec![panel(3)],
            },
        },
        &[fact],
        now,
        |p| std::path::Path::new(p).is_absolute(),
    );
    let orchbee_workbench_protocol::PanelAgentOutcome::Read {
        report_revision,
        panels,
        ..
    } = result.outcome
    else {
        panic!("read")
    };
    assert_eq!(report_revision, WorkbenchRevision::new(0));
    assert_eq!(panels[0].report, Some(Default::default()));
}

#[test]
fn presentation_metadata_reads_exact_binding_without_primary_fallback() {
    let mut a = WorkbenchAuthority::new(SiteId::parse(&format!("{:032x}", 80)).unwrap()).unwrap();
    install(&mut a, &tree());
    let report = |n: u128, value: &str| Action::ReportPresentation {
        panel_id: panel(3),
        presentation_id: orchbee_workbench_protocol::TerminalPresentationId::parse(&format!(
            "{:032x}",
            n + 3
        ))
        .unwrap(),
        expected_binding: binding(n),
        source: source(1),
        patch: Box::new(title(value)),
        sequence: Some(WorkbenchRevision::new(1)),
        ttl_milliseconds: None,
    };
    for (n, value) in [(1, "primary title"), (2, "child title")] {
        assert!(matches!(
            invoke(&mut a, report(n, value)).outcome,
            Outcome::PresentationReported {
                disposition: Disposition::Applied,
                ..
            }
        ));
        assert!(matches!(
            invoke(&mut a, report(n, "stale title")).outcome,
            Outcome::PresentationReported {
                disposition: Disposition::Stale,
                ..
            }
        ));
    }
    for wrong_owner in [false, true] {
        let mut rejected = report(2, "wrong");
        if let Action::ReportPresentation {
            panel_id,
            expected_binding,
            ..
        } = &mut rejected
        {
            if wrong_owner {
                *panel_id = panel(99);
            } else {
                *expected_binding = binding(1);
            }
        }
        assert_eq!(
            invoke(&mut a, rejected).outcome,
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
    let read = |ids: &[u128]| Action::ReadPresentations {
        presentation_ids: ids
            .iter()
            .map(|n| {
                orchbee_workbench_protocol::TerminalPresentationId::parse(&format!("{n:032x}"))
                    .unwrap()
            })
            .collect(),
    };
    let Outcome::PresentationsRead {
        presentations,
        metadata_revision,
        ..
    } = invoke(&mut a, read(&[5, 4])).outcome
    else {
        panic!("read")
    };
    for (row, n, value) in [
        (&presentations[0], 2, "child title"),
        (&presentations[1], 1, "primary title"),
    ] {
        assert_eq!(row.panel.panel_id, panel(3));
        assert_eq!(row.panel.binding, Some(binding(n)));
        assert_eq!(row.panel.presentation(None, None).title, Some(text(value)));
    }
    assert!(
        matches!(invoke(&mut a, read(&[])).outcome, Outcome::PresentationsRead { metadata_revision: r, presentations, .. } if r == metadata_revision && presentations.is_empty())
    );
    assert_eq!(
        invoke(&mut a, read(&[4, 99])).outcome,
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
        invoke(&mut a, report(2, "unbound")).outcome,
        Outcome::Failed {
            reason: Failure::Unbound
        }
    );
    let Outcome::PresentationsRead { presentations, .. } = invoke(&mut a, read(&[5, 4])).outcome
    else {
        panic!("unbound")
    };
    assert!(presentations[0].panel.binding.is_none());
    assert!(
        presentations[0].panel.presentations.is_empty()
            && presentations[0].panel.tokens.as_map().is_empty()
    );
    assert_eq!(
        presentations[1].panel.presentation(None, None).title,
        Some(text("primary title"))
    );
}
