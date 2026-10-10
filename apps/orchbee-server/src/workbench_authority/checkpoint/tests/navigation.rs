use super::*;

fn historical_at_wire_limit(version: u32) -> Value {
    let directories: Vec<_> = (0..256)
        .map(|index| {
            json!({
                "directoryId": identity(index + 3),
                "path": format!("C:/{}", "a".repeat(if index < 122 { 3984 } else { 3983 }))
            })
        })
        .collect();
    let mut checkpoint = json!({
        "schema": "orchbee.workbench.checkpoint", "version": version,
        "lastIdentity": identity(260),
        "navigation": {
            "standaloneTerminalContextId": identity(1),
            "projects": [{"projectId": identity(2), "label": "P",
                "directories": directories,
                "workspaces": [{"workspaceId": identity(259), "label": "W",
                    "primaryDirectoryId": identity(3), "terminalContextId": identity(260)}]}]
        },
        "terminalContexts": ([1, 260].map(|id| json!({
            "terminalContextId": identity(id), "groups": [], "tabs": []
        }))),
        "profiles": []
    });
    for (minimum, field) in [
        (2, "agentSessions"),
        (3, "terminalDirectories"),
        (4, "managedNames"),
    ] {
        if version >= minimum {
            checkpoint[field] = json!([]);
        }
    }
    checkpoint
}

#[test]
fn historical_wire_limit_refuses_migration_without_misclassifying_it_as_corruption() {
    use orchbee_workbench_protocol::MAXIMUM_ENCODED_PROJECTION_BYTES;
    for version in 1..=5 {
        let checkpoint = historical_at_wire_limit(version);
        // Recreate the historical wire shape independently of the new normalization owner.
        let mut old_navigation = checkpoint["navigation"].clone();
        for directory in old_navigation["projects"][0]["directories"]
            .as_array_mut()
            .unwrap()
        {
            directory["workerId"] = json!(identity(1));
        }
        let old_snapshot = json!({
            "kind": "workbench.snapshot", "protocolVersion": 1, "revision": "0",
            "siteId": identity(1), "navigation": old_navigation,
            "terminalContexts": checkpoint["terminalContexts"]
        });
        assert_eq!(
            serde_json::to_vec(&old_snapshot).unwrap().len(),
            MAXIMUM_ENCODED_PROJECTION_BYTES
        );
        let encoded = serde_json::to_vec(&checkpoint).unwrap();
        assert!(encoded.len() < MAXIMUM_WORKBENCH_CHECKPOINT_BYTES);
        assert!(matches!(
            WorkbenchCheckpoint::parse(&encoded),
            Err(WorkbenchCheckpointError::HistoricalMigrationTooLarge {
                version: retained_version,
                actual: 1_048_677,
                maximum: MAXIMUM_ENCODED_PROJECTION_BYTES,
            }) if retained_version == version
        ));

        // Only path length changes: the same identities, graph, memberships and source schema
        // fully validate when the normalized wire form fits the unchanged bound exactly.
        let mut fits = checkpoint.clone();
        let path = fits["navigation"]["projects"][0]["directories"][0]["path"]
            .as_str()
            .unwrap();
        fits["navigation"]["projects"][0]["directories"][0]["path"] =
            json!(&path[..path.len() - 101]);
        let restored = restore(&serde_json::to_vec(&fits).unwrap()).unwrap();
        assert_eq!(
            serialize_document(&WorkbenchProtocolDocument::Snapshot(
                restored.snapshot().clone()
            ))
            .unwrap()
            .len(),
            MAXIMUM_ENCODED_PROJECTION_BYTES
        );
        assert_eq!(restored.snapshot().navigation.directories.len(), 256);
        assert_eq!(restored.snapshot().navigation.workspaces.len(), 1);

        // Capacity refusal never relaxes source-shape admission.
        let mut unknown = checkpoint;
        unknown["navigation"]["projects"][0]["workspaces"][0]["projectId"] = json!(identity(2));
        assert!(matches!(
            WorkbenchCheckpoint::parse(&serde_json::to_vec(&unknown).unwrap()),
            Err(WorkbenchCheckpointError::InvalidDocument)
        ));
    }
}

