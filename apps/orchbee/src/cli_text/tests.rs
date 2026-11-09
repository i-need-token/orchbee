//! Retains CLI bilingual diagnostics, receipt formatting and escaping evidence through the real text owner.
use super::{CliText, Locale};

#[test]
fn agent_start_preserves_the_caller_context_usage_hint() {
    let mut failure =
        crate::cli_outcome::Failure::new(crate::cli_outcome::FailureKind::InvalidUsage)
            .failure_schema("orchbee.cli.failure.agent-start");
    failure.usage = Some(Box::new(
        crate::invocation::UsageReason::CallerContextUnavailable,
    ));
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let message = CliText::new(locale).cli_failure(&failure);
        assert!(message.contains("--panel"), "{message}");
        assert!(
            !message.contains("confirmed") && !message.contains("确认"),
            "{message}"
        );
    }
}

#[test]
fn missing_agent_panel_reports_the_target_and_discovery_command() {
    let failure = crate::cli_outcome::Failure::new(crate::cli_outcome::FailureKind::NotFound)
        .failure_schema("orchbee.cli.failure.agent-start")
        .details(serde_json::Map::from_iter([
            ("reason".into(), serde_json::json!("startup-failed")),
            ("cause".into(), serde_json::json!({"kind":"not-found"})),
        ]));
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let message = CliText::new(locale).cli_failure(&failure);
        assert!(
            message.contains("orchbee panel list") && message.contains("--panel"),
            "{message}"
        );
    }
}

#[test]
fn display_name_hints_follow_target_syntax_in_both_locales() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for label in [
            "00000000000000000000000000000002",
            "@workspace:0xabcd",
            "@tab:invalid",
            "@project:",
        ] {
            assert!(
                CliText::new(locale).cli_label_target_hint(label).is_some(),
                "{label}"
            );
        }
        for label in ["example", "中文 workspace", "@unknown:abc", "123"] {
            assert!(
                CliText::new(locale).cli_label_target_hint(label).is_none(),
                "{label}"
            );
        }
    }
}

