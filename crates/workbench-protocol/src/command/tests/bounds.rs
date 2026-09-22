use super::*;
use crate::{Navigation, Project, TerminalContext, Workspace};

fn near_limit_snapshot() -> Snapshot {
    let id = |value: u128| format!("{value:032x}");
    let standalone = WorkbenchTerminalContextId::parse(&id(1)).unwrap();
    let mut snapshot = Snapshot {
        protocol_version: ProtocolVersion,
        site_id: SiteId::parse(&id(10000)).unwrap(),
        revision: WorkbenchRevision::new(1),
        navigation: Navigation {
            projects: Vec::new(),
            standalone_terminal_context_id: standalone.clone(),
        },
        terminal_contexts: vec![TerminalContext {
            terminal_context_id: standalone,
            groups: Vec::new(),
            tabs: Vec::new(),
        }],
    };
    let mut next = 2;
    for _project in 0..2 {
        let mut project = Project {
            directories: Vec::new(),
            project_id: WorkbenchProjectId::parse(&id(next)).unwrap(),
            label: "P".to_owned(),
            workspaces: Vec::new(),
        };
        next += 1;
        for _workspace in 0..256 {
            let workspace_id = WorkbenchWorkspaceId::parse(&id(next)).unwrap();
            next += 1;
            let terminal_context_id = WorkbenchTerminalContextId::parse(&id(next)).unwrap();
            next += 1;
            project.workspaces.push(Workspace {
                workspace_id,
                terminal_context_id: terminal_context_id.clone(),
                label: "\u{1f41d}".repeat(512),
                primary_directory_id: None,
            });
            snapshot.terminal_contexts.push(TerminalContext {
                terminal_context_id,
                groups: Vec::new(),
                tabs: Vec::new(),
            });
        }
        snapshot.navigation.projects.push(project);
    }
    let encoded =
        serde_json::to_vec(&WorkbenchProtocolDocument::Snapshot(snapshot.clone())).unwrap();
    let mut excess = encoded.len() - MAXIMUM_ENCODED_PROJECTION_BYTES;
    for workspace in snapshot
        .navigation
        .projects
        .iter_mut()
        .flat_map(|project| &mut project.workspaces)
    {
        let removed = (excess / 4).min(511);
        workspace
            .label
            .truncate(workspace.label.len() - removed * 4);
        excess -= removed * 4;
        if excess > 0 && excess < 4 {
            workspace.label.pop();
            workspace.label.push(match excess {
                1 => '\u{20ac}',
                2 => '\u{a2}',
                3 => 'x',
                _ => unreachable!(),
            });
            excess = 0;
        }
        if excess == 0 {
            break;
        }
    }
    assert_eq!(
        serde_json::to_vec(&WorkbenchProtocolDocument::Snapshot(snapshot.clone()))
            .unwrap()
            .len(),
        MAXIMUM_ENCODED_PROJECTION_BYTES
    );
    snapshot
}

#[test]
fn nested_canonical_snapshot_and_raw_outer_result_enforce_distinct_exact_bounds() {
    let snapshot = near_limit_snapshot();
    let result = |snapshot| {
        WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse("11111111111111111111111111111111").unwrap(),
            outcome: WorkbenchSnapshotOutcome::Available { snapshot },
        })
    };
    let response = result(snapshot.clone());
    let mut encoded = serialize_workbench_response(&response).unwrap();
    encoded.resize(MAXIMUM_ENCODED_SNAPSHOT_RESULT_BYTES, b' ');
    assert_eq!(parse_workbench_response(&encoded).unwrap(), response);
    encoded.push(b' ');
    assert!(matches!(
        parse_workbench_response(&encoded),
        Err(ContractError::EncodedValueTooLarge {
            maximum: MAXIMUM_ENCODED_SNAPSHOT_RESULT_BYTES,
            ..
        })
    ));
    let mut too_large = snapshot;
    too_large.navigation.projects[0].label.push('x');
    let over_limit = result(too_large);
    let raw = serde_json::to_vec(&over_limit).unwrap();
    assert!(raw.len() < MAXIMUM_ENCODED_SNAPSHOT_RESULT_BYTES);
    assert!(matches!(
        parse_workbench_response(&raw),
        Err(ContractError::EncodedValueTooLarge {
            maximum: MAXIMUM_ENCODED_PROJECTION_BYTES,
            ..
        })
    ));
    assert!(serialize_workbench_response(&over_limit).is_err());
}

