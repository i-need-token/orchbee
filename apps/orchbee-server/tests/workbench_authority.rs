use orchbee_server::{WorkbenchAuthority, WorkbenchGrant};
use orchbee_workbench_protocol::{
    MAXIMUM_ENCODED_INTENT_BYTES, SiteId, WorkbenchRequest, WorkbenchResponse,
    WorkbenchSnapshotOutcome, parse_workbench_request,
};

const READ: &[u8] = br#"{"kind":"workbench.snapshot.read","protocolVersion":1,"requestId":"22222222222222222222222222222222"}"#;

#[test]
fn snapshot_reads_preserve_correlation_and_isolate_returned_values() {
    let site = SiteId::parse("11111111111111111111111111111111").unwrap();
    let mut authority = WorkbenchAuthority::new(site.clone()).unwrap();
    let grant = WorkbenchGrant::read_only(site.clone());
    let request = parse_workbench_request(READ).unwrap();
    let first = authority.execute(Some(&grant), request.clone().try_into().unwrap());
    let mut changed = first.clone();
    let WorkbenchResponse::Snapshot(result) = &mut changed else {
        panic!("expected snapshot")
    };
    assert_eq!(
        result.request_id.as_str(),
        "22222222222222222222222222222222"
    );
    let WorkbenchSnapshotOutcome::Available { snapshot } = &mut result.outcome else {
        panic!("expected available")
    };
    assert_eq!(snapshot.site_id, site);
    snapshot.terminal_contexts.clear();
    assert_eq!(
        authority.execute(Some(&grant), request.try_into().unwrap()),
        first
    );
}

#[test]
fn malformed_or_oversized_read_intents_fail_before_authority_access() {
    for encoded in [b"{}".as_slice(), b"null".as_slice(), b"[]".as_slice()] {
        assert!(parse_workbench_request(encoded).is_err());
    }
    let mut oversized = READ.to_vec();
    oversized.resize(MAXIMUM_ENCODED_INTENT_BYTES + 1, b' ');
    assert!(parse_workbench_request(&oversized).is_err());
    assert!(matches!(
        parse_workbench_request(READ).unwrap(),
        WorkbenchRequest::SnapshotRead(_)
    ));
}

#[test]
fn admission_without_a_site_grant_discloses_no_snapshot() {
    let mut authority =
        WorkbenchAuthority::new(SiteId::parse("11111111111111111111111111111111").unwrap())
            .unwrap();
    let WorkbenchResponse::Snapshot(result) = authority.execute(
        None,
        parse_workbench_request(READ).unwrap().try_into().unwrap(),
    ) else {
        panic!("expected result")
    };
    assert_eq!(result.outcome, WorkbenchSnapshotOutcome::Denied {});
}
