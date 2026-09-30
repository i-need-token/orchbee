use orchbee_workbench_protocol::*;
#[test]
fn workspace_git_shared_corpus_and_closed_values() {
    let corpus: serde_json::Value = serde_json::from_str(include_str!(
        "../../../schemas/workbench/v1/workbench-workspace-git-conformance-v1.json"
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
fn workspace_git_distinct_carriers_and_exact_context_correlation() {
    let request = WorkspaceGitRequest {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{:032x}", 1)).unwrap(),
        site_id: SiteId::parse(&format!("{:032x}", 2)).unwrap(),
        workspace_id: WorkbenchWorkspaceId::parse(&format!("{:032x}", 3)).unwrap(),
    };
    let mut bytes = Vec::new();
    write_workbench_request(&mut bytes, &WorkbenchRequest::WorkspaceGit(request.clone())).unwrap();
    assert_eq!(&bytes[6..8], &49_u16.to_be_bytes());
    assert_eq!(
        read_workbench_request(&mut bytes.as_slice()).unwrap(),
        WorkbenchRequest::WorkspaceGit(request.clone())
    );
    let mut result = WorkspaceGitResult {
        protocol_version: ProtocolVersion,
        request_id: request.request_id.clone(),
        site_id: request.site_id.clone(),
        workspace_id: request.workspace_id.clone(),
        outcome: WorkspaceGitOutcome::Observed {
            workbench_revision: WorkbenchRevision::new(1),
            branch: None,
            divergence: None,
        },
    };
    assert!(result.answers(&request));
    bytes.clear();
    write_workbench_response(&mut bytes, &WorkbenchResponse::WorkspaceGit(result.clone())).unwrap();
    assert_eq!(&bytes[6..8], &50_u16.to_be_bytes());
    assert_eq!(
        read_workbench_response(&mut bytes.as_slice()).unwrap(),
        WorkbenchResponse::WorkspaceGit(result.clone())
    );
    result.workspace_id = WorkbenchWorkspaceId::parse(&format!("{:032x}", 4)).unwrap();
    assert!(!result.answers(&request));
}

#[test]
fn workspace_git_frame_budget_is_rejected_before_reading_a_body() {
    let mut header = Vec::from(*b"OBWP");
    header.extend(1_u16.to_be_bytes());
    header.extend(50_u16.to_be_bytes());
    header.extend(((MAXIMUM_WORKSPACE_GIT_BYTES + 1) as u32).to_be_bytes());
    assert!(matches!(
        read_workbench_response(&mut header.as_slice()),
        Err(WorkbenchWireError::Contract(ContractError::EncodedValueTooLarge { actual, maximum })) if actual == MAXIMUM_WORKSPACE_GIT_BYTES+1 && maximum == MAXIMUM_WORKSPACE_GIT_BYTES
    ));
    header[6..8].copy_from_slice(&49_u16.to_be_bytes());
    assert!(
        matches!(
            read_workbench_response(&mut header.as_slice()),
            Err(WorkbenchWireError::InvalidHeader)
        ),
        "wrong direction must reject before body"
    );
}
