use orchbee_workbench_protocol::*;
#[test]
fn current_directory_shared_corpus_and_closed_values() {
    let corpus: serde_json::Value = serde_json::from_str(include_str!(
        "../../../schemas/workbench/v1/workbench-current-directory-conformance-v1.json"
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
fn current_directory_distinct_carriers_and_exact_target_correlation() {
    let request = CurrentDirectoryRequest {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{:032x}", 1)).unwrap(),
        session_id: SessionId::parse(&format!("{:032x}", 2)).unwrap(),
        workspace_id: WorkbenchWorkspaceId::parse(&format!("{:032x}", 3)).unwrap(),
        panel: Some(CurrentDirectoryPanel {
            panel_id: WorkbenchPanelId::parse(&format!("{:032x}", 5)).unwrap(),
            presentation_id: TerminalPresentationId::parse(&format!("{:032x}", 6)).unwrap(),
        }),
    };
    let mut bytes = Vec::new();
    write_workbench_request(
        &mut bytes,
        &WorkbenchRequest::CurrentDirectory(request.clone()),
    )
    .unwrap();
    assert_eq!(&bytes[6..8], &65_u16.to_be_bytes());
    assert_eq!(
        read_workbench_request(&mut bytes.as_slice()).unwrap(),
        WorkbenchRequest::CurrentDirectory(request.clone())
    );
    // The longest directory, every byte escaped, still fits the result budget.
    let directory = WorkbenchDirectoryPath::parse(&format!(
        "C:\\{}",
        "\u{1}".repeat(MAXIMUM_DIRECTORY_PATH_BYTES - 3)
    ))
    .unwrap();
    let mut result = CurrentDirectoryResult {
        protocol_version: ProtocolVersion,
        request_id: request.request_id.clone(),
        session_id: request.session_id.clone(),
        workspace_id: request.workspace_id.clone(),
        panel: request.panel.clone(),
        outcome: CurrentDirectoryOutcome::Observed { directory },
    };
    assert!(result.answers(&request));
    bytes.clear();
    write_workbench_response(
        &mut bytes,
        &WorkbenchResponse::CurrentDirectory(result.clone()),
    )
    .unwrap();
    assert_eq!(&bytes[6..8], &66_u16.to_be_bytes());
    assert_eq!(
        read_workbench_response(&mut bytes.as_slice()).unwrap(),
        WorkbenchResponse::CurrentDirectory(result.clone())
    );
    // An answer about the Workspace, or about another Panel, does not answer a Panel's read.
    let workspace_answer = CurrentDirectoryResult {
        panel: None,
        ..result.clone()
    };
    assert!(!workspace_answer.answers(&request));
    result.workspace_id = WorkbenchWorkspaceId::parse(&format!("{:032x}", 4)).unwrap();
    assert!(!result.answers(&request));
}
