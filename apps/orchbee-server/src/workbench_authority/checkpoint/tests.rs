use super::super::WorkbenchGrant;
use super::*;
use crate::test_support::WorkbenchFixture;
use orchbee_workbench_protocol::{
    LaunchProfileSelector, WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome,
    WorkbenchLabel, WorkbenchRequest, WorkbenchRequestId, WorkbenchResponse,
};
use serde_json::{Value, json};

mod directories;
mod navigation;
mod restore;
mod split_ratios;

#[test]
fn admission_validates_the_complete_graph_before_runtime_reconstruction() {
    let bytes = checkpoint();
    let admitted = WorkbenchCheckpoint::parse(&bytes).unwrap();
    let authority = admitted.reconstruct(site(300), &worker(400)).unwrap();
    assert_eq!(authority.snapshot(), restore(&bytes).unwrap().snapshot());
    assert_eq!(authority.catalog, restore(&bytes).unwrap().catalog);
    let invalid_graph =
        changed(|value| value["terminalContexts"][1]["terminalContextId"] = json!(identity(900)));
    assert!(WorkbenchCheckpoint::parse(&invalid_graph).is_err());
    assert!(WorkbenchCheckpoint::parse(b"{}").is_err());
    let unsupported = changed(|value| value["version"] = json!(8));
    assert!(matches!(
        WorkbenchCheckpoint::parse(&unsupported),
        Err(WorkbenchCheckpointError::UnsupportedVersion(8))
    ));
}

fn site(value: u128) -> SiteId {
    SiteId::parse(&identity(value)).unwrap()
}

fn worker(value: u128) -> WorkerId {
    WorkerId::parse(&identity(value)).unwrap()
}

fn presentation(id: u128, owner: &WorkerId, bound: bool) -> Value {
    json!({
        "binding": bound.then(|| json!({"workerId": owner, "terminalId": identity(id + 100)})),
        "customLabel": format!("自定义 {id}"),
        "label": format!("shell {id}"),
        "presentationId": identity(id),
    })
}

fn graph(site_id: SiteId, owner: &WorkerId, bound: bool, revision: u64) -> Value {
    json!({
        "kind": "workbench.snapshot", "protocolVersion": 1,
        "sessionId": site_id, "revision": revision.to_string(),
        "navigation": {
            "projects": [{"projectId": identity(2), "label": "项目"}],
            "directories": [{"directoryId": identity(3), "workerId": owner, "path": "C:\\work space\\项目"}],
            "workspaces":[{"workspaceId":identity(1),"terminalContextId":identity(1),"projectId":null,"primaryDirectoryId":null,"label":"Workspace"},{"workspaceId": identity(4), "projectId": identity(2), "label": "workspace", "primaryDirectoryId": identity(3), "terminalContextId": identity(5)}],
            "workspaceOrder":[identity(1),identity(4)]},
        "terminalContexts": [
            {"terminalContextId": identity(1), "groups": [], "tabs": []},
            {"terminalContextId": identity(5),
             "groups": [{"groupId": identity(6), "label": "group", "color": "cyan"}],
             "tabs": [{"tabId": identity(7), "label": "tab", "automaticLabel": true,
                "groupId": null, "pinned": true,
                "root": {"kind": "split", "splitId": identity(8), "orientation": "horizontal",
                    "children": [
                        {"slotId": identity(9), "node": {"kind": "panel", "panelId": identity(10),
                            "rightClick": "panel", "primaryPresentation": presentation(11, owner, bound),
                            "childPresentations": [presentation(15, owner, bound)]}},
                        {"slotId": identity(12), "node": {"kind": "split", "splitId": identity(19), "orientation": "vertical",
                            "children": [
                                {"slotId": identity(20), "node": {"kind": "panel", "panelId": identity(13),
                                    "primaryPresentation": presentation(14, owner, bound), "childPresentations": []}},
                                {"slotId": identity(21), "node": {"kind": "panel", "panelId": identity(22),
                                    "primaryPresentation": presentation(23, owner, bound), "childPresentations": []}}
                            ]}}
                    ]}
             }, {"tabId": identity(16), "label": "grouped tab", "groupId": identity(6), "pinned": false,
                "root": {"kind": "panel", "panelId": identity(17),
                    "primaryPresentation": presentation(18, owner, bound), "childPresentations": []}}]}
        ]
    })
}