/// The `invalid-usage` stderr diagnostic names one concrete cause plus a real
/// help hint in both locales; echoed tokens pass through the quoting/escape
/// layer with its 256-scalar prefix bound.
#[test]
fn cli_usage_diagnostics_are_localized_and_escaped() {
    use crate::cli_outcome::{Failure, FailureKind};
    use crate::invocation::{UsageContext, UsageReason};

    let usage_failure = |usage: UsageReason| Failure {
        kind: FailureKind::InvalidUsage,
        details: None,
        source: None,
        failure_schema: None,
        display: None,
        client_refusal: None,
        usage: Some(Box::new(usage)),
    };

    // Unknown command echoes the escaped word and keeps the root hint.
    let failure = usage_failure(UsageReason::UnknownCommand {
        context: UsageContext::Root,
        word: "bo\u{1b}gus".to_owned(),
    });
    assert_eq!(
        CliText::new(Locale::EnUs).cli_failure(&failure),
        "Unknown command \"bo\\u{001b}gus\". Run 'orchbee --help'.\n"
    );
    assert_eq!(
        CliText::new(Locale::ZhCn).cli_failure(&failure),
        "未知命令 \"bo\\u{001b}gus\"。请运行 ‘orchbee --help’。\n"
    );

    // Command context anchors the real `<path> --help` hint.
    let failure = usage_failure(UsageReason::UnexpectedOption {
        context: UsageContext::Path("workspace list"),
        option: "--workspace",
    });
    assert_eq!(
        CliText::new(Locale::EnUs).cli_failure(&failure),
        "'--workspace' is not a valid option for 'orchbee workspace list'. Run 'orchbee workspace list --help'.\n"
    );
    assert_eq!(
        CliText::new(Locale::ZhCn).cli_failure(&failure),
        "'--workspace' 不是 'orchbee workspace list' 的有效选项。请运行 ‘orchbee workspace list --help’。\n"
    );

    // The argv-envelope reason never echoes argv content.
    let failure = usage_failure(UsageReason::ArgumentEnvelope);
    assert_eq!(
        CliText::new(Locale::EnUs).cli_failure(&failure),
        "Invalid invocation: the argument list must be Unicode and its total size may not exceed 64 KiB. Run 'orchbee --help'.\n"
    );
    assert_eq!(
        CliText::new(Locale::ZhCn).cli_failure(&failure),
        "调用方式无效：参数必须为 Unicode，且总大小不得超过 64 KiB。请运行 ‘orchbee --help’。\n"
    );

    let failure = usage_failure(UsageReason::RelayRequiresPipes);
    assert_eq!(
        CliText::new(Locale::EnUs).cli_failure(&failure),
        "'orchbee server connect' carries protocol bytes for another program, so its standard input and output must be pipes, not a terminal. Run 'orchbee server connect --help'.\n"
    );
    assert_eq!(
        CliText::new(Locale::ZhCn).cli_failure(&failure),
        "'orchbee server connect' 为其他程序传输协议字节，因此标准输入和输出必须是管道，不能是终端。请运行 ‘orchbee server connect --help’。\n"
    );

    // A reason-free InvalidUsage keeps the original generic text.
    let failure = Failure {
        kind: FailureKind::InvalidUsage,
        details: None,
        source: None,
        failure_schema: None,
        display: None,
        client_refusal: None,
        usage: None,
    };
    assert_eq!(
        CliText::new(Locale::EnUs).cli_failure(&failure),
        "Invalid invocation. Run 'orchbee --help'.\n"
    );
    assert_eq!(
        CliText::new(Locale::ZhCn).cli_failure(&failure),
        "调用方式无效。请运行 ‘orchbee --help’。\n"
    );

    // Other failure kinds are untouched by the reason path.
    let failure = Failure {
        kind: FailureKind::NotFound,
        details: Some(serde_json::Map::from_iter([
            ("resource".to_owned(), serde_json::json!("tab")),
            ("target".to_owned(), serde_json::json!("x")),
        ])),
        source: None,
        failure_schema: None,
        display: None,
        client_refusal: None,
        usage: None,
    };
    assert!(
        CliText::new(Locale::EnUs)
            .cli_failure(&failure)
            .starts_with("No tab matches \"x\".")
    );
}

/// Cohort text names a sibling only when artifact-open evidence proves it is
/// absent; bounded I/O details remain available through the status client
/// JSON diagnostic. The failure envelope and exit code remain unchanged.
#[test]
fn cli_cohort_failure_uses_repair_text_without_leaking_raw_io() {
    use crate::cli::{Failure, FailureKind, failure_output, stage_details};
    use crate::invocation::OutputFormat;
    use serde_json::json;

    let cohort_failure = |error: &std::io::Error| {
        let error = orchbee_local_runtime::CohortError::from(std::io::Error::new(
            error.kind(),
            error.to_string(),
        ));
        Failure::new(FailureKind::CheckFailed)
            .details(stage_details("cohort"))
            .io_diagnostic(&error)
    };

    // The opaque failure itself carries no I/O detail to stdout or stderr; Windows points at the
    // status client JSON diagnostic instead.
    let hint = "orchbee status client --format json";
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let failure = cohort_failure(&std::io::Error::from_raw_os_error(110));
        let text = CliText::new(locale).cli_failure(&failure);
        assert!(!text.contains("110"), "{text}");
        assert_eq!(text.contains(hint), cfg!(windows), "{text}");
        let output = failure_output(failure, Some(OutputFormat::Text), locale);
        assert_eq!(output.status, 1);
        assert!(output.stdout.is_empty());
        assert_eq!(output.stderr, text);
        // A failure without typed cohort evidence does not invent a missing file.
        let untyped = Failure::new(FailureKind::CheckFailed).details(stage_details("cohort"));
        assert_eq!(CliText::new(locale).cli_failure(&untyped), text);
        // Only the typed missing-artifact path names the file.
        assert!(!text.contains("orchbee-server.exe"));
        assert!(
            CliText::new(locale)
                .cli_cohort_diagnostic(Some("orchbee-server.exe"))
                .contains("`orchbee-server.exe`")
        );
    }
    assert_ne!(
        CliText::new(Locale::EnUs).cli_cohort_diagnostic(None),
        CliText::new(Locale::ZhCn).cli_cohort_diagnostic(None)
    );

    // The failure document and exit stay exactly the existing contract.
    let output = failure_output(
        cohort_failure(&std::io::Error::from_raw_os_error(110)),
        Some(OutputFormat::Json),
        Locale::EnUs,
    );
    assert_eq!(output.status, 1);
    let document: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        document,
        json!({
            "schema": "orchbee.cli.failure",
            "version": 1,
            "failure": {"kind": "check-failed", "details": {"stage": "cohort"}},
        })
    );
}

