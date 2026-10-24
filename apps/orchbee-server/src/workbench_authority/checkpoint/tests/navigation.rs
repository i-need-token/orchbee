use super::*;

#[test]
fn project_order_round_trips_and_is_required() {
    use orchbee_workbench_protocol::NavigationConfiguration;
    let mut owner = fixture();
    let first = owner.snapshot().navigation.projects[0].project_id.clone();
    let WorkbenchCommandOutcome::ProjectCreated {
        project_id: second, ..
    } = command(
        &mut owner,
        WorkbenchAction::ProjectCreate {
            label: WorkbenchLabel::parse("Second").unwrap(),
        },
    )
    else {
        panic!("create Project")
    };
    assert!(matches!(
        command(
            &mut owner,
            WorkbenchAction::NavigationConfigure {
                operation: NavigationConfiguration::MoveProject {
                    project_id: second.clone(),
                    before_project_id: Some(first.clone())
                }
            }
        ),
        WorkbenchCommandOutcome::NavigationConfigured { .. }
    ));
    let bytes = owner.checkpoint(&worker(200)).unwrap();
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["version"], 1);
    assert_eq!(value["navigation"]["projectOrder"], json!([second, first]));
    let restored = restore(&bytes).unwrap();
    assert_eq!(
        restored.snapshot().navigation.project_order,
        owner.snapshot().navigation.project_order
    );
    assert_eq!(
        restored.snapshot().navigation.projects,
        owner.snapshot().navigation.projects
    );
    let again: Value = serde_json::from_slice(&restored.checkpoint(&worker(400)).unwrap()).unwrap();
    assert_eq!(
        again["navigation"]["projectOrder"],
        value["navigation"]["projectOrder"]
    );
    for order in [
        json!([first.clone()]),
        json!([first.clone(), first.clone()]),
        json!([first.clone(), identity(999)]),
    ] {
        let mut invalid = value.clone();
        invalid["navigation"]["projectOrder"] = order;
        assert!(WorkbenchCheckpoint::parse(&serde_json::to_vec(&invalid).unwrap()).is_err());
    }
    let mut missing = value;
    missing["navigation"]
        .as_object_mut()
        .unwrap()
        .remove("projectOrder");
    assert!(WorkbenchCheckpoint::parse(&serde_json::to_vec(&missing).unwrap()).is_err());
}

#[test]
fn navigation_version_shape_duplicates_and_high_water_reject_the_whole_document() {
    for fault in 0..7 {
        let bytes = changed(|value| match fault {
            0 => value["version"] = json!(7),
            1 => value["lastIdentity"] = json!(identity(2)),
            2 => {
                let duplicate = value["navigation"]["workspaces"][0].clone();
                value["navigation"]["workspaces"]
                    .as_array_mut()
                    .unwrap()
                    .push(duplicate);
            }
            3 => {
                value["navigation"]["workspaces"][0]
                    .as_object_mut()
                    .unwrap()
                    .remove("projectId");
            }
            4 => value["terminalContexts"][0]["terminalContextId"] = json!(identity(2)),
            5 => value["navigation"]["workspaceOrder"] = json!([identity(4), identity(4)]),
            _ => value["navigation"]["projects"][0]["workspaces"] = json!([]),
        });
        assert!(WorkbenchCheckpoint::parse(&bytes).is_err(), "fault {fault}");
    }
}

#[test]
fn normalized_navigation_requires_an_explicit_complete_workspace_order() {
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
            "normalized navigation must reject {} order for a retained Workspace",
            if omit { "omitted" } else { "empty" }
        );
    }

    let empty = WorkbenchAuthority::new(site(100)).unwrap();
    let value: Value = serde_json::from_slice(&empty.checkpoint(&worker(200)).unwrap()).unwrap();
    assert_eq!(value["navigation"]["workspaceOrder"], json!([]));
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
    assert_eq!(value["version"], 1);
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