fn historical(version: u32) -> Value {
    let mut value: Value = serde_json::from_slice(&checkpoint()).unwrap();
    historical_navigation(&mut value, version);
    value["lastIdentity"] = json!(identity(50));
    let first = &mut value["navigation"]["projects"][0];
    first["directories"].as_array_mut().unwrap().push(json!({
        "directoryId": identity(29), "path": "C:/unused-first"
    }));
    first["workspaces"].as_array_mut().unwrap().push(json!({
        "workspaceId": identity(35), "label": "last in first group",
        "primaryDirectoryId": null, "terminalContextId": identity(36)
    }));
    value["navigation"]["projects"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "projectId": identity(25), "label": "second group",
            "directories": [
                {"directoryId": identity(26), "path": "D:/second"},
                {"directoryId": identity(27), "path": "D:/unused-second"}
            ],
            "workspaces": [{"workspaceId": identity(24), "label": "interleaved",
                "primaryDirectoryId": identity(26), "terminalContextId": identity(28)}]
        }));
    for context in [28, 36] {
        value["terminalContexts"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "terminalContextId": identity(context), "groups": [], "tabs": []
            }));
    }
    for (minimum, field, rows) in [
        (
            2,
            "agentSessions",
            json!([{"presentationId":identity(11),
            "session":{"source":"herdr:grok","agent":"grok","reference":{"kind":"id","value":"retained-reference"}}}]),
        ),
        (
            3,
            "terminalDirectories",
            json!([{"presentationId":identity(11), "path":"E:/restart-cwd"}]),
        ),
        (
            4,
            "managedNames",
            json!([{"presentationId":identity(11), "name":"reviewer", "providerId":"grok"}]),
        ),
    ] {
        if version >= minimum {
            value[field] = rows;
        } else {
            value.as_object_mut().unwrap().remove(field);
        }
    }
    value
}

#[test]
fn versions_one_through_five_normalize_interleaved_ids_and_preserve_restart_intent() {
    for version in 1..=5 {
        let legacy = historical(version);
        let bytes = serde_json::to_vec(&legacy).unwrap();
        let admitted = WorkbenchCheckpoint::parse(&bytes).unwrap();
        let managed = admitted.managed_names().to_vec();
        let mut owner = admitted.reconstruct(site(300), &worker(400)).unwrap();
        let navigation = &owner.snapshot().navigation;
        assert_eq!(
            navigation
                .workspaces
                .iter()
                .map(|w| w.workspace_id.as_str())
                .collect::<Vec<_>>(),
            [4, 24, 35].map(identity)
        );
        assert_eq!(
            navigation
                .workspace_order
                .iter()
                .map(|id| id.as_str())
                .collect::<Vec<_>>(),
            [4, 35, 24].map(identity),
            "historical Project traversal, version {version}"
        );
        assert_eq!(
            navigation
                .directories
                .iter()
                .map(|d| d.directory_id.as_str())
                .collect::<Vec<_>>(),
            [3, 26, 27, 29].map(identity)
        );
        assert_eq!(
            navigation.workspaces[0]
                .project_id
                .as_ref()
                .unwrap()
                .as_str(),
            identity(2)
        );
        assert_eq!(
            navigation.workspaces[1]
                .project_id
                .as_ref()
                .unwrap()
                .as_str(),
            identity(25)
        );
        assert!(
            navigation
                .directories
                .iter()
                .all(|d| d.worker_id == worker(400))
        );
        assert_eq!(owner.last_identity, 50);
        let targets = owner.restoration_targets(&worker(400)).unwrap();
        let target = targets
            .iter()
            .find(|target| target.presentation_id().as_str() == identity(11))
            .unwrap();
        assert_eq!(
            target.working_directory().unwrap().as_str(),
            if version >= 3 {
                "E:/restart-cwd"
            } else {
                "C:\\work space\\项目"
            }
        );
        assert_eq!(target.agent_session().is_some(), version >= 2);
        assert_eq!(managed.len(), usize::from(version >= 4));

        let before = owner.snapshot().clone();
        let upgraded = owner
            .checkpoint_with_managed_agents(&worker(400), &[], &[], &managed)
            .unwrap();
        assert_eq!(owner.snapshot(), &before, "capture is pure");
        let value: Value = serde_json::from_slice(&upgraded).unwrap();
        assert_eq!(value["version"], 6);
        assert!(
            value["navigation"]["projects"][0]
                .get("workspaces")
                .is_none()
        );
        assert_eq!(
            value["navigation"]["workspaceOrder"],
            json!([identity(4), identity(35), identity(24)])
        );
        assert_eq!(restore(&upgraded).unwrap().snapshot(), &before);
        assert_eq!(
            WorkbenchCheckpoint::parse(&upgraded)
                .unwrap()
                .managed_names(),
            managed
        );
        assert!(
            matches!(command(&mut owner, WorkbenchAction::ProjectCreate {
            label: WorkbenchLabel::parse("after restart").unwrap()
        }), WorkbenchCommandOutcome::ProjectCreated { project_id, .. } if project_id.as_str() == identity(51))
        );
    }
}

