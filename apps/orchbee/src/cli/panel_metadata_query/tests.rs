use super::*;
fn id(n: u64) -> String {
    format!("{n:032x}")
}
fn snapshot() -> Snapshot {
    serde_json::from_value(json!({"protocolVersion":1,"siteId":id(2),"revision":"12","navigation":{"projects":[], "standaloneTerminalContextId":id(3), "directories":[], "workspaces":[]},"terminalContexts":[{"terminalContextId":id(3),"groups":[],"tabs":[{"tabId":id(4),"label":"tab","pinned":false,"groupId":null,"root":{"kind":"panel","panelId":id(5),"primaryPresentation":{"customLabel":null,"presentationId":id(6),"label":"canonical","binding":{"workerId":id(7),"terminalId":id(8)}},"childPresentations":[]}}]}]})).unwrap()
}
fn row() -> PanelMetadataRow {
    serde_json::from_value(json!({"panelId":id(5),"binding":{"workerId":id(7),"terminalId":id(8)},"tokens":{"branch":"main"},"presentations":[
 {"source":"plain","agent":null,"appliesToSource":null,"title":{"value":"标题","order":"1"},"displayAgent":null,"stateLabels":{"working":{"value":"忙","order":"1"}}},
 {"source":"guarded","agent":"codex","appliesToSource":"hook-a","title":{"value":"guarded title","order":"2"},"displayAgent":{"value":"guarded display","order":"2"},"stateLabels":{}}
 ]})).unwrap()
}
#[test]
fn metadata_projection_preserves_sources_without_inventing_authority_and_fences_whole_batch() {
    let state = snapshot();
    let panel = WorkbenchPanelId::parse(&id(5)).unwrap();
    let mut doc = Document {
        reports: Some(reports::fixture(
            &state,
            [WorkbenchPanelId::parse(&id(5)).unwrap()],
        )),
        rows: BTreeMap::new(),
        encoded_row_bytes: 0,
        revision: WorkbenchRevision::new(2),
        request_ids: vec![],
        site: state.site_id,
        workbench_revision: state.revision,
    };
    doc.admit(
        WorkbenchRequestId::parse(&id(10)).unwrap(),
        WorkbenchRevision::new(2),
        vec![row()],
    )
    .unwrap();
    doc.admit(
        WorkbenchRequestId::parse(&id(11)).unwrap(),
        WorkbenchRevision::new(2),
        vec![],
    )
    .unwrap();
    let value = doc.value(&panel);
    assert_eq!(
        value["presentation"],
        json!({"title":"标题","displayAgent":null,"stateLabels":{"working":"忙"}})
    );
    assert_eq!(
        value["context"],
        json!({"agent":null,"authoritySource":null})
    );
    assert_eq!(value["presentations"].as_array().unwrap().len(), 2);
    assert_eq!(value["tokens"]["branch"], "main");
    assert_eq!(value["source"]["requestIds"], json!([id(10), id(11)]));
    let error = doc
        .admit(
            WorkbenchRequestId::parse(&id(12)).unwrap(),
            WorkbenchRevision::new(3),
            vec![],
        )
        .unwrap_err();
    assert_eq!(error.kind, FailureKind::Rejected);
    assert_eq!(doc.value(&panel), value);
    assert_eq!(
        doc.admit(
            WorkbenchRequestId::parse(&id(13)).unwrap(),
            WorkbenchRevision::new(2),
            vec![row()]
        )
        .unwrap_err()
        .kind,
        FailureKind::ProtocolInvalid
    );
    assert_eq!(doc.value(&panel), value);
}
#[test]
fn metadata_observation_rejects_reused_binding_and_changed_canonical_snapshot() {
    let state = snapshot();
    let mut other = row();
    other.binding.as_mut().unwrap().terminal_id =
        orchbee_workbench_protocol::TerminalId::parse(&id(9)).unwrap();
    assert_eq!(
        observe(
            &state,
            PanelMetadataOutcome::Read {
                workbench_revision: state.revision,
                metadata_revision: WorkbenchRevision::new(2),
                panels: vec![other]
            }
        )
        .unwrap_err()
        .kind,
        FailureKind::ProtocolInvalid
    );
    assert_eq!(
        observe(
            &state,
            PanelMetadataOutcome::Read {
                workbench_revision: WorkbenchRevision::new(13),
                metadata_revision: WorkbenchRevision::new(2),
                panels: vec![row()]
            }
        )
        .unwrap_err()
        .kind,
        FailureKind::Rejected
    );
}

