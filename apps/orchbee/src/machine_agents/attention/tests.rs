use super::*;
fn row() -> Row {
    Row {
        seen: true,
        order: PlacementOrder::default(),
        target: super::super::tests::target(),
        label: "terminal".into(),
        workspace: String::new(),
        tab_label: None,
        state: PanelAgentState::Idle,
        sequence: WorkbenchRevision::new(1),
        metadata: PanelMetadataEffective::default(),
        tokens: MetadataTokens::default(),
    }
}
#[test]
fn endpoint_baseline_changed_new_rebound_and_restart_versions_are_distinct() {
    let mut attention = Attention::default();
    let local = Choice::Local;
    let round = super::super::tests::checkpoint(vec![row()]);
    assert!(attention.project(&local, &round).unwrap()[0].seen);
    let remote = Choice::Remote(ProfileId::parse(&"9".repeat(32)).unwrap());
    assert!(attention.project(&remote, &round).unwrap()[0].seen);
    let mut changed = round.as_ref().clone();
    let mut rows = changed.rows.to_vec();
    rows[0].sequence = WorkbenchRevision::new(2);
    changed.rows = rows.into();
    changed.report_revision = WorkbenchRevision::new(3);
    let next = attention
        .project(&local, &Arc::new(changed.clone()))
        .unwrap();
    assert!(!next[0].seen);
    assert_eq!(next[0].status(), "done");
    assert!(attention.project(&remote, &round).unwrap()[0].seen);
    let before = attention.clone();
    let mut invalid = changed.clone();
    invalid.report_revision = WorkbenchRevision::new(1);
    assert!(attention.project(&local, &Arc::new(invalid)).is_err());
    assert_eq!(attention, before);
    let binding = next[0].target.physical.clone();
    let mut ack = ClientTerminalAgentAttentionSnapshot {
        report_revision: WorkbenchRevision::new(3),
        outer_focused: Some(true),
        entries: vec![ClientTerminalAgentAttentionEntry {
            binding: binding.clone(),
            state: Some(ClientTerminalAgentAttentionVersion {
                sequence: WorkbenchRevision::new(1),
                seen: true,
            }),
        }],
    };
    assert!(!attention.acknowledge(&local, &ack));
    ack.entries[0].state.as_mut().unwrap().sequence = WorkbenchRevision::new(2);
    assert!(attention.acknowledge(&local, &ack));
    assert!(
        attention
            .project(&local, &Arc::new(changed.clone()))
            .unwrap()[0]
            .seen
    );
    let mut rows = changed.rows.to_vec();
    rows[0].target.physical.generation =
        orchbee_agent_observation_protocol::AgentAttachmentGeneration::parse(&"9".repeat(32))
            .unwrap();
    changed.rows = rows.into();
    assert!(
        !attention
            .project(&local, &Arc::new(changed.clone()))
            .unwrap()[0]
            .seen
    );
    assert!(!attention.acknowledge(&local, &ack));
    let site = SiteId::parse(&"8".repeat(32)).unwrap();
    changed.site_id = site.clone();
    let mut rows = changed.rows.to_vec();
    rows[0].target.context.site_id = site;
    changed.rows = rows.into();
    assert!(attention.project(&local, &Arc::new(changed)).unwrap()[0].seen);
}
#[test]
fn invalid_complete_checkpoint_preserves_prior_attention_and_missing_members_are_pruned() {
    let mut attention = Attention::default();
    let round = super::super::tests::checkpoint(vec![row()]);
    attention.project(&Choice::Local, &round).unwrap();
    let before = attention.clone();
    let invalid = super::super::tests::checkpoint(vec![row(), row()]);
    assert!(attention.project(&Choice::Local, &invalid).is_err());
    assert_eq!(attention, before);
    attention
        .project(&Choice::Local, &super::super::tests::checkpoint(Vec::new()))
        .unwrap();
    assert!(!attention.project(&Choice::Local, &round).unwrap()[0].seen);
}