fn fixture() -> WorkbenchAuthority {
    let mut authority = WorkbenchAuthority::workspace_fixture(site(100)).unwrap();
    authority.projection = WorkbenchProjectionStore::load(
        &serde_json::to_vec(&graph(site(100), &worker(200), true, 99)).unwrap(),
    )
    .unwrap();
    authority.last_identity = 40;
    authority.catalog_revision = 8;
    authority.catalog = [30, 31]
        .into_iter()
        .map(|counter| LaunchProfileEntry {
            profile_id: LaunchProfileId::parse(&format!("{}:{}", identity(100), identity(counter)))
                .unwrap(),
            name: LaunchProfileName::parse("same name").unwrap(),
            launch: LaunchSelection::new(
                r"C:\tools\pwsh.exe".to_owned(),
                vec![format!("arg {counter}")],
            )
            .unwrap(),
        })
        .collect();
    authority
}

fn checkpoint() -> Vec<u8> {
    fixture().checkpoint(&worker(200)).unwrap()
}

fn restore(encoded: &[u8]) -> Result<WorkbenchAuthority, WorkbenchCheckpointError> {
    WorkbenchAuthority::from_checkpoint(encoded, site(300), &worker(400))
}

fn command(authority: &mut WorkbenchAuthority, action: WorkbenchAction) -> WorkbenchCommandOutcome {
    let grant = WorkbenchGrant::manage_navigation(authority.site_id().clone());
    let request = WorkbenchRequest::Command(WorkbenchCommand {
        protocol_version: ProtocolVersion,
        site_id: authority.site_id().clone(),
        request_id: WorkbenchRequestId::parse(&identity(
            1_000 + u128::from(authority.snapshot().revision.value()),
        ))
        .unwrap(),
        expected_revision: authority.snapshot().revision,
        action,
    });
    let WorkbenchResponse::Command(result) =
        authority.execute(Some(&grant), request.try_into().unwrap())
    else {
        panic!("expected command result");
    };
    result.outcome
}

fn changed(edit: impl FnOnce(&mut Value)) -> Vec<u8> {
    let mut document: Value = serde_json::from_slice(&checkpoint()).unwrap();
    edit(&mut document);
    serde_json::to_vec(&document).unwrap()
}

#[test]
fn preserves_the_graph_and_profiles_without_retaining_process_authority() {
    let source = fixture();
    let before = source.snapshot().clone();
    let encoded = source.checkpoint(&worker(200)).unwrap();
    let restored = restore(&encoded).unwrap();
    let expected = WorkbenchProjectionStore::load(
        &serde_json::to_vec(&graph(site(300), &worker(400), false, 0)).unwrap(),
    )
    .unwrap();
    assert_eq!(restored.snapshot(), expected.snapshot());
    assert_eq!(source.snapshot(), &before);
    assert_eq!(source.catalog_revision, 8);
    assert_eq!(restored.catalog_revision, 0);
    assert!(restored.ledger.is_empty());
    assert_eq!(restored.ledger_bytes, 0);
    assert_eq!(
        restored
            .projection
            .publications_since(WorkbenchRevision::new(u64::MAX)),
        expected.publications_since(WorkbenchRevision::new(u64::MAX))
    );
    assert_eq!(restored.event_journal().head().unwrap().sequence.value(), 0);
    assert_ne!(
        restored.event_journal().head().unwrap().epoch,
        source.event_journal().head().unwrap().epoch
    );
    assert!(!restored.retired);
    assert_eq!(restored.last_identity, 40);
    for (index, counter) in [30, 31].into_iter().enumerate() {
        assert_eq!(
            restored.catalog[index].profile_id.as_str(),
            format!("{}:{}", identity(300), identity(counter))
        );
        assert_eq!(restored.catalog[index].name.as_str(), "same name");
        assert_eq!(restored.catalog[index].launch, source.catalog[index].launch);
    }
    let text = String::from_utf8(encoded.clone()).unwrap();
    for omitted in [
        "workerId",
        "sessionId",
        "terminalId",
        "revision",
        "requestId",
        "profileId",
    ] {
        assert!(!text.contains(omitted), "checkpoint retained {omitted}");
    }
    assert_eq!(restored.checkpoint(&worker(400)).unwrap(), encoded);
}