/// The worst legal launch selection: every aggregate byte is a control
/// character, which serde_json escapes to a six-byte `\uXXXX` sequence.
fn maximum_escape_launch() -> LaunchSelection {
    let mut arguments = vec!["\u{1}".repeat(15)];
    arguments.resize(MAXIMUM_LAUNCH_SELECTION_ARGUMENTS, "\u{1}".repeat(16));
    // 1-byte executable + 15 + 127×16 = exactly the aggregate bound.
    assert_eq!(
        1 + arguments.iter().map(String::len).sum::<usize>(),
        MAXIMUM_LAUNCH_SELECTION_BYTES
    );
    LaunchSelection::new("/".to_owned(), arguments).unwrap()
}

fn maximum_command(action: WorkbenchAction) -> WorkbenchRequest {
    WorkbenchRequest::Command(WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse("ffffffffffffffffffffffffffffffff").unwrap(),
        site_id: SiteId::parse("ffffffffffffffffffffffffffffffff").unwrap(),
        expected_revision: WorkbenchRevision::new(u64::MAX),
        action,
    })
}

#[test]
fn maximum_legal_launch_commands_fit_the_intent_bound() {
    // Worst legal encoding: a 512-character label of control characters
    // (six wire bytes each) plus a full 2048-byte launch selection of
    // escapes. Both actions must serialize inside the shared intent bound
    // — a legal typed command may never fail the envelope limit.
    let label = WorkbenchLabel::parse(&"\u{1}".repeat(MAXIMUM_LABEL_LENGTH)).unwrap();
    let geometry = WorkbenchTerminalGeometry::new(4096, 64, 15, 1023).unwrap();
    let context = WorkbenchTerminalContextId::parse("ffffffffffffffffffffffffffffffff").unwrap();
    let options = crate::LaunchOptions {
        cwd: Some(WorkbenchDirectoryPath::parse(&format!("/{}", "\u{1}".repeat(4095))).unwrap()),
        environment: crate::LaunchEnvironment::new({
            let mut entries: Vec<_> = (0..32).map(|n| (format!("K{n}"), String::new())).collect();
            let names: usize = entries.iter().map(|(name, _)| name.len()).sum();
            entries[0].1 = "\u{1}".repeat(1024 - names);
            entries
        })
        .unwrap(),
    };
    let commands = [
        maximum_command(WorkbenchAction::TabCreate {
            terminal_context_id: context.clone(),
            label: label.clone(),
            geometry,
            launch: Some(maximum_escape_launch()),
            profile: None,
            options: Box::new(options.clone()),
        }),
        maximum_command(WorkbenchAction::PanelSplit {
            terminal_context_id: context,
            tab_id: WorkbenchTabId::parse("ffffffffffffffffffffffffffffffff").unwrap(),
            panel_id: WorkbenchPanelId::parse("ffffffffffffffffffffffffffffffff").unwrap(),
            direction: WorkbenchPanelSplitDirection::Below,
            label,
            geometry,
            launch: Some(maximum_escape_launch()),
            profile: None,
            options: Box::new(options.clone()),
        }),
    ];
    for command in commands {
        let encoded = serialize_workbench_request(&command).unwrap();
        eprintln!("worst-case command encodes to {} bytes", encoded.len());
        assert!(
            encoded.len() <= MAXIMUM_ENCODED_LAUNCH_COMMAND_BYTES,
            "legal command encodes to {} bytes",
            encoded.len()
        );
        // The case must exercise real pressure, not a small fixture.
        assert!(
            encoded.len() > MAXIMUM_ENCODED_LAUNCH_COMMAND_BYTES - 4096,
            "maximum fixture encodes to only {} bytes",
            encoded.len()
        );
        assert_eq!(parse_workbench_request(&encoded).unwrap(), command);
        let mut framed = Vec::new();
        crate::write_workbench_request(&mut framed, &command).unwrap();
        assert_eq!(
            crate::read_workbench_request(&mut std::io::Cursor::new(framed)).unwrap(),
            command
        );
        let mut padded = encoded;
        padded.resize(MAXIMUM_ENCODED_LAUNCH_COMMAND_BYTES, b' ');
        assert!(parse_workbench_request(&padded).is_ok());
        padded.push(b' ');
        assert!(parse_workbench_request(&padded).is_err());
    }
}

