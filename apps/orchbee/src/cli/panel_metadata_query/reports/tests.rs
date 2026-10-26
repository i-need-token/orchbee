use super::*;
fn id(n: u64) -> String {
    format!("{n:032x}")
}
fn row(n: u64) -> PanelAgentRow {
    serde_json::from_value(json!({"panelId":id(n),"binding":{"workerId":id(1),"terminalId":id(n+100)},
            "report":{"agent":"Team Agent","state":"blocked","authoritySource":"source", "message":"message"}})).unwrap()
}
#[test]
fn report_batch_capacity_and_revision_failures_do_not_admit_partial_rows() {
    let size = serde_json::to_vec(&row(2)).unwrap().len();
    let mut doc = Document {
        rows: BTreeMap::new(),
        revision: WorkbenchRevision::new(2),
        request_ids: vec![],
        encoded_bytes: 0,
        byte_limit: size,
    };
    let request = |n| WorkbenchRequestId::parse(&id(n)).unwrap();
    assert_eq!(
        doc.admit(request(1), doc.revision, vec![row(2), row(3)])
            .unwrap_err()
            .kind,
        FailureKind::OutputTooLarge
    );
    assert!(doc.rows.is_empty() && doc.request_ids.is_empty());
    assert_eq!(doc.encoded_bytes, 0);
    doc.admit(request(2), doc.revision, vec![row(2)]).unwrap();
    assert_eq!(doc.encoded_bytes, size);
    assert_eq!(
        doc.admit(request(3), WorkbenchRevision::new(3), vec![])
            .unwrap_err()
            .kind,
        FailureKind::Rejected
    );
    assert_eq!(
        doc.admit(request(4), doc.revision, vec![row(3)])
            .unwrap_err()
            .kind,
        FailureKind::OutputTooLarge
    );
    assert_eq!(
        doc.admit(request(5), doc.revision, vec![row(2)])
            .unwrap_err()
            .kind,
        FailureKind::ProtocolInvalid
    );
    assert_eq!(doc.request_ids, vec![request(2)]);
    doc.admit(request(6), doc.revision, vec![]).unwrap();
    assert_eq!(doc.request_ids, vec![request(2), request(6)]);
}