#[test]
fn preserves_deleted_identity_high_water_and_rejects_exhaustion() {
    let mut restored = restore(&checkpoint()).unwrap();
    let outcome = command(
        &mut restored,
        WorkbenchAction::ProjectCreate {
            label: WorkbenchLabel::parse("new project").unwrap(),
        },
    );
    let WorkbenchCommandOutcome::ProjectCreated { project_id, .. } = outcome else {
        panic!("expected project creation");
    };
    assert_eq!(project_id.as_str(), identity(41));
    let mut exhausted = restore(&changed(|value| {
        value["lastIdentity"] = json!(identity(u128::MAX))
    }))
    .unwrap();
    assert!(matches!(
        command(
            &mut exhausted,
            WorkbenchAction::ProjectCreate {
                label: WorkbenchLabel::parse("cannot allocate").unwrap(),
            }
        ),
        WorkbenchCommandOutcome::Rejected {
            reason: orchbee_workbench_protocol::WorkbenchRejectionReason::IdentityExhausted,
            ..
        }
    ));
}

#[test]
fn deleted_profile_is_not_resurrected_and_its_counter_is_not_reused() {
    let mut source = WorkbenchAuthority::new(site(100)).unwrap();
    source.seed_fixture_workspace(worker(200));
    let action = || WorkbenchAction::LaunchProfileCreate {
        name: LaunchProfileName::parse("profile").unwrap(),
        launch: LaunchSelection::new(r"C:\tools\pwsh.exe".to_owned(), vec![]).unwrap(),
    };
    let WorkbenchCommandOutcome::LaunchProfileCreated { profile_id, .. } =
        command(&mut source, action())
    else {
        panic!("expected profile creation");
    };
    command(
        &mut source,
        WorkbenchAction::LaunchProfileDelete {
            selector: LaunchProfileSelector::parse(profile_id.as_str()).unwrap(),
        },
    );
    let mut restored = restore(&source.checkpoint(&worker(200)).unwrap()).unwrap();
    assert!(!source.ledger.is_empty());
    assert!(restored.ledger.is_empty());
    assert!(restored.catalog.is_empty());
    let WorkbenchCommandOutcome::LaunchProfileCreated { profile_id, .. } =
        command(&mut restored, action())
    else {
        panic!("expected restored profile creation");
    };
    assert_eq!(
        profile_id.as_str(),
        format!("{}:{}", identity(300), identity(3))
    );
}

