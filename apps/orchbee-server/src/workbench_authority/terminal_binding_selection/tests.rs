use super::*;
use crate::workbench_projection::WorkbenchProjectionStore;
use orchbee_workbench_protocol::{
    PanelOrientation, PanelSlot, Presentation, TerminalId, TerminalPresentationId,
    WorkbenchPanelId, WorkbenchPanelSlotId, WorkbenchProtocolDocument, WorkbenchSplitId, WorkerId,
    serialize_document,
};

fn identity(value: u128) -> String {
    format!("{value:032x}")
}

fn site(value: u128) -> SiteId {
    SiteId::parse(&identity(value)).unwrap()
}

fn binding(terminal: u128, worker: u128) -> PresentationBinding {
    PresentationBinding {
        terminal_id: TerminalId::parse(&identity(terminal)).unwrap(),
        worker_id: WorkerId::parse(&identity(worker)).unwrap(),
    }
}

fn authority() -> WorkbenchAuthority {
    WorkbenchAuthority::new(site(91)).unwrap()
}

#[test]
fn active_exposure_requires_the_exact_current_site_worker_and_terminal_without_mutation() {
    let mut authority = authority();
    let selected = binding(81, 71);
    assert_eq!(
        authority.terminal_binding_exposure(&site(91), &selected),
        WorkbenchBindingExposure::NotExposed
    );
    authority
        .commit_standalone_intent(selected.clone())
        .unwrap();
    let before = authority.projection.snapshot().clone();
    let cursor = authority.last_identity;
    let ledger_bytes = authority.ledger_bytes;

    for (selected_site, candidate, expected) in [
        (
            site(91),
            selected.clone(),
            WorkbenchBindingExposure::Exposed,
        ),
        (
            site(92),
            selected.clone(),
            WorkbenchBindingExposure::NotExposed,
        ),
        (
            site(91),
            binding(82, 71),
            WorkbenchBindingExposure::NotExposed,
        ),
        (
            site(91),
            binding(81, 72),
            WorkbenchBindingExposure::NotExposed,
        ),
    ] {
        assert_eq!(
            authority.terminal_binding_exposure(&selected_site, &candidate),
            expected
        );
    }
    assert_eq!(authority.projection.snapshot(), &before);
    assert_eq!(authority.last_identity, cursor);
    assert_eq!(authority.ledger_bytes, ledger_bytes);
    assert!(authority.ledger.is_empty());
}

#[test]
fn retired_authority_is_unavailable_before_site_or_tuple_interpretation() {
    let mut authority = authority();
    authority.retired = true;
    let before = authority.projection.snapshot().clone();
    for (selected_site, candidate) in [(site(91), binding(81, 71)), (site(92), binding(82, 72))] {
        assert_eq!(
            authority.terminal_binding_exposure(&selected_site, &candidate),
            WorkbenchBindingExposure::Unavailable
        );
    }
    assert_eq!(authority.projection.snapshot(), &before);
    assert_eq!(authority.last_identity, 1);
    assert!(authority.ledger.is_empty());
}

#[test]
fn exposure_walks_nested_panels_and_checks_primary_and_child_presentations() {
    let mut authority = authority();
    let child_binding = binding(81, 71);
    let nested_primary_binding = binding(82, 72);
    let mut snapshot = authority.projection.snapshot().clone();
    snapshot.terminal_contexts[0]
        .tabs
        .push(orchbee_workbench_protocol::Tab {
            automatic_label: false,
            group_id: None,
            label: "1".to_owned(),
            pinned: false,
            tab_id: orchbee_workbench_protocol::WorkbenchTabId::parse(&identity(100)).unwrap(),
            root: PanelNode::Split {
                ratio: Default::default(),
                children: Box::new([
                    PanelSlot {
                        node: PanelNode::Panel {
                            right_click: Default::default(),
                            child_presentations: vec![Presentation {
                                custom_label: None,
                                binding: Some(child_binding.clone()),
                                label: "child".to_owned(),
                                presentation_id: TerminalPresentationId::parse(&identity(104))
                                    .unwrap(),
                            }],
                            panel_id: WorkbenchPanelId::parse(&identity(102)).unwrap(),
                            primary_presentation: Presentation {
                                custom_label: None,
                                binding: None,
                                label: "primary".to_owned(),
                                presentation_id: TerminalPresentationId::parse(&identity(103))
                                    .unwrap(),
                            },
                        },
                        slot_id: WorkbenchPanelSlotId::parse(&identity(101)).unwrap(),
                    },
                    PanelSlot {
                        node: PanelNode::Split {
                            ratio: Default::default(),
                            children: Box::new([
                                PanelSlot {
                                    node: PanelNode::Panel {
                                        right_click: Default::default(),
                                        child_presentations: Vec::new(),
                                        panel_id: WorkbenchPanelId::parse(&identity(108)).unwrap(),
                                        primary_presentation: Presentation {
                                            custom_label: None,
                                            binding: Some(nested_primary_binding.clone()),
                                            label: "nested".to_owned(),
                                            presentation_id: TerminalPresentationId::parse(
                                                &identity(109),
                                            )
                                            .unwrap(),
                                        },
                                    },
                                    slot_id: WorkbenchPanelSlotId::parse(&identity(107)).unwrap(),
                                },
                                PanelSlot {
                                    node: PanelNode::Panel {
                                        right_click: Default::default(),
                                        child_presentations: Vec::new(),
                                        panel_id: WorkbenchPanelId::parse(&identity(111)).unwrap(),
                                        primary_presentation: Presentation {
                                            custom_label: None,
                                            binding: None,
                                            label: "empty".to_owned(),
                                            presentation_id: TerminalPresentationId::parse(
                                                &identity(112),
                                            )
                                            .unwrap(),
                                        },
                                    },
                                    slot_id: WorkbenchPanelSlotId::parse(&identity(110)).unwrap(),
                                },
                            ]),
                            orientation: PanelOrientation::Vertical,
                            split_id: WorkbenchSplitId::parse(&identity(106)).unwrap(),
                        },
                        slot_id: WorkbenchPanelSlotId::parse(&identity(105)).unwrap(),
                    },
                ]),
                orientation: PanelOrientation::Horizontal,
                split_id: WorkbenchSplitId::parse(&identity(99)).unwrap(),
            },
        });
    let encoded = serialize_document(&WorkbenchProtocolDocument::Snapshot(snapshot)).unwrap();
    authority.projection = WorkbenchProjectionStore::load(&encoded).unwrap();
    let before = authority.projection.snapshot().clone();

    for selected in [child_binding, nested_primary_binding] {
        assert_eq!(
            authority.terminal_binding_exposure(&site(91), &selected),
            WorkbenchBindingExposure::Exposed
        );
    }
    assert_eq!(
        authority.terminal_binding_exposure(&site(91), &binding(83, 73)),
        WorkbenchBindingExposure::NotExposed
    );
    assert_eq!(authority.projection.snapshot(), &before);
}
