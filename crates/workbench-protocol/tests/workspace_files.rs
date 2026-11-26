use orchbee_workbench_protocol::*;

fn request(operation: WorkspaceFilesOperation) -> WorkspaceFilesRequest {
    WorkspaceFilesRequest {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{:032x}", 1)).unwrap(),
        session_id: SessionId::parse(&format!("{:032x}", 2)).unwrap(),
        workspace_id: WorkbenchWorkspaceId::parse(&format!("{:032x}", 3)).unwrap(),
        operation,
    }
}

#[test]
fn workspace_files_shared_corpus_and_closed_values() {
    let corpus: serde_json::Value = serde_json::from_str(include_str!(
        "../../../schemas/workbench/v1/workbench-workspace-files-conformance-v1.json"
    ))
    .unwrap();
    for (key, expected) in [("valid", true), ("invalid", false)] {
        for case in corpus[key].as_array().unwrap() {
            let bytes = serde_json::to_vec(&case["document"]).unwrap();
            let actual = if case["direction"] == "request" {
                parse_workbench_request(&bytes).is_ok()
            } else {
                parse_workbench_response(&bytes).is_ok()
            };
            assert_eq!(actual, expected, "{}", case["name"]);
        }
    }
}

#[test]
fn workspace_files_distinct_carriers_and_operation_correlation() {
    let listing = request(WorkspaceFilesOperation::ListDirectory {
        path: String::new(),
    });
    let mut bytes = Vec::new();
    write_workbench_request(
        &mut bytes,
        &WorkbenchRequest::WorkspaceFiles(listing.clone()),
    )
    .unwrap();
    assert_eq!(&bytes[6..8], &83_u16.to_be_bytes());
    assert_eq!(
        read_workbench_request(&mut bytes.as_slice()).unwrap(),
        WorkbenchRequest::WorkspaceFiles(listing.clone())
    );
    let result = WorkspaceFilesResult {
        protocol_version: ProtocolVersion,
        request_id: listing.request_id.clone(),
        session_id: listing.session_id.clone(),
        workspace_id: listing.workspace_id.clone(),
        outcome: WorkspaceFilesOutcome::Directory {
            entries: vec![WorkspaceDirectoryEntry {
                name: "src".into(),
                entry_kind: WorkspaceEntryKind::Directory,
                symbolic_link: false,
                ignored: false,
            }],
            truncated: false,
        },
    };
    assert!(result.answers(&listing));
    bytes.clear();
    write_workbench_response(
        &mut bytes,
        &WorkbenchResponse::WorkspaceFiles(result.clone()),
    )
    .unwrap();
    assert_eq!(&bytes[6..8], &84_u16.to_be_bytes());
    assert_eq!(
        read_workbench_response(&mut bytes.as_slice()).unwrap(),
        WorkbenchResponse::WorkspaceFiles(result.clone())
    );
    // A listing never answers a file read, even with the same identity.
    let read = request(WorkspaceFilesOperation::ReadFile {
        path: "src".into(),
        source: WorkspaceFileSource::Worktree,
        offset: 0,
        encoding: WorkspaceFileEncoding::Text,
    });
    assert!(!result.answers(&read));
    // A chunk answers only the read of its own offset and encoding, so a late answer to an
    // earlier chunk cannot be stitched in at the wrong place.
    let chunk = WorkspaceFilesResult {
        outcome: WorkspaceFilesOutcome::File {
            byte_length: 4,
            version: None,
            content: WorkspaceFileContent::Text {
                offset: 0,
                text: "abcd".into(),
                next_offset: None,
            },
        },
        ..result
    };
    assert!(chunk.validate().is_ok());
    assert!(chunk.answers(&read));
    let later = request(WorkspaceFilesOperation::ReadFile {
        path: "src".into(),
        source: WorkspaceFileSource::Worktree,
        offset: 2,
        encoding: WorkspaceFileEncoding::Text,
    });
    assert!(!chunk.answers(&later));
    let as_bytes = request(WorkspaceFilesOperation::ReadFile {
        path: "src".into(),
        source: WorkspaceFileSource::Worktree,
        offset: 0,
        encoding: WorkspaceFileEncoding::Bytes,
    });
    assert!(!chunk.answers(&as_bytes));
}

#[test]
fn workspace_files_rejects_escaping_paths_before_any_effect() {
    for path in ["..", "a/../b", "/abs", "a//b", "a/", ".", "a\\b", "a\u{0}b"] {
        let encoded = serde_json::to_vec(&WorkbenchRequest::WorkspaceFiles(request(
            WorkspaceFilesOperation::ReadFile {
                path: path.into(),
                source: WorkspaceFileSource::Head,
                offset: 0,
                encoding: WorkspaceFileEncoding::Text,
            },
        )))
        .unwrap();
        assert!(parse_workbench_request(&encoded).is_err(), "{path:?}");
    }
    assert!(is_workspace_relative_path("src/a..b/.hidden"));
}

#[test]
fn workspace_files_frame_budget_is_rejected_before_reading_a_body() {
    let mut header = Vec::from(*b"OBWP");
    header.extend(1_u16.to_be_bytes());
    header.extend(84_u16.to_be_bytes());
    header.extend(((MAXIMUM_WORKSPACE_FILES_RESULT_BYTES + 1) as u32).to_be_bytes());
    assert!(matches!(
        read_workbench_response(&mut header.as_slice()),
        Err(WorkbenchWireError::Contract(ContractError::EncodedValueTooLarge { maximum, .. })) if maximum == MAXIMUM_WORKSPACE_FILES_RESULT_BYTES
    ));
}
