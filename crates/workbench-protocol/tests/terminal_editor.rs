use orchbee_workbench_protocol::*;

#[test]
fn terminal_editor_matches_shared_corpus_and_closed_carrier() {
    let corpus: serde_json::Value = serde_json::from_str(include_str!(
        "../../../schemas/workbench/v1/workbench-terminal-editor-conformance-v1.json"
    ))
    .unwrap();
    for (name, accepted) in [("valid", true), ("invalid", false)] {
        for case in corpus[name].as_array().unwrap() {
            let bytes = serde_json::to_vec(&case["document"]).unwrap();
            let result = if case["direction"] == "request" {
                parse_workbench_request(&bytes).map(|request| {
                    let mut wire = Vec::new();
                    write_workbench_request(&mut wire, &request).unwrap();
                    assert_eq!(&wire[6..8], &51u16.to_be_bytes());
                    assert_eq!(
                        read_workbench_request(&mut wire.as_slice()).unwrap(),
                        request
                    );
                    wire[6..8].copy_from_slice(&49u16.to_be_bytes());
                    assert!(read_workbench_request(&mut wire.as_slice()).is_err());
                })
            } else {
                parse_workbench_response(&bytes).map(|result| {
                    let mut wire = Vec::new();
                    write_workbench_response(&mut wire, &result).unwrap();
                    assert_eq!(&wire[6..8], &52u16.to_be_bytes());
                    assert_eq!(
                        read_workbench_response(&mut wire.as_slice()).unwrap(),
                        result
                    );
                    wire[6..8].copy_from_slice(&50u16.to_be_bytes());
                    assert!(read_workbench_response(&mut wire.as_slice()).is_err());
                })
            };
            assert_eq!(result.is_ok(), accepted, "{}", case["name"]);
        }
    }
    // Reject an over-budget frame before attempting to allocate/read its body.
    let mut header = Vec::from(*b"OBWP");
    header.extend_from_slice(&1u16.to_be_bytes());
    header.extend_from_slice(&51u16.to_be_bytes());
    header.extend_from_slice(
        &(u32::try_from(MAXIMUM_TERMINAL_EDITOR_BYTES).unwrap() + 1).to_be_bytes(),
    );
    assert!(matches!(
        read_workbench_request(&mut header.as_slice()),
        Err(WorkbenchWireError::Contract(ContractError::EncodedValueTooLarge { actual, maximum })) if actual == MAXIMUM_TERMINAL_EDITOR_BYTES + 1 && maximum == MAXIMUM_TERMINAL_EDITOR_BYTES
    ));
}