#[test]
fn missing_selected_session_names_a_valid_startup_command_in_both_locales() {
    use crate::cli::{Failure, FailureKind, failure_output, stage_details};
    use crate::invocation::OutputFormat;

    let failure = Failure::new(FailureKind::ServiceUnavailable).details(stage_details("discovery"));
    assert_eq!(
        CliText::new(Locale::EnUs).cli_service_unavailable_for_selection(&failure, Some("nosuch")),
        "No running local session named `nosuch` was found. Start it with: orchbee --session nosuch server start.\n"
    );
    assert_eq!(
        CliText::new(Locale::ZhCn).cli_service_unavailable_for_selection(&failure, Some("nosuch")),
        "未找到名为 `nosuch` 的运行中本地会话。请运行：orchbee --session nosuch server start。\n"
    );
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let output = failure_output(
            Failure::new(FailureKind::ServiceUnavailable).details(stage_details("discovery")),
            Some(OutputFormat::Text),
            locale,
        );
        assert_eq!(output.status, 4);
        assert!(output.stdout.is_empty());
        assert!(output.stderr.contains("orchbee server start"));
    }
}

#[test]
fn popup_receipt_text_preserves_incomplete_reason_status_and_query_guidance_in_both_locales() {
    let id = "a".repeat(32);
    for (locale, incomplete, reason, opened, closed) in [
        (
            Locale::EnUs,
            "Incomplete",
            "Projection unavailable",
            "Popup opened and drawn",
            "Popup close completed",
        ),
        (
            Locale::ZhCn,
            "未完成",
            "界面数据不可用",
            "popup 已打开并绘制",
            "popup 关闭操作已完成",
        ),
    ] {
        let mut document = serde_json::json!({"schema":"orchbee.popup.result","operationId":id,
            "observation":{"kind":"known","reply":{"outcome":{"kind":"operation","operation":{
                "command":{"clientId":"b".repeat(32)},"status":{"kind":"incomplete","reason":"projection-unavailable"}
            }}}},"wait":"timed-out"});
        let text = CliText::new(locale).cli_client_control(&document);
        assert!(text.contains(incomplete) && text.contains(reason), "{text}");
        assert!(
            text.contains(&format!("orchbee client control-status {id}")),
            "{text}"
        );
        document["schema"] = serde_json::json!("orchbee.client.control");
        for (state, label) in [("popup-opened", opened), ("popup-closed", closed)] {
            document["observation"]["reply"]["outcome"]["operation"]["status"]["kind"] =
                serde_json::json!(state);
            assert!(
                CliText::new(locale)
                    .cli_client_control(&document)
                    .contains(label)
            );
        }
    }
}

#[test]
fn viewer_read_failures_explain_the_cause_without_internal_reason_codes() {
    use crate::cli_outcome::{Failure, FailureKind};

    for reason in [
        "stale-revision",
        "stale-generation",
        "history-evicted",
        "missing-resource",
    ] {
        let failure =
            Failure::new(FailureKind::ServiceUnavailable).details(serde_json::Map::from_iter([
                ("stage".to_owned(), serde_json::json!("viewer")),
                ("operation".to_owned(), serde_json::json!("read-history")),
                ("reason".to_owned(), serde_json::json!(reason)),
            ]));
        for (locale, hint) in [(Locale::EnUs, "Retry the read"), (Locale::ZhCn, "重试读取")] {
            let text = CliText::new(locale).cli_service_unavailable_for_selection(&failure, None);
            assert!(!text.contains(reason), "{reason}: {text}");
            assert!(text.contains(hint), "{reason}: {text}");
        }
    }
    assert_eq!(
        CliText::new(Locale::EnUs).cli_service_unavailable_for_selection(
            &Failure::new(FailureKind::ServiceUnavailable).details(serde_json::Map::from_iter([
                ("stage".to_owned(), serde_json::json!("viewer")),
                ("reason".to_owned(), serde_json::json!("stale-revision")),
            ])),
            None,
        ),
        "The terminal output kept changing, so no consistent copy could be read before the deadline. Retry the read once the output settles.\n"
    );
}