#[test]
fn restores_actual_committed_membership_with_new_grants_and_an_empty_journal() {
    let mut source = WorkbenchAuthority::new(site(100)).unwrap();
    source.seed_fixture_workspace(worker(200));
    let membership = source
        .prepare_fixture_tab(orchbee_workbench_protocol::PresentationBinding {
            terminal_id: orchbee_workbench_protocol::TerminalId::parse(&identity(900)).unwrap(),
            worker_id: worker(200),
        })
        .unwrap()
        .commit()
        .metadata()
        .clone();
    command(
        &mut source,
        WorkbenchAction::ProjectCreate {
            label: WorkbenchLabel::parse("retained project").unwrap(),
        },
    );
    let mut restored = restore(&source.checkpoint(&worker(200)).unwrap()).unwrap();
    let retained = &restored.snapshot().terminal_contexts[0].tabs[0];
    assert_eq!(&retained.tab_id, membership.tab_id());
    let PanelNode::Panel {
        primary_presentation,
        ..
    } = &retained.root
    else {
        panic!("expected initial_workspace panel");
    };
    assert!(primary_presentation.binding.is_none());
    assert_eq!(
        &primary_presentation.presentation_id,
        membership.presentation_id()
    );
    assert_eq!(
        restored.snapshot().navigation.projects[0].label,
        "retained project"
    );
    let fresh_projection = WorkbenchProjectionStore::load(
        &serialize_document(&WorkbenchProtocolDocument::Snapshot(
            restored.snapshot().clone(),
        ))
        .unwrap(),
    )
    .unwrap();
    assert_eq!(restored.projection.snapshot(), fresh_projection.snapshot());
    assert_eq!(
        restored
            .projection
            .publications_since(WorkbenchRevision::new(u64::MAX)),
        fresh_projection.publications_since(WorkbenchRevision::new(u64::MAX))
    );
    assert_eq!(restored.event_journal().head().unwrap().sequence.value(), 0);
    let before = restored.snapshot().clone();
    let request = WorkbenchRequest::Command(WorkbenchCommand {
        protocol_version: ProtocolVersion,
        site_id: restored.site_id().clone(),
        request_id: WorkbenchRequestId::parse(&identity(77)).unwrap(),
        expected_revision: WorkbenchRevision::new(0),
        action: WorkbenchAction::ProjectCreate {
            label: WorkbenchLabel::parse("stale grant").unwrap(),
        },
    });
    let response = restored.execute(
        Some(&WorkbenchGrant::manage_navigation(site(100))),
        request.try_into().unwrap(),
    );
    assert!(matches!(
        response,
        WorkbenchResponse::Command(orchbee_workbench_protocol::WorkbenchCommandResult {
            outcome: WorkbenchCommandOutcome::Denied {},
            ..
        })
    ));
    assert_eq!(restored.snapshot(), &before);
    assert!(matches!(
        command(
            &mut restored,
            WorkbenchAction::ProjectCreate {
                label: WorkbenchLabel::parse("fresh grant").unwrap(),
            }
        ),
        WorkbenchCommandOutcome::ProjectCreated { .. }
    ));
}

#[test]
fn capture_refuses_foreign_directories_primary_and_child_presentations() {
    let paths = [
        "/navigation/directories/0/workerId",
        "/terminalContexts/1/tabs/0/root/children/0/node/primaryPresentation/binding/workerId",
        "/terminalContexts/1/tabs/0/root/children/0/node/childPresentations/0/binding/workerId",
    ];
    for path in paths {
        let mut source = fixture();
        let mut value = graph(site(100), &worker(200), true, 99);
        *value.pointer_mut(path).unwrap() = json!(identity(500));
        source.projection =
            WorkbenchProjectionStore::load(&serde_json::to_vec(&value).unwrap()).unwrap();
        let before = source.snapshot().clone();
        assert_eq!(
            source.checkpoint(&worker(200)),
            Err(WorkbenchCheckpointError::ForeignWorker)
        );
        assert_eq!(source.snapshot(), &before);
    }
    let mut retired = fixture();
    retired.retired = true;
    assert_eq!(
        retired.checkpoint(&worker(200)),
        Err(WorkbenchCheckpointError::RetiredAuthority)
    );
}

