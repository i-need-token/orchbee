use super::*;

#[test]
fn workspace_numbers_survive_filtering_and_counts_exclude_child_presentations() {
    let mut state = snapshot();
    state.navigation.workspace_order = vec![
        state.navigation.workspaces[1].workspace_id.clone(),
        state.navigation.workspaces[0].workspace_id.clone(),
    ];
    let all = project(&QueryCommand::WorkspaceList { project: None }, &state)
        .unwrap()
        .result();
    assert_eq!(all["workspaces"][0]["workspaceId"], hex(0x41));
    assert_eq!(all["workspaces"][0]["number"], 1);
    assert_eq!(all["workspaces"][1]["number"], 2);
    let filtered = project(
        &QueryCommand::WorkspaceList {
            project: Some(Target::Id(hex(0x10))),
        },
        &state,
    )
    .unwrap()
    .result();
    assert_eq!(filtered["workspaces"][0], all["workspaces"][1]);
    let get = project(&QueryCommand::WorkspaceGet(Target::Id(hex(0x40))), &state)
        .unwrap()
        .result();
    for key in ["number", "tabCount", "panelCount"] {
        assert_eq!(get["workspace"][key], filtered["workspaces"][0][key]);
    }
    assert_eq!(get["workspace"]["tabCount"], 1);
    assert_eq!(get["workspace"]["panelCount"], 2);
    state.terminal_contexts[1].tabs.clear();
    let empty = project(&QueryCommand::WorkspaceGet(Target::Id(hex(0x40))), &state)
        .unwrap()
        .result();
    assert_eq!(empty["workspace"]["tabCount"], 0);
    assert_eq!(empty["workspace"]["panelCount"], 0);
    assert_eq!(empty["workspace"]["number"], 2);
}

#[test]
fn tab_numbers_follow_context_order_and_leaf_counts_follow_the_tree() {
    let mut state = snapshot();
    state.terminal_contexts[0].tabs.rotate_left(1);
    let all = project(
        &QueryCommand::TabList {
            workspace: None,
            standalone: false,
        },
        &state,
    )
    .unwrap()
    .result();
    for (position, id) in DUP_TABS
        .iter()
        .cycle()
        .skip(1)
        .take(DUP_TABS.len())
        .enumerate()
    {
        assert_eq!(all["tabs"][position]["tabId"], hex(*id));
        assert_eq!(all["tabs"][position]["number"], position + 1);
        assert_eq!(all["tabs"][position]["panelCount"], 1);
    }
    assert_eq!(all["tabs"][9]["number"], 1);
    assert_eq!(all["tabs"][9]["panelCount"], 2);
    let get = project(&QueryCommand::TabGet(Target::Id(hex(0x52))), &state)
        .unwrap()
        .result();
    assert_eq!(get["tab"]["number"], 1);
    assert_eq!(get["tab"]["panelCount"], 2);
}

#[test]
fn panel_lists_preserve_primary_labels_and_workspace_identity() {
    let mut state = snapshot();
    let PanelNode::Split { children, .. } = &mut state.terminal_contexts[1].tabs[0].root else {
        panic!("split fixture");
    };
    let PanelNode::Panel {
        primary_presentation,
        ..
    } = &mut children[1].node
    else {
        panic!("panel fixture");
    };
    primary_presentation.label = "shell\u{1b}[31m\n中文".into();
    primary_presentation.custom_label =
        Some(orchbee_workbench_protocol::PanelCustomLabel::parse("名称").unwrap());
    let all = project(&QueryCommand::PanelList { tab: None }, &state)
        .unwrap()
        .result();
    assert!(all["panels"][0]["workspaceId"].is_null());
    let filtered = project(
        &QueryCommand::PanelList {
            tab: Some(Target::Id(hex(0x52))),
        },
        &state,
    )
    .unwrap()
    .result();
    let row = &filtered["panels"][1];
    assert_eq!(row["workspaceId"], hex(0x40));
    assert_eq!(row["label"], "shell\u{1b}[31m\n中文");
    assert_eq!(row["customLabel"], "名称");
    assert_eq!(row["displayLabel"], "名称");
    let get = project(&QueryCommand::PanelGet(Target::Id(hex(0x58))), &state)
        .unwrap()
        .result();
    for key in ["label", "customLabel", "displayLabel"] {
        assert_eq!(row[key], get["panel"]["primaryPresentation"][key]);
    }
}

