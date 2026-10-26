//! Exact write receipts cannot answer a primary request or another member's request.
use orchbee_workbench_protocol::*;
use serde_json::Value;

#[test]
fn exact_report_receipts_correlate_every_target_dimension_and_reject_legacy_crossovers() {
    for corpus in [
        include_str!("../../../schemas/workbench/v1/workbench-panel-agent-conformance-v1.json"),
        include_str!("../../../schemas/workbench/v1/workbench-panel-metadata-conformance-v1.json"),
    ] {
        let corpus: Value = serde_json::from_str(corpus).unwrap();
        let find = |name| {
            corpus["valid"]
                .as_array()
                .unwrap()
                .iter()
                .find(|c| c["name"] == name)
                .unwrap()["document"]
                .clone()
        };
        let request = find("exact-member-report-presentation");
        let mut response = find("exact-member-presentation-reported");
        response["requestId"] = request["requestId"].clone();
        response["sessionId"] = request["sessionId"].clone();
        let answers = |request: &Value, response: &Value| {
            let request = parse_workbench_request(&serde_json::to_vec(request).unwrap()).unwrap();
            let response =
                parse_workbench_response(&serde_json::to_vec(response).unwrap()).unwrap();
            match (request, response) {
                (WorkbenchRequest::PanelAgent(a), WorkbenchResponse::PanelAgent(b)) => {
                    b.answers(&a)
                }
                (WorkbenchRequest::PanelMetadata(a), WorkbenchResponse::PanelMetadata(b)) => {
                    b.answers(&a)
                }
                _ => panic!("matching carriers"),
            }
        };
        assert!(answers(&request, &response));
        for key in ["panelId", "presentationId"] {
            let mut wrong = response.clone();
            wrong["outcome"][key] = serde_json::json!("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee");
            assert!(!answers(&request, &wrong));
        }
        let mut wrong = response.clone();
        wrong["outcome"]["binding"]["terminalId"] =
            serde_json::json!("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee");
        assert!(!answers(&request, &wrong));
        let mut primary_request = request.clone();
        primary_request["action"]["kind"] = serde_json::json!("report");
        primary_request["action"]
            .as_object_mut()
            .unwrap()
            .remove("presentationId");
        assert!(!answers(&primary_request, &response));
        let mut primary_response = response.clone();
        primary_response["outcome"]["kind"] = serde_json::json!("reported");
        primary_response["outcome"]
            .as_object_mut()
            .unwrap()
            .remove("presentationId");
        assert!(!answers(&request, &primary_response));
        assert!(answers(&primary_request, &primary_response));
    }
}