/// Receipts open with one sentence, list affected resources under localized
/// labels, and keep revisions and request identities out of text.
#[test]
fn mutation_receipts_share_one_localized_shape() {
    let workspace = serde_json::json!({
        "action": "workspace.create",
        "outcome": "committed",
        "revision": "7",
        "projectId": null,
        "workspaceId": "a".repeat(32),
        "directoryId": "b".repeat(32),
        "terminalContextId": "c".repeat(32),
        "tabId": "d".repeat(32),
        "panelId": "e".repeat(32),
        "presentationId": "f".repeat(32),
        "label": "proj-a",
        "path": r"C:\work",
        "terminal": {"outcome": "started", "geometry": {"columns": 80, "rows": 24, "cellPixelWidth": 0, "cellPixelHeight": 0}},
    });
    let panel = "e".repeat(32);
    assert_eq!(
        CliText::new(Locale::EnUs).cli_receipt(&workspace),
        format!(
            "Created workspace \"proj-a\".\n\
             Project    Ungrouped\n\
             Workspace  {}\n\
             Tab        {}\n\
             Panel      {panel}\n\
             Label      \"proj-a\"\n\
             Directory  C:\\work\n\
             Terminal   started (80x24)\n\
             Next: orchbee terminal list --panel {panel}\n",
            "a".repeat(32),
            "d".repeat(32),
        )
        .replace("Ungrouped", CliText::new(Locale::EnUs).cli_ungrouped())
    );
    let zh = CliText::new(Locale::ZhCn).cli_receipt(&workspace);
    assert!(zh.starts_with("已创建工作区 \"proj-a\"。\n"), "{zh}");
    assert!(zh.contains(&format!("面板    {panel}\n")), "{zh}");
    assert!(zh.contains("终端    已启动 (80x24)\n"), "{zh}");
    assert!(zh.ends_with(&format!("下一步：orchbee terminal list --panel {panel}\n")));

    let split = serde_json::json!({
        "action": "panel.split",
        "outcome": "committed",
        "revision": "8",
        "panelId": "1".repeat(32),
        "newPanelId": "2".repeat(32),
        "splitId": "3".repeat(32),
        "direction": "right",
        "terminal": {"outcome": "pending"},
    });
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let text = CliText::new(locale).cli_receipt(&split);
        assert!(text.contains(&"2".repeat(32)), "{text}");
        assert!(
            !text.contains(&"3".repeat(32)),
            "split IDs stay in JSON: {text}"
        );
        assert!(
            text.ends_with(&format!("--panel {}\n", "2".repeat(32))),
            "{text}"
        );
    }
    assert!(
        CliText::new(Locale::EnUs)
            .cli_receipt(&split)
            .starts_with("Split the panel.\nNew panel  ")
    );

    let close = serde_json::json!({
        "action": "tab.close",
        "outcome": "accepted",
        "acceptedRevision": "9",
        "closeRequestId": "4".repeat(32),
        "terminalContextId": "5".repeat(32),
        "tabId": "6".repeat(32),
    });
    assert_eq!(
        CliText::new(Locale::EnUs).cli_receipt(&close),
        format!(
            "Closing tab; the session finishes the close in the background.\nTab  {}\n",
            "6".repeat(32)
        )
    );
    assert_eq!(
        CliText::new(Locale::ZhCn).cli_receipt(&close),
        format!(
            "正在关闭标签页；会话会在后台完成关闭。\n标签页  {}\n",
            "6".repeat(32)
        )
    );
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for receipt in [&workspace, &split, &close] {
            let text = CliText::new(locale).cli_receipt(receipt);
            for internal in [
                "revision",
                "Revision",
                "acceptedRevision",
                "closeRequestId",
                "requestId",
            ] {
                assert!(!text.contains(internal), "{internal} in {text}");
            }
        }
    }
}

