use super::*;
use orchbee_terminal_protocol::{
    CURRENT_PROTOCOL_VERSION, CorrelationId, Envelope, TerminalId, TerminalProcess, TypedFailure,
    encode_connection_control, encode_process_info_control, encode_transport_envelope,
};

fn info() -> TerminalProcessInfo {
    TerminalProcessInfo {
        attachment_id: AttachmentId::from_bytes([1; 16]).unwrap(),
        terminal_id: TerminalId::from_bytes([2; 16]).unwrap(),
        root_pid: 41,
        processes: vec![TerminalProcess {
            pid: 41,
            parent_pid: None,
            creation_time: u64::MAX,
            image: Some("C:\\Windows\\cmd.exe".into()),
            command_line: Some("cmd /C echo 中 😀\u{1b}]2;title\u{7}".into()),
            current_directory: None,
        }],
    }
}

fn record(kind: u16, payload: &[u8], change: impl FnOnce(&mut Envelope<'_>)) -> Vec<u8> {
    let mut envelope = Envelope {
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Response,
        kind,
        optional_message: false,
        payload_encoding: PayloadEncoding::ControlJson,
        correlation_id: Some(CorrelationId::from_bytes([3; 16]).unwrap()),
        host_epoch: Some(HostEpoch::from_bytes([4; 16]).unwrap()),
        payload,
    };
    change(&mut envelope);
    encode_transport_envelope(&envelope).unwrap()
}

fn read(record: &[u8]) -> Result<(TerminalProcessInfo, String), Failure> {
    decode(
        record,
        info().attachment_id,
        HostEpoch::from_bytes([4; 16]).unwrap(),
    )
}

#[test]
fn process_info_rejects_wrong_envelope_attachment_and_malformed_payload() {
    let encoded = encode_process_info_control(&ProcessInfoControl::ProcessInfo(info())).unwrap();
    let valid = record(encoded.kind, &encoded.payload, |_| {});
    assert_eq!(read(&valid).unwrap().0, info());
    let epoch = record(encoded.kind, &encoded.payload, |e| {
        e.host_epoch = Some(HostEpoch::from_bytes([5; 16]).unwrap())
    });
    let role = record(encoded.kind, &encoded.payload, |e| {
        e.role = MessageRole::Request
    });
    let malformed = record(encoded.kind, b"{}", |_| {});
    let mut wrong = info();
    wrong.attachment_id = AttachmentId::from_bytes([6; 16]).unwrap();
    let wrong = encode_process_info_control(&ProcessInfoControl::ProcessInfo(wrong)).unwrap();
    let attachment = record(wrong.kind, &wrong.payload, |_| {});
    for bad in [&epoch, &role, &malformed, &attachment, &vec![0; 65536 + 65]] {
        assert_eq!(read(bad).unwrap_err().kind, FailureKind::ProtocolInvalid);
    }
}

#[test]
fn process_info_preserves_expected_host_failures_with_valid_failure_schema() {
    let schema: Value = serde_json::from_str(include_str!(
        "../../schemas/orchbee.cli.failure.process-info.json"
    ))
    .unwrap();
    let validator = jsonschema::draft202012::new(&schema).unwrap();
    for code in [
        "limit-exceeded",
        "missing-resource",
        "process-sample-unsupported",
        "process-sample-changed",
        "process-sample-unavailable",
        "invalid-field",
    ] {
        let encoded = encode_connection_control(&ConnectionControl::TypedFailure(TypedFailure {
            code: code.into(),
            message: "untrusted Host diagnostic".into(),
            retryable: false,
        }))
        .unwrap();
        let failure = read(&record(encoded.kind, &encoded.payload, |_| {})).unwrap_err();
        if code == "invalid-field" {
            assert_eq!(failure.kind, FailureKind::ProtocolInvalid);
            continue;
        }
        let output = crate::cli::failure_output(
            failure,
            Some(crate::invocation::OutputFormat::Json),
            crate::localization::Locale::EnUs,
        );
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        validator.validate(&document).unwrap();
        assert_eq!(document["failure"]["details"]["reason"], code);
        assert!(!output.stderr.contains("untrusted"));
    }
}

#[test]
fn process_info_machine_contract_preserves_precision_nulls_and_escapes_text() {
    let terminal = results::TerminalRow {
        terminal_id: format!("{:032x}", 10),
        worker_id: format!("{:032x}", 11),
        presentation_id: format!("{:032x}", 12),
        panel_id: format!("{:032x}", 13),
        tab_id: format!("{:032x}", 14),
        terminal_context_id: format!("{:032x}", 15),
    };
    let projection = Projection::PanelProcessInfo(results::PanelProcessInfoDoc {
        terminal,
        info: info(),
    });
    let source = json!({"kind":"terminal-process-query","siteId":format!("{:032x}",16),"snapshotRevision":"7","requestId":format!("{:032x}",17),"hostRequestId":format!("{:032x}",18),"hostEpoch":format!("{:032x}",19),"attachmentId":format!("{:032x}",20),"hostTerminalId":format!("{:032x}",21)});
    let schema: Value = serde_json::from_str(include_str!(
        "../../schemas/orchbee.panel.process-info.json"
    ))
    .unwrap();
    let validator = jsonschema::draft202012::new(&schema).unwrap();
    let document = json!({"schema":"orchbee.panel.process-info","version":2,"source":source,"result":projection.result()});
    validator.validate(&document).unwrap();
    assert_eq!(
        document["result"]["processes"][0]["creationTime"],
        u64::MAX.to_string()
    );
    assert!(document["result"]["processes"][0]["currentDirectory"].is_null());
    for locale in [
        crate::localization::Locale::EnUs,
        crate::localization::Locale::ZhCn,
    ] {
        let output = crate::cli::output::render_text(|out| {
            crate::cli::present::render(
                out,
                &projection,
                &QueryCommand::PanelProcessInfo(crate::invocation::PanelQueryTarget::Current),
                locale,
            )
        })
        .unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(!text.contains('\u{1b}'));
        assert!(!text.contains('\u{7}'));
        assert!(text.contains("中 😀"));
    }
}

#[test]
fn process_info_targets_only_the_primary_binding_without_child_fallback() {
    let id = |value: u8| format!("{value:032x}");
    let mut value = json!({"kind":"workbench.snapshot","protocolVersion":1,"siteId":id(1),"revision":"7",
    "navigation":{"projects":[], "directories":[], "workspaces":[{"workspaceId":id(2),"terminalContextId":id(2),"projectId":null,"primaryDirectoryId":null,"label":"Workspace"}]},
    "terminalContexts":[{"terminalContextId":id(2),"groups":[],"tabs":[{
        "tabId":id(3),"label":"tab","pinned":false,"groupId":null,
        "root":{"kind":"panel","panelId":id(4),
            "primaryPresentation":{"customLabel":null,"presentationId":id(5),"label":"primary","binding":{"terminalId":id(10),"workerId":id(11)}},
            "childPresentations":[{"customLabel":null,"presentationId":id(6),"label":"child","binding":{"terminalId":id(12),"workerId":id(11)}}]}
    }]}]});
    let parse = |value: &Value| {
        orchbee_workbench_protocol::parse_snapshot(&serde_json::to_vec(value).unwrap()).unwrap()
    };
    assert_eq!(
        primary_terminal(&parse(&value), &Target::Selector("@panel:04".into())).unwrap(),
        Target::Id(id(10))
    );
    value["terminalContexts"][0]["tabs"][0]["root"]["primaryPresentation"]["binding"] = Value::Null;
    assert_eq!(
        primary_terminal(&parse(&value), &Target::Id(id(4)))
            .unwrap_err()
            .kind,
        FailureKind::NotFound
    );
}
