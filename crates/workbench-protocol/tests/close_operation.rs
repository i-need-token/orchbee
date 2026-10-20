use orchbee_workbench_protocol::{
    CloseOperationOutcome, CloseOperationRead, CloseOperationResult, ProtocolVersion, SiteId,
    WorkbenchRequest, WorkbenchRequestId, WorkbenchResponse, read_workbench_request,
    read_workbench_response, write_workbench_request, write_workbench_response,
};

#[test]
fn retained_close_identity_lookup_roundtrips_and_correlates_every_identity() {
    let request = CloseOperationRead {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse("00000000000000000000000000000001").unwrap(),
        site_id: SiteId::parse("00000000000000000000000000000002").unwrap(),
        operation_id: WorkbenchRequestId::parse("00000000000000000000000000000003").unwrap(),
    };
    let mut bytes = Vec::new();
    let wire = WorkbenchRequest::CloseOperationRead(request.clone());
    write_workbench_request(&mut bytes, &wire).unwrap();
    assert_eq!(read_workbench_request(&mut bytes.as_slice()).unwrap(), wire);
    let result = CloseOperationResult {
        protocol_version: ProtocolVersion,
        request_id: request.request_id.clone(),
        site_id: request.site_id.clone(),
        operation_id: request.operation_id.clone(),
        outcome: CloseOperationOutcome::NotFound {},
    };
    assert!(result.answers(&request));
    let changed = CloseOperationRead {
        operation_id: request.request_id.clone(),
        ..request
    };
    assert!(!result.answers(&changed));
    let wire = WorkbenchResponse::CloseOperation(result);
    bytes.clear();
    write_workbench_response(&mut bytes, &wire).unwrap();
    assert_eq!(
        read_workbench_response(&mut bytes.as_slice()).unwrap(),
        wire
    );
}