#[test]
fn close_waits_name_the_closed_member_and_the_status_command() {
    assert_eq!(
        CliText::new(Locale::EnUs).cli_close_finished("tab", "build", 1),
        "Closed tab \"build\" (1 terminal).\n"
    );
    assert_eq!(
        CliText::new(Locale::EnUs).cli_close_finished("workspace", "api", 3),
        "Closed workspace \"api\" (3 terminals).\n"
    );
    assert_eq!(
        CliText::new(Locale::ZhCn).cli_close_finished("tab", "build", 2),
        "已关闭标签页 \"build\"（2 个终端）。\n"
    );
    let id = "a".repeat(32);
    assert!(
        CliText::new(Locale::EnUs)
            .cli_close_wait_hint(true, &id, 500)
            .ends_with(&format!(
                "Check with: orchbee workspace close-status --workspace {id}\n"
            ))
    );
    assert!(
        CliText::new(Locale::ZhCn)
            .cli_close_wait_hint(false, &id, 500)
            .ends_with("查看进度：orchbee tab list\n")
    );
    for locale in [Locale::EnUs, Locale::ZhCn] {
        assert!(
            CliText::new(locale)
                .cli_close_preview("tab")
                .contains("--confirm")
        );
    }
}

/// A Workspace create refused before any Workspace committed applied nothing,
/// so its text never calls the creation incomplete. A later stage after the
/// commit, or an unknown outcome that may have committed, never claims that
/// nothing was created.
#[test]
fn a_workspace_create_that_applied_nothing_never_says_incomplete() {
    let document = |workspace: serde_json::Value, stage: &str, kind: &str| {
        serde_json::json!({
            "schema": "orchbee.workspace.creation", "version": 1, "resolvedLaunch": null,
            "workspace": workspace,
            "directory": {"policy": "explicit", "source": "explicit",
                "path": "C:/System Volume Information", "sourcePanelId": null},
            "clientId": null, "requestedFocus": false, "focus": null, "stage": stage,
            "failure": {"kind": kind,
                "details": {"stage": "command", "reason": "directory-unavailable"},
                "source": null, "schema": "orchbee.cli.failure", "version": 1},
            "outcome": "incomplete"
        })
    };
    let committed = serde_json::json!({"schema": "orchbee.mutation.result", "version": 1,
        "source": null, "result": {"action": "workspace.create", "outcome": "committed",
            "workspaceId": "a".repeat(32), "label": "demo"}});
    for (locale, incomplete, not_created) in [
        (Locale::EnUs, "incomplete", "not created"),
        (Locale::ZhCn, "未完成", "未创建"),
    ] {
        let text = CliText::new(locale);
        for kind in [
            "service-unavailable",
            "rejected",
            "denied",
            "timeout",
            "cancelled",
            "transport-loss",
        ] {
            let refused =
                text.cli_workspace_creation(&document(serde_json::Value::Null, "workspace", kind));
            assert!(
                !refused.contains(incomplete),
                "{locale:?} {kind}: {refused}"
            );
            assert!(
                refused.contains(not_created),
                "{locale:?} {kind}: {refused}"
            );
        }
        for stage in ["terminal", "focus"] {
            let partial =
                text.cli_workspace_creation(&document(committed.clone(), stage, "rejected"));
            assert!(
                partial.contains(incomplete),
                "{locale:?} {stage}: {partial}"
            );
            assert!(
                !partial.contains(not_created),
                "{locale:?} {stage}: {partial}"
            );
        }
        let unknown = text.cli_workspace_creation(&document(
            serde_json::Value::Null,
            "workspace",
            "outcome-unknown",
        ));
        assert!(!unknown.contains(not_created), "{locale:?}: {unknown}");
    }
}