#[test]
fn historical_explicit_order_survives_flattening_in_every_supported_version() {
    for version in 1..=5 {
        let mut value = historical(version);
        value["navigation"]["workspaceOrder"] = json!([identity(24), identity(35), identity(4)]);
        let owner = restore(&serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(
            owner
                .snapshot()
                .navigation
                .workspace_order
                .iter()
                .map(|id| id.as_str())
                .collect::<Vec<_>>(),
            [24, 35, 4].map(identity)
        );
    }
}

#[test]
fn historical_validation_rejects_cross_project_references_and_unsorted_arrays_before_flattening() {
    for version in 1..=5 {
        for fault in 0..4 {
            let mut value = historical(version);
            let first = &mut value["navigation"]["projects"][0];
            match fault {
                0 => first["workspaces"][0]["primaryDirectoryId"] = json!(identity(26)),
                1 => first["directories"].as_array_mut().unwrap().reverse(),
                2 => first["workspaces"].as_array_mut().unwrap().reverse(),
                _ => first["workspaces"][0]["projectId"] = json!(identity(2)),
            }
            assert!(
                matches!(
                    WorkbenchCheckpoint::parse(&serde_json::to_vec(&value).unwrap()),
                    Err(WorkbenchCheckpointError::InvalidDocument)
                ),
                "version {version}, fault {fault}"
            );
        }
    }
}

#[test]
fn navigation_versions_duplicates_and_high_water_never_fall_back_to_empty_success() {
    for version in 1..=6 {
        for fault in 0..6 {
            let mut value = if version == 6 {
                serde_json::from_slice(&checkpoint()).unwrap()
            } else {
                historical(version)
            };
            match fault {
                0 => value["version"] = json!(if version == 6 { 5 } else { 6 }),
                1 => value["lastIdentity"] = json!(identity(2)),
                2 => {
                    let workspaces = if version == 6 {
                        &mut value["navigation"]["workspaces"]
                    } else {
                        &mut value["navigation"]["projects"][0]["workspaces"]
                    };
                    let duplicate = workspaces[0].clone();
                    workspaces.as_array_mut().unwrap().push(duplicate);
                }
                3 => {
                    if version == 6 {
                        value["navigation"]["workspaces"][0]
                            .as_object_mut()
                            .unwrap()
                            .remove("projectId");
                    } else {
                        value["navigation"]["projects"][1]["directories"][0]["directoryId"] =
                            json!(identity(3));
                        value["navigation"]["projects"][1]["workspaces"][0]["primaryDirectoryId"] =
                            json!(identity(3));
                    }
                }
                4 => value["terminalContexts"][0]["terminalContextId"] = json!(identity(2)),
                _ => value["navigation"]["workspaceOrder"] = json!([identity(4), identity(4)]),
            }
            assert!(
                WorkbenchCheckpoint::parse(&serde_json::to_vec(&value).unwrap()).is_err(),
                "version {version}, fault {fault}"
            );
        }
    }
}

#[test]
fn version_six_requires_an_explicit_complete_workspace_order() {
    let bytes = checkpoint();
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    let workspace = value["navigation"]["workspaces"][0]["workspaceId"].clone();
    assert_eq!(value["navigation"]["workspaceOrder"], json!([workspace]));
    for omit in [false, true] {
        let mut invalid = value.clone();
        if omit {
            invalid["navigation"]
                .as_object_mut()
                .unwrap()
                .remove("workspaceOrder");
        } else {
            invalid["navigation"]["workspaceOrder"] = json!([]);
        }
        assert!(
            WorkbenchCheckpoint::parse(&serde_json::to_vec(&invalid).unwrap()).is_err(),
            "version 6 must reject {} order for a retained Workspace",
            if omit { "omitted" } else { "empty" }
        );
    }

    let empty = WorkbenchAuthority::new(site(100)).unwrap();
    let value: Value = serde_json::from_slice(&empty.checkpoint(&worker(200)).unwrap()).unwrap();
    assert_eq!(value["navigation"]["workspaceOrder"], json!([]));
}

#[test]
fn historical_per_project_directory_caps_are_checked_before_global_normalization() {
    for version in 1..=5 {
        let mut value = historical(version);
        value["lastIdentity"] = json!(identity(2000));
        let directories = value["navigation"]["projects"][0]["directories"]
            .as_array_mut()
            .unwrap();
        directories.truncate(1);
        directories.extend((0..255).map(|index| {
            json!({
                "directoryId": identity(1000 + index), "path": "C:/unused"
            })
        }));
        assert!(WorkbenchCheckpoint::parse(&serde_json::to_vec(&value).unwrap()).is_ok());
        value["navigation"]["projects"][0]["directories"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "directoryId": identity(1255), "path": "C:/overflow"
            }));
        assert!(
            matches!(
                WorkbenchCheckpoint::parse(&serde_json::to_vec(&value).unwrap()),
                Err(WorkbenchCheckpointError::InvalidDocument)
            ),
            "version {version}"
        );
    }
}

