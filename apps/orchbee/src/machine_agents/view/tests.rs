use super::*;
use crate::machines::ProfileId;
fn row() -> Row {
    Row {
        seen: true,
        order: crate::machine_agents::PlacementOrder::default(),
        target: crate::machine_agents::tests::target(),
        label: "terminal".into(),
        workspace: "project / workspace".into(),
        tab_label: None,
        terminal_title: orchbee_workbench_protocol::TerminalTitleObservation::Unavailable {},
        state: PanelAgentState::Working,
        sequence: WorkbenchRevision::new(1),
        metadata: PanelMetadataEffective::default(),
        tokens: MetadataTokens::default(),
    }
}

#[test]
fn source_view_uses_canonical_agent_exact_member_and_raw_multikey_ties() {
    let remote = Choice::Remote(ProfileId::parse(&format!("{:032x}", 7)).unwrap());
    let mut first = row();
    first.metadata.display_agent = Some(MetadataTokenValue::parse("friendly").unwrap());
    let mut second = first.clone();
    second.state = PanelAgentState::Blocked;
    second.target.member = TerminalPresentationId::parse(&format!("{:032x}", 24)).unwrap();
    second.target.physical = crate::machine_agents::tests::physical(24);
    let candidates = vec![
        (Choice::Local, first.clone()),
        (remote.clone(), second.clone()),
    ];
    let spec=parse_agent_view_spec(br#"{"source":"x","filter":{"op":"eq","field":"agent","value":"codex"},"sort":[{"field":"seen"}]}"#).unwrap();
    let view = View {
        order: Order::Priority,
        ..View::default()
    };
    let entries = || {
        candidates.iter().map(|(machine, row)| Entry {
            machine,
            label: "machine",
            row,
        })
    };
    let result = view.project_source(
        entries(),
        &Choice::Local,
        Some(&first.target.context),
        None,
        Some(&spec),
    );
    assert_eq!(result.len(), 2);
    assert_eq!(
        result[0].machine,
        &Choice::Local,
        "explicit sort ties retain placement, not preset priority"
    );
    let spec=parse_agent_view_spec(&serde_json::to_vec(&serde_json::json!({"source":"x","filter":{"op":"eq","field":"pane_id","value":first.target.member.as_str()}})).unwrap()).unwrap();
    assert_eq!(
        view.project_source(entries(), &Choice::Local, None, None, Some(&spec))
            .len(),
        1
    );
    let spec=parse_agent_view_spec(br#"{"source":"x","filter":{"op":"eq","field":"tab_id","value":{"context":"current_tab_id"}}}"#).unwrap();
    let current = crate::terminal_session::NativeMemberTarget {
        tab: first.target.tab.clone(),
        panel: first.target.panel.clone(),
        presentation: first.target.member.clone(),
    };
    let result = view.project_source(
        entries(),
        &remote,
        Some(&first.target.context),
        Some(&current),
        Some(&spec),
    );
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].machine, &remote);
}
fn project<'a>(
    view: &View,
    rows: &'a [(Choice, Row)],
    current: &Choice,
    context: Option<&NativeContextTarget>,
) -> Vec<Entry<'a>> {
    view.project(
        rows.iter().map(|(machine, row)| Entry {
            machine,
            label: "machine",
            row,
        }),
        current,
        context,
    )
}
#[test]
fn view_scope_qualifies_equal_site_context_and_member_ids_by_machine() {
    let remote = Choice::Remote(ProfileId::parse(&format!("{:032x}", 7)).unwrap());
    let rows = vec![(Choice::Local, row()), (remote.clone(), row())];
    let mut view = View {
        scope: Scope::Context,
        ..View::default()
    };
    let result = project(
        &view,
        &rows,
        &Choice::Local,
        Some(&rows[0].1.target.context),
    );
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].machine, &Choice::Local);
    assert!(project(&view, &rows, &remote, None).is_empty());
    view.scope = Scope::Machine;
    assert_eq!(project(&view, &rows, &remote, None)[0].machine, &remote);
    view.scope = Scope::All;
    assert_eq!(project(&view, &rows, &remote, None).len(), 2);
}
#[test]
fn view_priority_and_state_sequence_are_stable_and_display_metadata_cannot_retarget() {
    let mut rows = Vec::new();
    for (index, state) in [
        PanelAgentState::Idle,
        PanelAgentState::Working,
        PanelAgentState::Blocked,
        PanelAgentState::Unknown,
        PanelAgentState::Blocked,
    ]
    .into_iter()
    .enumerate()
    {
        let mut row = row();
        row.state = state;
        row.sequence = WorkbenchRevision::new(index as u64);
        row.label = format!("{index}");
        row.metadata.title = Some(MetadataTokenValue::parse(&format!("Title {index}")).unwrap());
        rows.push((Choice::Local, row));
    }
    let view = View {
        order: Order::Priority,
        ..View::default()
    };
    let result = project(&view, &rows, &Choice::Local, None);
    assert_eq!(
        result
            .iter()
            .map(|e| e.row.label.as_str())
            .collect::<Vec<_>>(),
        ["2", "4", "1", "0", "3"]
    );
    let target = result[0].row.target.clone();
    assert_eq!(target, crate::machine_agents::tests::target());
    let view = View {
        order: Order::Sequence,
        ..View::default()
    };
    assert_eq!(
        project(&view, &rows, &Choice::Local, None)
            .iter()
            .map(|e| e.row.label.as_str())
            .collect::<Vec<_>>(),
        ["4", "3", "2", "1", "0"]
    );
    let view = View {
        state: Some(MetadataState::Blocked),
        ..View::default()
    };
    assert_eq!(project(&view, &rows, &Choice::Local, None).len(), 2);
}
#[test]
fn view_searches_guarded_display_and_tokens_and_rejects_host_control_characters() {
    let mut row = row();
    row.metadata.title = Some(MetadataTokenValue::parse("Fix rendering").unwrap());
    row.metadata.display_agent = Some(MetadataTokenValue::parse("My Agent").unwrap());
    row.tokens = MetadataTokens::new(
        [(
            MetadataTokenKey::parse("issue").unwrap(),
            MetadataTokenValue::parse("ORCH-42").unwrap(),
        )]
        .into(),
    )
    .unwrap();
    let rows = vec![(Choice::Local, row)];
    for query in [
        "fix RENDER",
        "my agent",
        "issue:orch-42",
        "workspace",
        "machine",
        "LOCAL",
    ] {
        let view = View {
            query: query.into(),
            ..View::default()
        };
        assert_eq!(project(&view, &rows, &Choice::Local, None).len(), 1);
    }
    let mut view = View::default();
    for c in ['\x1b', '\n', '\u{202e}'] {
        view.push(c);
    }
    assert!(view.query.is_empty());
    for _ in 0..129 {
        view.push('界');
    }
    assert_eq!(view.query.chars().count(), 128);
    assert!(project(&view, &rows, &Choice::Local, None).is_empty());
}
#[test]
fn view_order_uses_effective_titles_and_providers_with_canonical_ties() {
    let mut a = row();
    a.label = "z".into();
    a.metadata.title = Some(MetadataTokenValue::parse("a").unwrap());
    let mut b = row();
    b.label = "a".into();
    b.metadata.title = Some(MetadataTokenValue::parse("z").unwrap());
    b.metadata.display_agent = Some(MetadataTokenValue::parse("a").unwrap());
    let rows = vec![(Choice::Local, a), (Choice::Local, b)];
    assert_eq!(
        project(
            &View {
                order: Order::Title,
                ..View::default()
            },
            &rows,
            &Choice::Local,
            None
        )[0]
        .row
        .label,
        "z"
    );
    assert_eq!(
        project(
            &View {
                order: Order::Provider,
                ..View::default()
            },
            &rows,
            &Choice::Local,
            None
        )[0]
        .row
        .label,
        "a"
    );
    let mut view = View::default();
    for _ in 0..5 {
        view.cycle_order();
    }
    for _ in 0..6 {
        view.cycle_state();
    }
    assert_eq!(view, View::default());
    for _ in 0..3 {
        view.cycle_scope();
    }
    assert_eq!(view.scope, Scope::All);
}
