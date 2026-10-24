use super::*;
use crate::invocation::{SendSourceCause, UsageReason};

#[test]
fn text_failure_retains_localized_source_diagnostic_and_escapes_untrusted_paths() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let mut failure = Failure::new(FailureKind::InvalidUsage);
        failure.usage = Some(Box::new(UsageReason::SendSource {
            option: "--file",
            value: Some("C:\\bad\u{1b}[31m\tfile".into()),
            cause: SendSourceCause::NotRegularFile,
        }));
        let diagnostic = locale.cli_failure(&failure);
        let result = Receipt::new().emit(Some(failure), OutputFormat::Text, locale);
        assert_eq!(result.status, 2);
        assert!(result.stdout.is_empty());
        assert!(result.stderr.contains(&diagnostic));
        assert!(!result.stderr.contains('\u{1b}'));
    }
}

#[test]
fn repeated_close_observations_retain_latest_status_without_unbounded_output() {
    let mut receipt = Receipt::new();
    receipt.record("replace", json!({"request":"retained"}));
    for _ in 0..10000 {
        receipt.record("replace-status", json!({"status":{"kind":"closing"}}));
    }
    receipt.record(
        "replace-status",
        json!({"status":{"kind":"closed","revision":"8"}}),
    );
    assert_eq!(receipt.effects.len(), 2);
    assert_eq!(receipt.effects[0]["document"]["request"], "retained");
    assert_eq!(receipt.effects[1]["document"]["status"]["kind"], "closed");
}

#[test]
fn large_valid_member_layout_keeps_all_recovery_identities_within_the_native_output_budget() {
    fn id(n: usize) -> String {
        format!("{n:032x}")
    }
    fn root(count: usize, next: &mut usize) -> Value {
        if count > 1 {
            return json!({"type":"split","direction":"right","ratio":0.5,
            "first":root(count/2,next),"second":root(count-count/2,next)});
        }
        *next += 1;
        let panel = id(*next);
        let mut members = Vec::new();
        for _ in 0..17 {
            *next += 1;
            members.push(
                json!({"presentationId":id(*next),"name":"n".repeat(512),"label":"l".repeat(512),
            "start":true,"command":null,"cwd":format!("C:\\{}","x".repeat(697)),"env":{}}),
            );
        }
        let primary = members.remove(0);
        json!({"type":"panel","panelId":panel,"rightClick":"orchbee","primary":primary,"children":members})
    }
    let layout = json!({"workspaceId":null,"terminalContextId":id(2),"tabId":id(3),"zoomed":false,"focusedPanelId":id(101),"root":root(24,&mut 100)});
    let export = json!({"schema":"orchbee.layout.export","version":1,"source":{"kind":"layout-observation","scope":"client","siteId":id(1),"revision":"500","clientId":id(4),"viewRevision":"2"},"result":{"layout":layout}});
    assert!(output::encode_document(&export).unwrap().len() <= 1024 * 1024);
    let mut receipt = Receipt::new();
    receipt.stage = "complete";
    receipt.new_tab = Some(WorkbenchTabId::parse(&id(3)).unwrap());
    receipt.layout = Some(layout);
    for n in 0..408 {
        receipt.record("launch",json!({"action":"panel.create-terminal","siteId":id(1),"requestId":id(n+1000),"baseRevision":n.to_string(),
            "result":{"kind":"panel-terminal-pending","revision":(n+1).to_string(),"terminalContextId":id(2),"tabId":id(3),"panelId":id(101),"presentationId":id(102)}}));
    }
    let output = receipt.emit(None, OutputFormat::Json, Locale::EnUs);
    assert_eq!(output.status, 0);
    assert!(output.stdout.len() > 1024 * 1024);
    assert!(output.stdout.len() <= crate::cli::output::LIMIT);
    let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(doc["result"]["effects"].as_array().unwrap().len(), 408);
    let schema: Value =
        serde_json::from_str(include_str!("../../../schemas/orchbee.layout.apply.json")).unwrap();
    jsonschema::validator_for(&schema)
        .unwrap()
        .validate(&doc)
        .unwrap();
}

#[test]
fn both_text_locales_retain_exact_layout_request_id_on_unknown_launch() {
    let mut receipt = Receipt::new();
    receipt.stage = "launch";
    let request = "00000000000000000000000000001234";
    receipt.record(
        "launch",
        json!({"requestId":request,"result":{"kind":"outcome-unknown","reason":"response-read"}}),
    );
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let result = receipt.emit(
            Some(super::super::error(
                FailureKind::OutcomeUnknown,
                "launch-unavailable",
            )),
            OutputFormat::Text,
            locale,
        );
        assert_eq!(result.status, 8);
        assert!(result.stdout.is_empty());
        assert!(result.stderr.contains(request));
    }
}
