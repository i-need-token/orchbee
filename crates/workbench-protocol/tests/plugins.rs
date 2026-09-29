use orchbee_workbench_protocol::*;
use serde_json::json;
#[test]
fn plugin_matches_shared_corpus_and_distinct_carrier_kinds() {
    let corpus: serde_json::Value = serde_json::from_str(include_str!(
        "../../../schemas/workbench/v1/workbench-plugin-conformance-v1.json"
    ))
    .unwrap();
    for (name, accepted) in [("valid", true), ("invalid", false)] {
        for case in corpus[name].as_array().unwrap() {
            let bytes = serde_json::to_vec(&case["document"]).unwrap();
            let result = if case["direction"] == "request" {
                parse_workbench_request(&bytes).map(|request| {
                    let mut wire = Vec::new();
                    write_workbench_request(&mut wire, &request).unwrap();
                    assert_eq!(&wire[6..8], &45_u16.to_be_bytes());
                    assert_eq!(
                        read_workbench_request(&mut wire.as_slice()).unwrap(),
                        request
                    );
                })
            } else {
                parse_workbench_response(&bytes).map(|result| {
                    let mut wire = Vec::new();
                    write_workbench_response(&mut wire, &result).unwrap();
                    assert_eq!(&wire[6..8], &46_u16.to_be_bytes());
                    assert_eq!(
                        read_workbench_response(&mut wire.as_slice()).unwrap(),
                        result
                    );
                })
            };
            assert_eq!(result.is_ok(), accepted, "{}", case["name"]);
        }
    }
}

#[test]
fn plugin_large_log_frames_and_query_correlation_are_bounded() {
    let request: PluginRequest = serde_json::from_value(json!({"protocolVersion":1,"requestId":"11111111111111111111111111111111","siteId":"22222222222222222222222222222222","action":{"kind":"logs","pluginId":null,"cursor":null,"limit":4}})).unwrap();
    let log = json!({"jobId":"44444444444444444444444444444444","sequence":1,"pluginId":"tools","commandId":"hello","command":["cmd.exe"],"startedMs":1,"finishedMs":2,"exitCode":0,"failure":null,"stdout":"\0".repeat(65536),"stderr":"\0".repeat(65536),"stdoutTruncated":false,"stderrTruncated":false});
    let logs = (1..=4)
        .rev()
        .map(|seq| {
            let mut log = log.clone();
            log["sequence"] = json!(seq);
            log
        })
        .collect::<Vec<_>>();
    let mut result:PluginResult=serde_json::from_value(json!({"protocolVersion":1,"requestId":request.request_id,"siteId":request.site_id,"outcome":{"kind":"logs","worker":{"workerId":"33333333333333333333333333333333","generation":"1"},"logs":logs,"next":null}})).unwrap();
    assert!(result.answers(&request));
    let message = WorkbenchResponse::Plugin(result.clone());
    let mut wire = Vec::new();
    write_workbench_response(&mut wire, &message).unwrap();
    assert!(wire.len() > 1024 * 1024);
    assert_eq!(
        read_workbench_response(&mut wire.as_slice()).unwrap(),
        message
    );
    if let PluginOutcome::Logs { logs, .. } = &mut result.outcome {
        logs[0].plugin_id = "different".into();
    }
    let mut filtered = request.clone();
    if let PluginAction::Logs { plugin_id, .. } = &mut filtered.action {
        *plugin_id = Some("tools".into());
    }
    assert!(!result.answers(&filtered));
    let mut header = wire[..12].to_vec();
    header[8..12]
        .copy_from_slice(&((MAXIMUM_ENCODED_PLUGIN_RESULT_BYTES + 1) as u32).to_be_bytes());
    assert!(read_workbench_response(&mut header.as_slice()).is_err());
}

#[test]
fn plugin_wire_rejects_unpaired_surrogate_context() {
    assert!(parse_workbench_request(br#"{"kind":"workbench.plugin","protocolVersion":1,"requestId":"11111111111111111111111111111111","siteId":"22222222222222222222222222222222","action":{"kind":"invoke","operationId":"44444444444444444444444444444444","pluginId":null,"actionId":"hello","context":{"selectedText":"\ud800"}}}"#).is_err());
}

#[test]
fn plugin_uncertain_cancellation_still_pins_worker_generation() {
    let request: PluginRequest = serde_json::from_value(json!({"protocolVersion":1,"requestId":"11111111111111111111111111111111","siteId":"22222222222222222222222222222222","action":{"kind":"cancel","worker":{"workerId":"33333333333333333333333333333333","generation":"1"},"jobId":"44444444444444444444444444444444"}})).unwrap();
    let mut result: PluginResult = serde_json::from_value(json!({"protocolVersion":1,"requestId":request.request_id,"siteId":request.site_id,"outcome":{"kind":"unknown","worker":{"workerId":"33333333333333333333333333333333","generation":"1"}}})).unwrap();
    assert!(result.answers(&request));
    if let PluginOutcome::Unknown { worker } = &mut result.outcome {
        worker.generation = WorkbenchRevision::new(2);
    }
    assert!(!result.answers(&request));
}