#[test]
fn launch_selection_enforces_its_lexical_bounds() {
    let launch = |executable: &str, arguments: &[&str]| {
        LaunchSelection::new(
            executable.to_owned(),
            arguments
                .iter()
                .map(|argument| (*argument).to_owned())
                .collect(),
        )
    };
    // Absolute executable; verbatim arguments including empty, leading-dash,
    // and terminator literals.
    assert!(launch("/bin/sh", &[]).is_ok());
    assert!(launch("/bin/sh", &["", "-c", "--", "--flag=1", "漢字"]).is_ok());
    // Executable shape: empty, relative, and NUL-bearing all fail.
    assert!(launch("", &[]).is_err());
    assert!(launch("bin/sh", &[]).is_err());
    assert!(launch("sh", &[]).is_err());
    assert!(launch("/bin/\u{0}", &[]).is_err());
    // NUL inside any argument fails.
    assert!(launch("/bin/sh", &["a\u{0}b"]).is_err());
    // Argument count: 128 admits, 129 rejects.
    let maximum: Vec<String> = vec![String::new(); MAXIMUM_LAUNCH_SELECTION_ARGUMENTS];
    assert!(LaunchSelection::new("/x".to_owned(), maximum).is_ok());
    let over = vec![String::new(); MAXIMUM_LAUNCH_SELECTION_ARGUMENTS + 1];
    assert!(LaunchSelection::new("/x".to_owned(), over).is_err());
    // Aggregate bound: exact fit admits, one byte over rejects.
    assert!(launch("/x", &["a"]).is_ok_and(|selection| {
        selection.executable() == "/x" && selection.arguments() == ["a"]
    }));
    let fit = MAXIMUM_LAUNCH_SELECTION_BYTES - 2;
    assert!(launch("/x", &["a".repeat(fit).as_str()]).is_ok());
    assert!(launch("/x", &["a".repeat(fit + 1).as_str()]).is_err());
}

#[test]
fn launch_selection_wire_shape_is_closed_and_default_omits_the_key() {
    let context = WorkbenchTerminalContextId::parse("33333333333333333333333333333333").unwrap();
    let geometry = WorkbenchTerminalGeometry::new(80, 24, 0, 0).unwrap();
    let command = |launch| {
        WorkbenchRequest::Command(WorkbenchCommand {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse("11111111111111111111111111111111").unwrap(),
            site_id: SiteId::parse("22222222222222222222222222222222").unwrap(),
            expected_revision: WorkbenchRevision::new(0),
            action: WorkbenchAction::TabCreate {
                terminal_context_id: context.clone(),
                label: WorkbenchLabel::parse("t").unwrap(),
                geometry,
                launch,
                profile: None,
                options: Default::default(),
            },
        })
    };
    // Absent selection serializes no `launch` key at all.
    let encoded = serialize_workbench_request(&command(None)).unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&encoded).unwrap()["action"]["launch"],
        Value::Null
    );
    // Present selection emits {"exec","args"} and round-trips verbatim.
    let with_launch = command(Some(
        LaunchSelection::new("/bin/sh".to_owned(), vec!["-c".to_owned(), "".to_owned()]).unwrap(),
    ));
    let encoded = serialize_workbench_request(&with_launch).unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&encoded).unwrap()["action"]["launch"],
        serde_json::json!({"exec": "/bin/sh", "args": ["-c", ""]})
    );
    assert_eq!(parse_workbench_request(&encoded).unwrap(), with_launch);
    // Malformed wire selections fail closed: relative executable, missing
    // `args`, unknown key — each is InvalidDocument before any effect.
    for launch_json in [
        r#"{"exec":"bin/sh","args":[]}"#,
        r#"{"exec":"/bin/sh"}"#,
        r#"{"exec":"/bin/sh","args":[],"extra":1}"#,
        r#"{"exec":"/bin/\u0000","args":[]}"#,
    ] {
        let raw = format!(
            r#"{{"kind":"workbench.command","protocolVersion":1,"requestId":"11111111111111111111111111111111","siteId":"22222222222222222222222222222222","expectedRevision":"0","action":{{"kind":"tab.create","terminalContextId":"33333333333333333333333333333333","label":"t","geometry":{{"columns":80,"rows":24,"cellPixelWidth":0,"cellPixelHeight":0}},"launch":{launch_json}}}}}"#
        );
        assert!(
            parse_workbench_request(raw.as_bytes()).is_err(),
            "accepted {launch_json}"
        );
    }
}