#[test]
fn topology_documents_validate_and_text_escapes_labels_in_both_locales() {
    use crate::{invocation::OutputFormat, localization::Locale};
    let mut state = snapshot();
    let PanelNode::Panel {
        primary_presentation,
        ..
    } = &mut state.terminal_contexts[0].tabs[0].root
    else {
        panic!("panel fixture");
    };
    primary_presentation.custom_label =
        Some(orchbee_workbench_protocol::PanelCustomLabel::parse("中文\u{1b}[31m名").unwrap());
    for command in [
        QueryCommand::WorkspaceList { project: None },
        QueryCommand::WorkspaceGet(Target::Id(hex(0x40))),
        QueryCommand::TabList {
            workspace: None,
            standalone: false,
        },
        QueryCommand::TabGet(Target::Id(hex(DUP_TABS[0]))),
        QueryCommand::PanelList { tab: None },
    ] {
        let published =
            crate::cli::schema::answer(Some(schema_id(&command)), OutputFormat::Json, Locale::EnUs);
        let catalog: Value = serde_json::from_slice(&published.stdout).unwrap();
        let schema = &catalog["result"]["schema"]["document"];
        let validator = jsonschema::validator_for(schema).unwrap();
        let output = crate::cli::success_output(
            project(&command, &state).unwrap(),
            snapshot_source(&state),
            &command,
            OutputFormat::Json,
            Locale::EnUs,
        );
        assert_eq!(output.status, 0);
        let mut document: Value = serde_json::from_slice(&output.stdout).unwrap();
        #[cfg(windows)]
        if matches!(command, QueryCommand::PanelList { .. }) {
            // This suite owns the Snapshot-only v7 projection. The joined v9
            // document is validated by workbench_query's selected-query suite.
            assert!(!validator.is_valid(&document));
            document["version"] = json!(7);
        }
        validator.validate(&document).unwrap();
        let mut wrong_version = document.clone();
        wrong_version["version"] = json!(0);
        assert!(!validator.is_valid(&wrong_version));
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let text = crate::cli::success_output(
                project(&command, &state).unwrap(),
                snapshot_source(&state),
                &command,
                OutputFormat::Text,
                locale,
            );
            assert_eq!(text.status, 0);
            assert!(!text.stdout.contains(&0x1b));
            if matches!(command, QueryCommand::PanelList { .. }) {
                assert!(
                    String::from_utf8(text.stdout)
                        .unwrap()
                        .contains("\\u{001b}")
                );
            }
        }
    }
}

#[test]
fn workspace_grouping_is_nullable_metadata_and_global_scope_survives_regrouping() {
    use crate::{invocation::OutputFormat, localization::Locale};
    let mut state = snapshot();
    let command = QueryCommand::WorkspaceGet(Target::Id(hex(0x40)));
    let before = project(&command, &state).unwrap().result();
    for destination in [None, Some(state.navigation.projects[1].project_id.clone())] {
        state.navigation.workspaces[0].project_id = destination.clone();
        let after = project(&command, &state).unwrap().result();
        for key in [
            "workspaceId",
            "terminalContextId",
            "primaryDirectoryId",
            "number",
            "tabCount",
            "panelCount",
        ] {
            assert_eq!(before["workspace"][key], after["workspace"][key], "{key}");
        }
        assert_eq!(after["workspace"]["projectId"], json!(destination));
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let output = crate::cli::success_output(
                project(&command, &state).unwrap(),
                snapshot_source(&state),
                &command,
                OutputFormat::Text,
                locale,
            );
            assert_eq!(output.status, 0);
            if destination.is_none() {
                assert!(
                    String::from_utf8(output.stdout)
                        .unwrap()
                        .contains(locale.cli_ungrouped())
                );
            }
        }
        let output = crate::cli::success_output(
            project(&command, &state).unwrap(),
            snapshot_source(&state),
            &command,
            OutputFormat::Json,
            Locale::EnUs,
        );
        let mut doc: Value = serde_json::from_slice(&output.stdout).unwrap();
        let schema: Value =
            serde_json::from_str(include_str!("../../schemas/orchbee.workspace.get.json")).unwrap();
        let validator = jsonschema::validator_for(&schema).unwrap();
        validator.validate(&doc).unwrap();
        doc["result"]["workspace"]
            .as_object_mut()
            .unwrap()
            .remove("projectId");
        assert!(
            !validator.is_valid(&doc),
            "nullable grouping must still be explicit"
        );
    }
    for workspace in &mut state.navigation.workspaces {
        workspace.project_id = None;
    }
    state.navigation.projects.clear();
    assert_eq!(
        project(&QueryCommand::WorkspaceList { project: None }, &state)
            .unwrap()
            .result()["workspaces"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        resolve_workspace(&state, &Target::Label("ws".into()))
            .unwrap_err()
            .kind,
        FailureKind::Ambiguous
    );
}