#[test]
fn metadata_batch_admission_bounds_aggregate_rows_without_partial_state() {
    let state = snapshot();
    let row = row();
    let size = serde_json::to_vec(&row).unwrap().len();
    let mut doc = Document {
        reports: Some(reports::fixture(
            &state,
            [WorkbenchPanelId::parse(&id(5)).unwrap()],
        )),
        rows: BTreeMap::new(),
        encoded_row_bytes: super::super::output::LIMIT - size,
        revision: WorkbenchRevision::new(2),
        request_ids: vec![],
        site: state.site_id,
        workbench_revision: state.revision,
    };
    doc.admit(
        WorkbenchRequestId::parse(&id(10)).unwrap(),
        WorkbenchRevision::new(2),
        vec![row.clone()],
    )
    .unwrap();
    let before = doc.value(&row.panel_id);
    let mut next = row.clone();
    next.panel_id = WorkbenchPanelId::parse(&id(99)).unwrap();
    assert_eq!(
        doc.admit(
            WorkbenchRequestId::parse(&id(11)).unwrap(),
            WorkbenchRevision::new(2),
            vec![next]
        )
        .unwrap_err()
        .kind,
        FailureKind::OutputTooLarge
    );
    assert_eq!(doc.rows.len(), 1);
    assert_eq!(doc.value(&row.panel_id), before);
    doc.admit(
        WorkbenchRequestId::parse(&id(12)).unwrap(),
        WorkbenchRevision::new(2),
        vec![],
    )
    .unwrap();
}

#[test]
fn actual_report_context_selects_guarded_metadata_without_rewriting_canonical_fields() {
    let state = snapshot();
    let panel = WorkbenchPanelId::parse(&id(5)).unwrap();
    let mut doc = Document {
        reports: None,
        rows: BTreeMap::new(),
        encoded_row_bytes: 0,
        revision: WorkbenchRevision::new(2),
        request_ids: vec![],
        site: state.site_id.clone(),
        workbench_revision: state.revision,
    };
    doc.admit(
        WorkbenchRequestId::parse(&id(10)).unwrap(),
        doc.revision,
        vec![row()],
    )
    .unwrap();
    for (agent, authority, title) in [
        ("codex", "hook-a", "guarded title"),
        ("codex", "different", "标题"),
        ("custom", "hook-a", "标题"),
    ] {
        doc.reports = Some(reports::fixture(&state, [panel.clone()]).with_view(
            orchbee_workbench_protocol::PanelAgentView {
                agent: Some(agent.into()),
                state: orchbee_workbench_protocol::PanelAgentState::Blocked,
                authority_source: Some(authority.into()),
                message: Some("中文\n\u{1b}[31m".into()),
                session: None,
            },
        ));
        let metadata = doc.value(&panel);
        assert_eq!(
            metadata["context"],
            json!({"agent":agent,"authoritySource":authority})
        );
        assert_eq!(metadata["presentation"]["title"], title);
        assert_eq!(metadata["presentations"].as_array().unwrap().len(), 2);
        let mut result = json!({"panel":{"panelId":panel,"agentStatus":"canonical","displayLabel":"canonical label"}});
        doc.enrich(&mut result);
        assert_eq!(result["panel"]["agentStatus"], "canonical");
        assert_eq!(result["panel"]["displayLabel"], "canonical label");
        assert_eq!(result["panel"]["reportedAgent"]["view"]["state"], "blocked");
        assert_eq!(
            result["panel"]["reportedAgent"]["source"]["reportRevision"],
            "0"
        );
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let rendered = crate::cli::output::render_text(|out| doc.render(out, locale)).unwrap();
            assert!(!rendered.contains(&0x1b));
        }
    }
}
