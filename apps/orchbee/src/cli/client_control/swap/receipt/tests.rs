use super::*;
use crate::cli::client_control::Wait;
use orchbee_workbench_protocol::WorkbenchResponse;

fn receipt() -> Receipt {
    let id = |n| format!("{n:032x}");
    Receipt {
        site_id: SiteId::parse(&id(2)).unwrap(),
        client_id: WorkbenchClientId::parse(&id(7)).unwrap(),
        base_revision: WorkbenchRevision::new(12),
        source: Panel {
            terminal_context_id: WorkbenchTerminalContextId::parse(&id(4)).unwrap(),
            tab_id: WorkbenchTabId::parse(&id(5)).unwrap(),
            panel_id: WorkbenchPanelId::parse(&id(6)).unwrap(),
        },
        target: None,
        direction: None,
        layout_operation_id: None,
        layout_revision: None,
        request_id: Some(WorkbenchRequestId::parse(&id(1)).unwrap()),
        canonical: Canonical::Committed {
            revision: WorkbenchRevision::new(13),
        },
        focus: None,
        stage: "focus",
        reason: None,
        failure: None,
    }
}

#[test]
fn terminal_failure_cuts_preserve_canonical_and_focus_evidence_in_both_locales() {
    let corpus: Value = serde_json::from_str(include_str!(
        "../../../../../../../schemas/workbench/v1/workbench-client-view-conformance-v1.json"
    ))
    .unwrap();
    let schema: Value =
        serde_json::from_str(include_str!("../../../schemas/orchbee.panel.swap.json")).unwrap();
    let validator = jsonschema::draft202012::new(&schema).unwrap();
    for name in [
        None,
        Some("operation-queued"),
        Some("operation-delivered"),
        Some("operation-applied"),
        Some("operation-rejected"),
        Some("operation-incomplete"),
    ] {
        for failure in [
            None,
            Some(FailureKind::Cancelled),
            Some(FailureKind::Timeout),
            Some(FailureKind::TransportLoss),
            Some(FailureKind::InternalFailure),
        ] {
            let mut receipt = receipt();
            receipt.failure = failure;
            if let Some(name) = name {
                let document = &corpus["valid"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|e| e["name"] == name)
                    .unwrap()["document"];
                let WorkbenchResponse::ClientView(reply) =
                    serde_json::from_value(document.clone()).unwrap()
                else {
                    panic!("reply");
                };
                receipt.focus = Some(Answer {
                    status_only: false,
                    operation_id: Some(
                        WorkbenchRequestId::parse("00000000000000000000000000000003").unwrap(),
                    ),
                    site_id: Some(receipt.site_id.clone()),
                    reply: Some(reply),
                    unknown: false,
                    wait: None,
                });
            }
            let document = receipt.document();
            assert_eq!(
                document["canonical"],
                json!({"kind":"committed","revision":"13"})
            );
            validator.validate(&document).unwrap();
            let complete = name == Some("operation-applied") && failure.is_none();
            assert_eq!(receipt.exit_code() == 0, complete, "{name:?}/{failure:?}");
            assert_eq!(
                document["outcome"],
                if complete { "swapped" } else { "incomplete" }
            );
            for locale in [Locale::EnUs, Locale::ZhCn] {
                let output = receipt.emit(OutputFormat::Json, locale);
                assert!(output.stderr.is_empty());
                assert_eq!(
                    serde_json::from_slice::<Value>(&output.stdout).unwrap(),
                    document
                );
                let output = receipt.emit(OutputFormat::Text, locale);
                assert_eq!(output.stdout.is_empty(), !complete);
                assert_eq!(output.stderr.is_empty(), complete);
            }
        }
    }
    let mut receipt = receipt();
    receipt.focus = Some(Answer {
        status_only: false,
        operation_id: receipt.request_id.clone(),
        site_id: Some(receipt.site_id.clone()),
        reply: None,
        unknown: true,
        wait: None,
    });
    for wait in [
        None,
        Some(Wait::TimedOut),
        Some(Wait::Cancelled),
        Some(Wait::TransportLost),
        Some(Wait::ObservationFailed),
    ] {
        receipt.focus.as_mut().unwrap().wait = wait;
        let document = receipt.document();
        validator.validate(&document).unwrap();
        assert_eq!(document["canonical"]["kind"], "committed");
        assert_eq!(document["focus"]["observation"]["kind"], "unknown");
        assert_ne!(receipt.exit_code(), 0);
    }
}
