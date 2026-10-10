use orchbee_workbench_protocol::{
    ProtocolVersion, SiteId, WorkbenchDirectoryPath, WorkbenchRequest, WorkbenchRequestId,
    WorkbenchResponse, WorkbenchTerminalGeometry, WorkbenchWorktreeAction as Action,
    WorkbenchWorktreeOutcome as Outcome, WorkbenchWorktreeRequest as Request,
    WorkbenchWorktreeResult as ResultDocument, WorktreeGitEffect, WorktreeGitError,
    WorktreeGitStage, WorktreeIntent, WorktreeOperationFailure, WorktreeOperationPhase,
    WorktreeOperationStatus, WorktreeProgress, WorktreeReference, WorktreeSelector, WorktreeSource,
    WorktreeWorkspaceDisposition, parse_workbench_request, parse_workbench_response,
    read_workbench_request, read_workbench_response, serialize_workbench_request,
    serialize_workbench_response, write_workbench_request, write_workbench_response,
};

#[test]
fn shared_worktree_corpus_keeps_rust_and_typescript_semantics_aligned() {
    let corpus: serde_json::Value = serde_json::from_str(include_str!(
        "../../../schemas/workbench/v1/workbench-worktree-conformance-v1.json"
    ))
    .unwrap();
    for (group, expected) in [("valid", true), ("invalid", false)] {
        for case in corpus[group].as_array().unwrap() {
            let bytes = serde_json::to_vec(&case["document"]).unwrap();
            let accepted = match case["direction"].as_str().unwrap() {
                "request" => parse_workbench_request(&bytes).is_ok(),
                "response" => parse_workbench_response(&bytes).is_ok(),
                _ => panic!("corpus direction"),
            };
            assert_eq!(accepted, expected, "{}", case["name"]);
        }
    }
    for case in corpus["validJson"].as_array().unwrap() {
        let bytes = case["json"].as_str().unwrap().as_bytes();
        let canonical = if case["direction"] == "request" {
            serde_json::to_value(parse_workbench_request(bytes).unwrap()).unwrap()
        } else {
            serde_json::to_value(parse_workbench_response(bytes).unwrap()).unwrap()
        };
        assert_eq!(canonical, case["canonical"], "{}", case["name"]);
    }
}
fn id() -> WorkbenchRequestId {
    WorkbenchRequestId::parse("11111111111111111111111111111111").unwrap()
}
fn request(action: Action) -> WorkbenchRequest {
    WorkbenchRequest::WorktreeOperation(Request {
        protocol_version: ProtocolVersion,
        request_id: id(),
        site_id: SiteId::parse("33333333333333333333333333333333").unwrap(),
        action,
    })
}
fn source() -> WorktreeSource {
    WorktreeSource::Directory {
        path: WorkbenchDirectoryPath::parse("C:/repo/subdir").unwrap(),
    }
}
fn progress() -> WorktreeProgress {
    WorktreeProgress {
        path: None,
        checkout: None,
        workspace: WorktreeWorkspaceDisposition::Unassociated {},
        git_effect: WorktreeGitEffect::None,
        restoration: None,
    }
}
fn response(status: WorktreeOperationStatus) -> WorkbenchResponse {
    WorkbenchResponse::WorktreeOperation(ResultDocument {
        protocol_version: ProtocolVersion,
        request_id: id(),
        operation_id: id(),
        outcome: Outcome::Available {
            project_id: None,
            attempt: 1,
            status: Box::new(status),
        },
    })
}
#[test]
fn request_variants_preserve_identity_intent_and_strict_frame_direction() {
    let geometry = WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap();
    let workspace_id =
        orchbee_workbench_protocol::WorkbenchWorkspaceId::parse("44444444444444444444444444444444")
            .unwrap();
    let mut actions = vec![
        Action::Read { operation_id: id() },
        Action::Resume {
            operation_id: id(),
            expected_attempt: 64,
        },
    ];
    for intent in [
        WorktreeIntent::Create {
            source: source(),
            branch: Some(WorktreeReference::parse("topic/一").unwrap()),
            base: None,
            path: None,
            label: None,
            geometry,
            trust_repository: true,
        },
        WorktreeIntent::Open {
            source: WorktreeSource::Workspace {
                workspace_id: workspace_id.clone(),
            },
            selector: WorktreeSelector::Branch {
                branch: WorktreeReference::parse("main").unwrap(),
            },
            label: None,
            geometry,
            trust_repository: false,
        },
        WorktreeIntent::Open {
            source: source(),
            selector: WorktreeSelector::Path {
                path: WorkbenchDirectoryPath::parse("C:/w").unwrap(),
            },
            label: None,
            geometry,
            trust_repository: false,
        },
        WorktreeIntent::Remove {
            workspace_id,
            force: true,
            trust_repository: false,
        },
    ] {
        actions.push(Action::Start {
            operation_id: id(),
            project_id: None,
            intent,
        });
    }
    for action in actions {
        let message = request(action);
        let encoded = serialize_workbench_request(&message).unwrap();
        assert_eq!(parse_workbench_request(&encoded).unwrap(), message);
        let mut frame = vec![];
        write_workbench_request(&mut frame, &message).unwrap();
        assert_eq!(&frame[..8], b"OBWP\0\x01\0\x13");
        assert_eq!(
            read_workbench_request(&mut frame.as_slice()).unwrap(),
            message
        );
        assert!(read_workbench_response(&mut frame.as_slice()).is_err());
        for length in 0..frame.len() {
            assert!(read_workbench_request(&mut &frame[..length]).is_err());
        }
        let mut json: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
        json["action"]["unexpected"] = true.into();
        assert!(parse_workbench_request(&serde_json::to_vec(&json).unwrap()).is_err());
    }
}
#[test]
fn resume_fences_and_reference_argv_lexicon_are_checked_on_parse_and_serialize() {
    for attempt in [0, 65, u32::MAX] {
        let message = request(Action::Resume {
            operation_id: id(),
            expected_attempt: attempt,
        });
        assert!(serialize_workbench_request(&message).is_err());
        let encoded = serde_json::to_vec(&message).unwrap();
        assert!(parse_workbench_request(&encoded).is_err());
    }
    for value in ["", "-branch", "bad\nbranch", "bad\u{7f}"] {
        assert!(WorktreeReference::parse(value).is_err());
    }
    assert!(WorktreeReference::parse(&"a".repeat(1024)).is_ok());
    assert!(WorktreeReference::parse(&"a".repeat(1025)).is_err());
    assert!(WorktreeReference::parse("feature/开发").is_ok());
}
#[test]
fn result_failures_preserve_raw_diagnostics_without_accepting_invalid_combinations() {
    let status = WorktreeOperationStatus::Failed {
        reason: WorktreeOperationFailure::Git {
            stage: WorktreeGitStage::Mutate,
            error: WorktreeGitError::GitFailed,
            status: Some(128),
            diagnostics: vec![0, 255, 27],
        },
        progress: WorktreeProgress {
            git_effect: WorktreeGitEffect::MayHaveChanged,
            restoration: None,
            ..progress()
        },
        resumable: true,
    };
    let message = response(status);
    let encoded = serialize_workbench_response(&message).unwrap();
    assert_eq!(parse_workbench_response(&encoded).unwrap(), message);
    let mut frame = vec![];
    write_workbench_response(&mut frame, &message).unwrap();
    assert_eq!(&frame[..8], b"OBWP\0\x01\0\x14");
    assert_eq!(
        read_workbench_response(&mut frame.as_slice()).unwrap(),
        message
    );
    assert!(read_workbench_request(&mut frame.as_slice()).is_err());
    let original: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
    for size in [2049, 10000] {
        let mut value = original.clone();
        value["outcome"]["status"]["reason"]["diagnostics"] = serde_json::json!(vec![255; size]);
        assert!(parse_workbench_response(&serde_json::to_vec(&value).unwrap()).is_err());
    }
    let mut value = original.clone();
    value["outcome"]["status"]["reason"]["error"] = "dirty".into();
    assert!(parse_workbench_response(&serde_json::to_vec(&value).unwrap()).is_err());
    let mut value = original;
    value["outcome"]["attempt"] = 0.into();
    assert!(parse_workbench_response(&serde_json::to_vec(&value).unwrap()).is_err());
}
#[test]
fn oversized_frames_and_inconsistent_checkout_paths_are_rejected() {
    for kind in [19_u16, 20] {
        let mut frame = b"OBWP\0\x01".to_vec();
        frame.extend(kind.to_be_bytes());
        frame.extend(
            ((orchbee_workbench_protocol::MAXIMUM_ENCODED_WORKTREE_OPERATION_BYTES + 1) as u32)
                .to_be_bytes(),
        );
        if kind == 19 {
            assert!(matches!(
                read_workbench_request(&mut frame.as_slice()),
                Err(orchbee_workbench_protocol::WorkbenchWireError::Contract(_))
            ));
        } else {
            assert!(matches!(
                read_workbench_response(&mut frame.as_slice()),
                Err(orchbee_workbench_protocol::WorkbenchWireError::Contract(_))
            ));
        }
    }
    let message = response(WorktreeOperationStatus::Pending {
        phase: WorktreeOperationPhase::Creating,
        progress: progress(),
    });
    let mut value = serde_json::to_value(message).unwrap();
    value["outcome"]["status"]["progress"]["checkout"] = serde_json::json!({"repositoryRoot":"C:/r", "commonDirectory":"C:/r/.git", "path":"C:/different", "branch":null, "dirty":false});
    assert!(parse_workbench_response(&serde_json::to_vec(&value).unwrap()).is_err());
}
