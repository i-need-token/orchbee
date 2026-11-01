use super::*;
use orchbee_workbench_protocol::{
    SemanticEventData as D, SemanticEventKind as K, SemanticEventMatch, SemanticEventScope,
    WorkbenchTabId, WorkbenchTerminalContextId,
};
fn initial() -> Snapshot {
    serde_json::from_value(
        serde_json::json!({"protocolVersion":1,"sessionId":format!("{:032x}",2),"revision":"0",
        "navigation":{"workspaceOrder":[],"projects":[],"directories":[],"workspaces":[{"workspaceId":format!("{:032x}",1),"terminalContextId":format!("{:032x}",1),"projectId":null,"primaryDirectoryId":null,"label":"Workspace"}]},
        "terminalContexts":[{"terminalContextId":format!("{:032x}",1),"groups":[],"tabs":[]}]}),
    )
    .unwrap()
}
fn filter(kind: K) -> SemanticEventFilter {
    SemanticEventFilter::Event {
        event: kind,
        scope: SemanticEventScope::All {},
        condition: SemanticEventMatch::Any {},
    }
}
fn journal() -> SessionEventJournal {
    SessionEventJournal::with_epoch(
        &initial(),
        WorkbenchRequestId::parse(&format!("{:032x}", 3)).unwrap(),
    )
}
fn tab(label: &str) -> D {
    D::Tab {
        terminal_context_id: WorkbenchTerminalContextId::parse(&format!("{:032x}", 1)).unwrap(),
        tab_id: WorkbenchTabId::parse(&format!("{:032x}", 4)).unwrap(),
        label: label.into(),
        index: 0,
    }
}