#[test]
fn current_checkpoint_preserves_nullable_organization_and_shared_directories_after_dissolve() {
    let mut owner = fixture();
    let mut source = graph(site(100), &worker(200), true, 99);
    source["navigation"]["projects"]
        .as_array_mut()
        .unwrap()
        .push(json!({"projectId": identity(25), "label": "retained group"}));
    source["navigation"]["directories"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "directoryId": identity(26), "workerId": worker(200), "path": "D:/unused"
        }));
    source["navigation"]["workspaces"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "workspaceId": identity(24), "projectId": identity(25), "label": "shared directory",
            "primaryDirectoryId": identity(3), "terminalContextId": identity(28)
        }));
    source["navigation"]["workspaceOrder"] = json!([identity(24), identity(4)]);
    source["terminalContexts"]
        .as_array_mut()
        .unwrap()
        .push(json!({"terminalContextId": identity(28), "groups": [], "tabs": []}));
    owner.projection =
        WorkbenchProjectionStore::load(&serde_json::to_vec(&source).unwrap()).unwrap();
    let workspace = owner.snapshot().navigation.workspaces[0]
        .workspace_id
        .clone();
    let project = owner.snapshot().navigation.projects[0].project_id.clone();
    assert!(matches!(
        command(
            &mut owner,
            WorkbenchAction::ProjectDissolve {
                project_id: project
            }
        ),
        WorkbenchCommandOutcome::ProjectDissolved { .. }
    ));
    let bytes = owner.checkpoint(&worker(200)).unwrap();
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["version"], 6);
    assert!(
        value["navigation"]["workspaces"][0]
            .get("projectId")
            .unwrap()
            .is_null()
    );
    assert_eq!(
        value["navigation"]["projects"],
        json!([
            {"projectId": identity(25), "label": "retained group"}
        ])
    );
    assert_eq!(
        value["navigation"]["workspaces"][1]["projectId"],
        json!(identity(25))
    );
    assert_eq!(
        value["navigation"]["directories"].as_array().unwrap().len(),
        2
    );
    let restarted = restore(&bytes).unwrap();
    let mut expected_navigation = owner.snapshot().navigation.clone();
    for directory in &mut expected_navigation.directories {
        directory.worker_id = worker(400);
    }
    assert_eq!(restarted.snapshot().navigation, expected_navigation);
    assert_eq!(
        restarted
            .snapshot()
            .navigation
            .workspace(&workspace)
            .unwrap()
            .project_id,
        None
    );
    assert_eq!(
        restarted.restoration_targets(&worker(400)).unwrap()[0]
            .working_directory()
            .unwrap()
            .as_str(),
        "C:\\work space\\项目"
    );
    assert_eq!(restarted.checkpoint(&worker(400)).unwrap(), bytes);
}