#[test]
fn rejects_missing_unknown_duplicate_and_unsupported_fields() {
    for field in [
        "schema",
        "version",
        "lastIdentity",
        "navigation",
        "terminalContexts",
        "profiles",
        "terminalDirectories",
    ] {
        assert!(
            restore(&changed(|value| {
                value.as_object_mut().unwrap().remove(field);
            }))
            .is_err(),
            "missing {field}"
        );
    }
    for (path, field) in [
        ("/terminalContexts/1/tabs/1", "automaticLabel"),
        ("/terminalContexts/1/tabs/0", "groupId"),
        ("/terminalContexts/1/tabs/0/root", "ratio"),
        ("/terminalContexts/1/tabs/1/root", "rightClick"),
        (
            "/terminalContexts/1/tabs/1/root/primaryPresentation",
            "binding",
        ),
        (
            "/terminalContexts/1/tabs/1/root/primaryPresentation",
            "customLabel",
        ),
    ] {
        assert!(
            restore(&changed(|value| {
                assert!(
                    value
                        .pointer_mut(path)
                        .unwrap()
                        .as_object_mut()
                        .unwrap()
                        .remove(field)
                        .is_some()
                );
            }))
            .is_err(),
            "missing {path}/{field}"
        );
    }
    assert!(restore(&changed(|value| value["sessionId"] = json!(identity(100)))).is_err());
    assert!(restore(&changed(|value| value["schema"] = json!("another.schema"))).is_err());
    assert_eq!(
        restore(&changed(|value| value["version"] = json!(8))).err(),
        Some(WorkbenchCheckpointError::UnsupportedVersion(8))
    );
    let encoded = String::from_utf8(checkpoint()).unwrap();
    for (needle, replacement) in [
        ("\"version\":1", "\"version\":1,\"version\":1"),
        ("\"binding\":null", "\"binding\":null,\"binding\":null"),
        (
            "\"name\":\"same name\"",
            "\"name\":\"same name\",\"name\":\"same name\"",
        ),
    ] {
        assert!(encoded.contains(needle));
        assert!(
            restore(encoded.replacen(needle, replacement, 1).as_bytes()).is_err(),
            "duplicate {needle}"
        );
    }
}

#[test]
fn refuses_injected_bindings_and_workers_in_retained_configuration() {
    let primary = "/terminalContexts/1/tabs/0/root/children/0/node/primaryPresentation/binding";
    let child = "/terminalContexts/1/tabs/0/root/children/0/node/childPresentations/0/binding";
    for path in [primary, child] {
        let encoded = changed(|value| {
            *value.pointer_mut(path).unwrap() =
                json!({"workerId": identity(400), "terminalId": identity(800)})
        });
        assert_eq!(
            restore(&encoded).err(),
            Some(WorkbenchCheckpointError::TerminalBinding)
        );
    }
    assert!(
        restore(&changed(
            |value| value["navigation"]["directories"][0]["workerId"] = json!(identity(200))
        ))
        .is_err()
    );
}

#[test]
fn refuses_invalid_membership_paths_launches_and_allocation_counters() {
    for (path, value) in [
        (
            "/navigation/workspaces/0/primaryDirectoryId",
            json!(identity(900)),
        ),
        ("/navigation/directories/0/path", json!("relative")),
        ("/terminalContexts/1/tabs/0/groupId", json!(identity(900))),
        (
            "/terminalContexts/1/tabs/0/root/children/0/node/panelId",
            json!(identity(2)),
        ),
        ("/profiles/0/counter", json!(identity(3))),
        ("/profiles/1/counter", json!(identity(30))),
        ("/profiles/1/counter", json!(identity(41))),
        ("/profiles/0/name", json!("")),
        ("/profiles/0/launch/exec", json!("")),
        ("/lastIdentity", json!(identity(15))),
        ("/lastIdentity", json!(identity(0))),
        ("/lastIdentity", json!("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")),
        ("/lastIdentity", json!(40)),
    ] {
        let encoded = changed(|document| *document.pointer_mut(path).unwrap() = value);
        assert!(restore(&encoded).is_err(), "accepted invalid {path}");
    }
}