#[test]
fn semantic_event_journal_paging_preserves_filters_full_width_and_gap() {
    let journal = journal();
    let start = journal.head().unwrap();
    for index in 0..514 {
        assert!(journal.publish(
            WorkbenchRevision::new(u64::MAX),
            None,
            if index % 2 == 0 {
                K::TabCreated
            } else {
                K::TabRenamed
            },
            tab("shell")
        ));
    }
    let first = journal.read(&start, &[filter(K::TabRenamed)], 2).unwrap();
    assert_eq!(
        first.gap,
        Some(SemanticEventGap {
            after: WorkbenchRevision::new(0),
            through: WorkbenchRevision::new(2)
        })
    );
    assert_eq!(first.events.len(), 2);
    assert!(first.more);
    assert_eq!(first.cursor.sequence.value(), 7);
    assert!(
        first
            .events
            .iter()
            .all(|event| event.event == K::TabRenamed
                && event.workbench_revision.value() == u64::MAX)
    );
    let empty = journal
        .read(&first.cursor, &[filter(K::PaneExited)], 32)
        .unwrap();
    assert!(empty.events.is_empty());
    assert!(!empty.more);
    assert_eq!(empty.cursor.sequence.value(), 514);
}
#[test]
fn only_a_terminal_output_frame_publishes_without_a_commit_hint() {
    let journal = journal();
    let signal = std::sync::Arc::new(SessionChangeSignal::default());
    journal.watch(&signal);
    let revision = initial().revision;
    journal.publish(revision, None, K::PaneOutputChanged, tab("output"));
    assert_eq!((signal.revision(), signal.commit_revision()), (1, 0));
    journal.publish(revision, None, K::PaneExited, tab("exited"));
    assert_eq!((signal.revision(), signal.commit_revision()), (2, 1));
    journal.publish(revision, None, K::PaneAgentStatusChanged, tab("agent"));
    assert_eq!((signal.revision(), signal.commit_revision()), (3, 2));
}
#[test]
fn semantic_event_journal_rejects_foreign_future_and_exhausted_cursors() {
    let journal = journal();
    let mut cursor = journal.head().unwrap();
    cursor.sequence = WorkbenchRevision::new(1);
    assert!(matches!(
        journal.read(&cursor, &[filter(K::TabCreated)], 1),
        Err(Failure::CursorAhead)
    ));
    cursor.sequence = WorkbenchRevision::new(0);
    cursor.epoch = WorkbenchRequestId::parse(&format!("{:032x}", 9)).unwrap();
    assert_eq!(journal.validate_cursor(&cursor), Err(Failure::CursorEpoch));
    journal.0.lock().unwrap().head = u64::MAX;
    assert!(!journal.publish(WorkbenchRevision::new(1), None, K::TabCreated, tab("shell")));
    assert_eq!(journal.head(), Err(Failure::RevisionExhausted));
}
#[test]
fn semantic_event_invalid_or_over_budget_publication_marks_loss_without_fabrication() {
    let journal = journal();
    let start = journal.head().unwrap();
    assert!(journal.publish(WorkbenchRevision::new(1), None, K::TabCreated, tab("shell")));
    assert!(!journal.publish(
        WorkbenchRevision::new(2),
        None,
        K::PaneExited,
        tab("incompatible")
    ));
    let page = journal.read(&start, &[filter(K::TabCreated)], 32).unwrap();
    assert!(page.events.is_empty());
    assert_eq!(page.gap.unwrap().through.value(), 2);
    let mut state = journal.0.lock().unwrap();
    state.head = u64::MAX - 1;
    drop(state);
    assert!(journal.publish(WorkbenchRevision::new(3), None, K::TabCreated, tab("shell")));
    assert_eq!(journal.head().unwrap().sequence.value(), u64::MAX);
}
#[test]
fn semantic_event_projection_records_intermediate_commits_and_noop_revision() {
    use crate::workbench_projection::WorkbenchProjectionStore;
    use orchbee_workbench_protocol::{
        PanelNode, Presentation, Tab, TerminalPresentationId, WorkbenchPanelId,
        WorkbenchProtocolDocument, serialize_document,
    };
    let initial = initial();
    let bytes = serialize_document(&WorkbenchProtocolDocument::Snapshot(initial.clone())).unwrap();
    let mut store = WorkbenchProjectionStore::load(&bytes).unwrap();
    let journal = store.events();
    let cursor = journal.head().unwrap();
    let mut next = initial;
    next.revision = WorkbenchRevision::new(1);
    next.terminal_contexts[0].tabs.push(Tab {
        tab_id: WorkbenchTabId::parse(&format!("{:032x}", 4)).unwrap(),
        label: "one".into(),
        pinned: false,
        automatic_label: false,
        group_id: None,
        root: PanelNode::Panel {
            panel_id: WorkbenchPanelId::parse(&format!("{:032x}", 5)).unwrap(),
            right_click: Default::default(),
            child_presentations: Vec::new(),
            primary_presentation: Presentation {
                presentation_id: TerminalPresentationId::parse(&format!("{:032x}", 6)).unwrap(),
                binding: None,
                custom_label: None,
                label: "shell".into(),
                launch_failure: None,
                launch_program: None,
            },
        },
    });
    let encoded = |snapshot: &Snapshot| {
        serialize_document(&WorkbenchProtocolDocument::Snapshot(snapshot.clone())).unwrap()
    };
    store.advance_projection(&encoded(&next)).unwrap();
    next.revision = WorkbenchRevision::new(2);
    next.terminal_contexts[0].tabs[0].label = "two".into();
    store.advance_projection(&encoded(&next)).unwrap();
    next.revision = WorkbenchRevision::new(3);
    next.terminal_contexts[0].tabs[0].label = "three".into();
    store.advance_projection(&encoded(&next)).unwrap();
    let head = journal.head().unwrap();
    next.revision = WorkbenchRevision::new(4);
    store.advance_projection(&encoded(&next)).unwrap();
    assert_eq!(journal.head().unwrap(), head);
    let page = journal
        .read(&cursor, &[filter(K::TabRenamed), filter(K::TabCreated)], 32)
        .unwrap();
    assert_eq!(
        page.events
            .iter()
            .map(|event| match &event.data {
                D::Tab { label, .. } => label.as_str(),
                _ => panic!(),
            })
            .collect::<Vec<_>>(),
        ["one", "two", "three"]
    );
    assert!(page.gap.is_none());
}