#[test]
fn enforces_checkpoint_and_catalog_limits() {
    let mut exact = checkpoint();
    exact.resize(MAXIMUM_WORKBENCH_CHECKPOINT_BYTES, b' ');
    assert!(restore(&exact).is_ok());
    exact.push(b' ');
    assert_eq!(
        restore(&exact).err(),
        Some(WorkbenchCheckpointError::EncodedTooLarge {
            actual: MAXIMUM_WORKBENCH_CHECKPOINT_BYTES + 1,
            maximum: MAXIMUM_WORKBENCH_CHECKPOINT_BYTES,
        })
    );
    for count in [64, 65] {
        let entries = (0..count)
            .map(|index| {
                json!({
                    "counter": identity(100 + index), "name": "profile",
                    "launch": {"exec": r"C:\tools\pwsh.exe", "args": []},
                })
            })
            .collect::<Vec<_>>();
        let encoded = changed(|value| {
            value["lastIdentity"] = json!(identity(200));
            value["profiles"] = json!(entries);
        });
        assert_eq!(restore(&encoded).is_ok(), count == 64);
    }
    let mut source = fixture();
    source.last_identity = 200;
    source.catalog = (100..164)
        .map(|counter| LaunchProfileEntry {
            profile_id: LaunchProfileId::parse(&format!("{}:{}", identity(100), identity(counter)))
                .unwrap(),
            name: LaunchProfileName::parse("profile").unwrap(),
            launch: LaunchSelection::new(
                r"C:\tools\pwsh.exe".to_owned(),
                vec!["\u{1}".repeat(1_600)],
            )
            .unwrap(),
        })
        .collect();
    assert_eq!(
        source.checkpoint(&worker(200)),
        Err(WorkbenchCheckpointError::InvalidCatalog)
    );
    let oversized_catalog =
        changed(|value| {
            value["lastIdentity"] = json!(identity(200));
            value["profiles"] =
                json!((100..164).map(|counter| json!({
            "counter": identity(counter), "name": "profile",
            "launch": {"exec": r"C:\tools\pwsh.exe", "args": ["\u{1}".repeat(1_600)]},
        })).collect::<Vec<_>>());
        });
    assert_eq!(
        restore(&oversized_catalog).err(),
        Some(WorkbenchCheckpointError::InvalidCatalog)
    );
}

#[test]
fn retains_shared_projection_size_limit_and_rejects_foreign_profile_identity() {
    let oversized_graph = changed(|value| {
        value["lastIdentity"] = json!(identity(1_000));
        value["navigation"]["directories"] =
            json!((100..356).map(|counter| json!({
            "directoryId": identity(counter), "path": format!("C:\\{}", "x".repeat(4_093)),
        })).collect::<Vec<_>>());
        value["navigation"]["workspaces"][0]["primaryDirectoryId"] = json!(identity(100));
    });
    assert!(matches!(
        restore(&oversized_graph),
        Err(WorkbenchCheckpointError::InvalidProjection(
            ContractError::EncodedValueTooLarge { .. }
        ))
    ));
    let mut source = fixture();
    source.catalog[0].profile_id =
        LaunchProfileId::parse(&format!("{}:{}", identity(999), identity(30))).unwrap();
    assert_eq!(
        source.checkpoint(&worker(200)),
        Err(WorkbenchCheckpointError::InvalidIdentity)
    );
}

#[test]
fn workspace_order_survives_checkpoint_and_invalid_permutations_are_refused() {
    let mut owner = fixture();
    let project_id = owner.snapshot().navigation.projects[0].project_id.clone();
    let added = command(
        &mut owner,
        WorkbenchAction::WorkspaceCreate {
            project_id: Some(project_id),
            label: WorkbenchLabel::parse("second").unwrap(),
        },
    );
    let WorkbenchCommandOutcome::WorkspaceCreated { workspace_id, .. } = added else {
        panic!("workspace")
    };
    let result = command(
        &mut owner,
        WorkbenchAction::WorkspaceMove {
            workspace_id: workspace_id.clone(),
            insert_index: orchbee_workbench_protocol::WorkbenchWorkspaceInsertIndex::new(0)
                .unwrap(),
        },
    );
    assert!(matches!(
        result,
        WorkbenchCommandOutcome::WorkspacesReordered { .. }
    ));
    let bytes = owner.checkpoint(&worker(200)).unwrap();
    let recovered = restore(&bytes).unwrap();
    assert_eq!(
        recovered.snapshot().navigation.workspace_order,
        owner.snapshot().navigation.workspace_order
    );
    assert_eq!(
        recovered
            .snapshot()
            .navigation
            .ordered_workspaces()
            .unwrap()[0]
            .1
            .workspace_id,
        workspace_id
    );
    for order in [
        json!([identity(4)]),
        json!([identity(4), identity(4)]),
        json!([identity(4), identity(900)]),
    ] {
        let mut value: Value = serde_json::from_slice(&bytes).unwrap();
        value["navigation"]["workspaceOrder"] = order;
        assert!(WorkbenchCheckpoint::parse(&serde_json::to_vec(&value).unwrap()).is_err());
    }
}
