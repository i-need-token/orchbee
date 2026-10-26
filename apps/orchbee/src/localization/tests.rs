use std::{collections::HashMap, ffi::OsString};

use crate::client_view::{BackendAction, Notice};

use super::{Locale, select_locale};

#[test]
fn compact_terminal_end_copy_keeps_complete_results_within_twenty_cells_in_both_locales() {
    use crate::terminal_projection::TerminalEnd;
    use orchbee_terminal_protocol::ExitResult;
    for (end, english, chinese) in [
        (
            TerminalEnd::Exited(ExitResult::ExitCode(0)),
            "Exited 0",
            "已退出 0",
        ),
        (
            TerminalEnd::Exited(ExitResult::ExitCode(u32::MAX)),
            "Exited 4294967295",
            "已退出 4294967295",
        ),
        (
            TerminalEnd::Exited(ExitResult::Signal(u32::MAX)),
            "Signal 4294967295",
            "信号 4294967295",
        ),
        (
            TerminalEnd::Exited(ExitResult::Unknown(0)),
            "Exit code unknown",
            "退出码未知",
        ),
        (TerminalEnd::Failed, "Terminal failed", "终端运行故障"),
    ] {
        for (locale, expected) in [(Locale::EnUs, english), (Locale::ZhCn, chinese)] {
            let text = locale.terminal_end_compact(end);
            assert_eq!(text, expected);
            assert!(crate::terminal_text::display_width(&text) <= 20);
        }
    }
}

#[test]
fn agent_start_preserves_the_caller_context_usage_hint() {
    let mut failure = crate::cli::Failure::new(crate::cli::FailureKind::InvalidUsage)
        .failure_schema("orchbee.cli.failure.agent-start");
    failure.usage = Some(Box::new(
        crate::invocation::UsageReason::CallerContextUnavailable,
    ));
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let message = locale.cli_failure(&failure);
        assert!(message.contains("--panel"), "{message}");
        assert!(
            !message.contains("confirmed") && !message.contains("确认"),
            "{message}"
        );
    }
}

#[test]
fn missing_agent_panel_reports_the_target_and_discovery_command() {
    let failure = crate::cli::Failure::new(crate::cli::FailureKind::NotFound)
        .failure_schema("orchbee.cli.failure.agent-start")
        .details(serde_json::Map::from_iter([
            ("reason".into(), serde_json::json!("startup-failed")),
            ("cause".into(), serde_json::json!({"kind":"not-found"})),
        ]));
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let message = locale.cli_failure(&failure);
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
            assert!(locale.cli_label_target_hint(label).is_some(), "{label}");
        }
        for label in ["example", "中文 workspace", "@unknown:abc", "123"] {
            assert!(locale.cli_label_target_hint(label).is_none(), "{label}");
        }
    }
}

#[cfg(unix)]
mod location_labels;

#[cfg(windows)]
#[test]
fn tab_menu_copy_localizes_scope_and_preserves_close_and_rename_consequences() {
    use std::num::NonZeroUsize;

    use crate::{
        action_menu::{MenuAction, MenuTarget, MenuUnavailable, TabMenuScope},
        workbench_client::CloseConfirmationKind,
    };
    use orchbee_workbench_protocol::WorkbenchTabId;

    let locale = Locale::ZhCn;
    let count = NonZeroUsize::new(2).unwrap();
    assert_eq!(locale.menu_action(&MenuAction::PinTab), "固定标签页");
    assert_eq!(
        locale.menu_action(&MenuAction::CloseSelectedTabs),
        "关闭标签页…"
    );
    assert_eq!(
        locale.menu_tab_action(
            &MenuAction::CloseSelectedTabs,
            TabMenuScope::Selected(count)
        ),
        "关闭所选 2 个标签页…"
    );
    assert_eq!(
        locale.menu_tab_action(&MenuAction::CloseGroup, TabMenuScope::Group(count)),
        "关闭分组（2 个标签页）…"
    );
    assert_eq!(
        locale.menu_tab_action(&MenuAction::Ungroup, TabMenuScope::Group(count)),
        "解散分组，保留 2 个标签页"
    );
    let tab = WorkbenchTabId::parse("00000000000000000000000000000023").unwrap();
    assert_eq!(
        locale.menu_tab_path(&MenuTarget::Tab(tab.clone()), TabMenuScope::Addressed, ">"),
        "标签页"
    );
    assert_eq!(
        locale.menu_tab_path(
            &MenuTarget::Tab(tab.clone()),
            TabMenuScope::Selected(count),
            ">"
        ),
        "所选 2 个标签页"
    );
    assert_eq!(
        locale.menu_tab_path(
            &MenuTarget::TabGroupChoices(tab),
            TabMenuScope::Addressed,
            ">"
        ),
        "标签页 > 加入分组"
    );
    assert_eq!(
        locale.menu_unavailable(MenuUnavailable::NoMatchingTabs, true),
        "无匹配标签页"
    );
    assert_eq!(
        locale.tab_drag_hint(2, true),
        "移动 2 个标签页 · 松手放置 · Esc 取消"
    );
    assert_eq!(
        locale.tab_drag_hint(2, false),
        "移动 2 个标签页 · 此处松手取消 · Esc 取消"
    );
    assert_eq!(locale.close_set_title(2), "关闭 2 个标签页？");
    assert_eq!(
        locale.close_title(CloseConfirmationKind::Resume),
        "继续关闭此标签页？"
    );
    assert_eq!(
        locale.close_set_warning(),
        "关闭将终止这些标签页中运行的任务，影响所有连接到此工作区的人。标签页将逐个关闭；失败时停止后续关闭。"
    );
    assert_eq!(locale.rename_title(), "重命名标签页");
    assert_eq!(
        locale.rename_warning(),
        "新的标签页名称会同步给所有连接到此工作区的人。"
    );
}

#[cfg(windows)]
#[test]
fn help_page_bindings_follow_the_configured_prefix() {
    use crate::client_view::{ClientView, KeyLabelGlyphs, PrefixAction};
    use orchbee_local_runtime::{KeyChord, KeySettings};
    for (remapped, prefix) in [(false, "Ctrl+b"), (true, "Ctrl+a")] {
        let mut view = ClientView::new(80);
        if remapped {
            view.configure_keys(&KeySettings {
                commands: Vec::new(),
                overrides: Vec::new(),
                prefix: KeyChord {
                    key: "a".into(),
                    ctrl: true,
                    alt: false,
                    shift: false,
                    super_key: false,
                },
            });
        }
        assert_eq!(
            view.bindings()
                .action_labels_with_glyphs(PrefixAction::HelpPagePrevious, KeyLabelGlyphs::Unicode),
            vec![format!("{prefix} PageUp")]
        );
        assert_eq!(
            view.bindings()
                .action_labels_with_glyphs(PrefixAction::HelpPageNext, KeyLabelGlyphs::Unicode),
            vec![format!("{prefix} PageDown")]
        );
    }
}

#[test]
fn uses_locale_precedence_and_skips_empty_values() {
    let empty_override = HashMap::from([
        ("LANG", OsString::from("zh_CN.UTF-8")),
        ("LC_MESSAGES", OsString::from("en_US.UTF-8")),
        ("LC_ALL", OsString::from("")),
    ]);
    assert_eq!(
        select_locale(|name| empty_override.get(name).cloned()),
        Locale::EnUs
    );

    let explicit_override = HashMap::from([
        ("LANG", OsString::from("en_US.UTF-8")),
        ("LC_MESSAGES", OsString::from("en_US.UTF-8")),
        ("LC_ALL", OsString::from("zh_CN.UTF-8")),
    ]);
    assert_eq!(
        select_locale(|name| explicit_override.get(name).cloned()),
        Locale::ZhCn
    );
}

#[test]
fn recognizes_chinese_prefix_case_insensitively() {
    for value in ["zh", "zh-CN", "zh_CN.UTF-8", "ZH_TW"] {
        assert_eq!(
            select_locale(|name| (name == "LANG").then(|| OsString::from(value))),
            Locale::ZhCn
        );
    }
}

#[test]
fn falls_back_to_english_for_absent_other_and_non_unicode_values() {
    assert_eq!(select_locale(|_name| None), Locale::EnUs);
    assert_eq!(
        select_locale(|name| (name == "LANG").then(|| OsString::from("fr_FR.UTF-8"))),
        Locale::EnUs
    );

    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;

        assert_eq!(
            select_locale(|name| (name == "LANG").then(|| OsString::from_vec(vec![0xff]))),
            Locale::EnUs
        );
    }
}

#[test]
fn help_page_binding_dispatches_scroll_actions() {
    use crate::client_view::{
        ClientAction, ClientActionDisposition, ClientTransition, ClientView, KeyCode, KeyEvent,
        Modifiers, Overlay, PrefixInput,
    };
    fn press(view: &mut ClientView, input: KeyCode, mods: Modifiers) -> ClientTransition {
        let transition = view.decide(PrefixInput::Key(KeyEvent::press(input, mods)));
        view.commit(transition.clone(), ClientActionDisposition::KeepNavigator);
        transition
    }

    let mut view = ClientView::new(120);
    for (code, expected_page) in [(KeyCode::PageUp, 0), (KeyCode::PageDown, 8)] {
        press(&mut view, KeyCode::Char('b'), Modifiers::CTRL);
        press(&mut view, KeyCode::Char('?'), Modifiers::NONE);
        assert_eq!(view.overlay(), Some(Overlay::ShortcutHelp));
        press(&mut view, KeyCode::Char('b'), Modifiers::CTRL);
        let paged = press(&mut view, code, Modifiers::NONE);
        assert_eq!(
            paged.action(),
            Some(ClientAction::PageShortcutHelp),
            "Ctrl+B {code:?}"
        );
        assert_eq!(view.help_page(), expected_page, "Ctrl+B {code:?}");
        // Close the help again for the next iteration: the page resets.
        press(&mut view, KeyCode::Char('b'), Modifiers::CTRL);
        press(&mut view, KeyCode::Char('?'), Modifiers::NONE);
        assert_eq!(view.help_page(), 0);
    }
}

#[test]
fn panel_shortcut_discovery_and_dispatch_match_in_both_locales() {
    use crate::client_view::{
        ClientAction, ClientActionDisposition, ClientView, KeyCode, KeyEvent, KeyKind,
        KeyLabelGlyphs, Modifiers, PanelDirection, PrefixAction, PrefixInput,
    };
    fn key(code: KeyCode, mods: Modifiers) -> PrefixInput {
        PrefixInput::Key(KeyEvent {
            code,
            mods,
            kind: KeyKind::Press,
        })
    }

    let cases = [
        (
            KeyCode::Char('h'),
            ClientAction::MovePanelFocus(PanelDirection::Left),
            "Focus panel left",
            "向左聚焦面板",
        ),
        (
            KeyCode::Char('j'),
            ClientAction::MovePanelFocus(PanelDirection::Down),
            "Focus panel down",
            "向下聚焦面板",
        ),
        (
            KeyCode::Char('k'),
            ClientAction::MovePanelFocus(PanelDirection::Up),
            "Focus panel up",
            "向上聚焦面板",
        ),
        (
            KeyCode::Char('l'),
            ClientAction::MovePanelFocus(PanelDirection::Right),
            "Focus panel right",
            "向右聚焦面板",
        ),
        (
            KeyCode::Char('-'),
            ClientAction::SplitPanel(PanelDirection::Down),
            "Split panel down",
            "向下分割面板",
        ),
        (
            KeyCode::Char('_'),
            ClientAction::SplitPanel(PanelDirection::Up),
            "Split panel up",
            "向上分割面板",
        ),
        (
            KeyCode::Char('\\'),
            ClientAction::SplitPanel(PanelDirection::Left),
            "Split panel left",
            "向左分割面板",
        ),
        (
            KeyCode::Char('|'),
            ClientAction::SplitPanel(PanelDirection::Right),
            "Split panel right",
            "向右分割面板",
        ),
        (
            KeyCode::Char('X'),
            ClientAction::ClosePanel,
            "Close panel",
            "关闭面板",
        ),
        (
            KeyCode::Char('H'),
            ClientAction::MovePanel(PanelDirection::Left),
            "Move panel left",
            "向左移动面板",
        ),
        (
            KeyCode::Char('J'),
            ClientAction::MovePanel(PanelDirection::Down),
            "Move panel down",
            "向下移动面板",
        ),
        (
            KeyCode::Char('K'),
            ClientAction::MovePanel(PanelDirection::Up),
            "Move panel up",
            "向上移动面板",
        ),
        (
            KeyCode::Char('L'),
            ClientAction::MovePanel(PanelDirection::Right),
            "Move panel right",
            "向右移动面板",
        ),
    ];
    let inventory = ClientView::new(120);
    let owner_entries = inventory
        .bindings()
        .discovery_entries(KeyLabelGlyphs::Unicode);
    let english = Locale::EnUs.shortcut_help_entries(&inventory, true, KeyLabelGlyphs::Unicode);
    let chinese = Locale::ZhCn.shortcut_help_entries(&inventory, true, KeyLabelGlyphs::Unicode);
    for (code, expected, en_title, zh_title) in cases {
        let owned_key = owner_entries
            .iter()
            .find(|entry| entry.action == PrefixAction::Client(expected.clone()))
            .expect("panel action remains in the input owner")
            .key
            .as_str();
        assert!(
            english
                .iter()
                .any(|entry| { entry.key == owned_key && entry.title == en_title }),
            "English Help should show {en_title:?} for owned key {owned_key:?} ({expected:?})"
        );
        assert!(
            chinese
                .iter()
                .any(|entry| { entry.key == owned_key && entry.title == zh_title }),
            "Chinese Help should show {zh_title:?} for owned key {owned_key:?} ({expected:?})"
        );

        let mut view = ClientView::new(120);
        let prefix = view.decide(key(KeyCode::Char('b'), Modifiers::CTRL));
        view.commit(prefix, ClientActionDisposition::KeepNavigator);
        let transition = view.decide(key(code, Modifiers::NONE));
        assert_eq!(transition.action(), Some(expected), "Ctrl+B {code:?}");
    }

    for (action, en, zh) in [
        (
            BackendAction::SplitPanel,
            "Panel split unavailable · no command was sent",
            "面板拆分不可用 · 未发送命令",
        ),
        (
            BackendAction::ClosePanel,
            "Panel close unavailable · no command was sent",
            "面板关闭不可用 · 未发送命令",
        ),
        (
            BackendAction::MovePanel,
            "Panel move unavailable · no command was sent",
            "面板移动不可用 · 未发送命令",
        ),
    ] {
        let notice = Some(Notice::BackendUnavailable(action));
        assert_eq!(Locale::EnUs.shell_notice(notice), en);
        assert_eq!(Locale::ZhCn.shell_notice(notice), zh);
    }
}

#[cfg(unix)]
#[test]
fn settled_setup_notices_never_hide_later_failures_or_retirement() {
    use crate::workbench_client::{SetupStatus, WorkbenchClientNotice};
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for setup in [
            SetupStatus::ProjectCreated,
            SetupStatus::WorkspaceCreated,
            SetupStatus::DirectoryAccepted,
            SetupStatus::NotApplied,
        ] {
            for notice in [
                WorkbenchClientNotice::CreateNotCreated,
                WorkbenchClientNotice::RenameNotRenamed,
                WorkbenchClientNotice::LocalBusy,
            ] {
                assert_eq!(
                    locale.native_status_with_setup(
                        "phase".to_owned(),
                        false,
                        Some(notice),
                        Some(setup),
                        true
                    ),
                    locale.native_notice(notice)
                );
                assert_eq!(
                    locale.native_status_with_setup(
                        "retired outcome unknown".to_owned(),
                        true,
                        Some(notice),
                        Some(setup),
                        true
                    ),
                    "retired outcome unknown"
                );
            }
        }
    }
}

#[cfg(unix)]
#[test]
fn current_failure_keeps_the_known_setup_recovery_fact() {
    use crate::workbench_client::{SetupStatus, WorkbenchClientNotice};
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for setup in [
            SetupStatus::CommittedNeedRefresh,
            SetupStatus::OutcomeUnknown,
        ] {
            let notice = WorkbenchClientNotice::LocalBusy;
            let status = locale.native_status_with_setup(
                "phase".to_owned(),
                false,
                Some(notice),
                Some(setup),
                false,
            );
            assert!(status.starts_with(&locale.native_notice(notice)));
            assert!(status.contains(locale.setup_status(setup)));
            let retired = locale.native_status_with_setup(
                "retired".to_owned(),
                true,
                None,
                Some(setup),
                false,
            );
            assert!(retired.starts_with("retired"));
            assert!(
                retired.contains(if setup == SetupStatus::CommittedNeedRefresh {
                    locale.setup_committed_fact()
                } else {
                    locale.setup_status(setup)
                })
            );
            assert!(!retired.contains("Ctrl+B w"));
            assert_eq!(
                locale.native_status_with_setup(
                    "phase".to_owned(),
                    false,
                    None,
                    Some(setup),
                    false
                ),
                locale.setup_status(setup)
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn workspace_created_only_offers_directory_setup_for_an_eligible_current_context() {
    use crate::workbench_client::SetupStatus;
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let setup = SetupStatus::WorkspaceCreated;
        let eligible =
            locale.native_status_with_setup("phase".to_owned(), false, None, Some(setup), true);
        assert_eq!(
            eligible,
            match locale {
                Locale::EnUs => "Workspace created · its first directory is not configured",
                Locale::ZhCn => "工作区已创建 · 尚未配置首个目录",
            }
        );
        assert!(!eligible.contains("Ctrl+B"));
        let configured_or_nonempty =
            locale.native_status_with_setup("phase".to_owned(), false, None, Some(setup), false);
        assert_eq!(configured_or_nonempty, locale.setup_status(setup));
        assert!(!configured_or_nonempty.contains("Ctrl+B e"));
        let retired =
            locale.native_status_with_setup("retired".to_owned(), true, None, Some(setup), true);
        assert_eq!(retired, "retired");
    }
}

/// The `invalid-usage` stderr diagnostic names one concrete cause plus a real
/// help hint in both locales; echoed tokens pass through the quoting/escape
/// layer with its 256-scalar prefix bound.
#[test]
fn cli_usage_diagnostics_are_localized_and_escaped() {
    use crate::cli::{Failure, FailureKind};
    use crate::invocation::{UsageContext, UsageReason};

    let usage_failure = |usage: UsageReason| Failure {
        kind: FailureKind::InvalidUsage,
        details: None,
        source: None,
        failure_schema: None,
        io_diagnostic: None,
        candidate_display: None,
        usage: Some(Box::new(usage)),
    };

    // Unknown command echoes the escaped word and keeps the root hint.
    let failure = usage_failure(UsageReason::UnknownCommand {
        context: UsageContext::Root,
        word: "bo\u{1b}gus".to_owned(),
    });
    assert_eq!(
        Locale::EnUs.cli_failure(&failure),
        "Unknown command \"bo\\u{001b}gus\". Run 'orchbee --help'.\n"
    );
    assert_eq!(
        Locale::ZhCn.cli_failure(&failure),
        "未知命令 \"bo\\u{001b}gus\"。请运行 ‘orchbee --help’。\n"
    );

    // Command context anchors the real `<path> --help` hint.
    let failure = usage_failure(UsageReason::UnexpectedOption {
        context: UsageContext::Path("workspace list"),
        option: "--workspace",
    });
    assert_eq!(
        Locale::EnUs.cli_failure(&failure),
        "'--workspace' is not a valid option for 'orchbee workspace list'. Run 'orchbee workspace list --help'.\n"
    );
    assert_eq!(
        Locale::ZhCn.cli_failure(&failure),
        "'--workspace' 不是 'orchbee workspace list' 的有效选项。请运行 ‘orchbee workspace list --help’。\n"
    );

    // The argv-envelope reason never echoes argv content.
    let failure = usage_failure(UsageReason::ArgumentEnvelope);
    assert_eq!(
        Locale::EnUs.cli_failure(&failure),
        "Invalid invocation: the argument list must be Unicode and its total size may not exceed 64 KiB. Run 'orchbee --help'.\n"
    );
    assert_eq!(
        Locale::ZhCn.cli_failure(&failure),
        "调用方式无效：参数必须为 Unicode，且总大小不得超过 64 KiB。请运行 ‘orchbee --help’。\n"
    );

    // A reason-free InvalidUsage keeps the original generic text.
    let failure = Failure {
        kind: FailureKind::InvalidUsage,
        details: None,
        source: None,
        failure_schema: None,
        io_diagnostic: None,
        candidate_display: None,
        usage: None,
    };
    assert_eq!(
        Locale::EnUs.cli_failure(&failure),
        "Invalid invocation. Run 'orchbee --help'.\n"
    );
    assert_eq!(
        Locale::ZhCn.cli_failure(&failure),
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
        io_diagnostic: None,
        candidate_display: None,
        usage: None,
    };
    assert_eq!(
        Locale::EnUs.cli_failure(&failure),
        "No tab matches \"x\".\n"
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

    // The opaque failure itself carries no I/O detail to stdout or stderr.
    let failure = cohort_failure(&std::io::Error::from_raw_os_error(110));
    let expected_en = if cfg!(windows) {
        "The native development cohort could not be validated. Verify that all cohort files are present, readable, and unchanged. Run `orchbee status client --format json` for I/O details.\n"
    } else {
        "The native development cohort could not be validated. Verify that all cohort files are present, readable, and unchanged.\n"
    };
    let expected_zh = if cfg!(windows) {
        "无法验证原生开发 cohort。请确认所有 cohort 文件均存在、可读取且未被修改。运行 `orchbee status client --format json` 查看 I/O 详情。\n"
    } else {
        "无法验证原生开发 cohort。请确认所有 cohort 文件均存在、可读取且未被修改。\n"
    };
    assert_eq!(Locale::EnUs.cli_failure(&failure), expected_en);
    assert_eq!(Locale::ZhCn.cli_failure(&failure), expected_zh);
    for (locale, expected) in [(Locale::EnUs, expected_en), (Locale::ZhCn, expected_zh)] {
        let output = failure_output(
            cohort_failure(&std::io::Error::from_raw_os_error(110)),
            Some(OutputFormat::Text),
            locale,
        );
        assert_eq!(output.status, 1);
        assert!(output.stdout.is_empty());
        assert_eq!(output.stderr, expected);
    }

    // Exact artifact copy is emitted only by the typed missing-artifact path.
    assert_eq!(
        Locale::EnUs.cli_cohort_diagnostic(Some("orchbee-server.exe")),
        if cfg!(windows) {
            "Required native sibling `orchbee-server.exe` is missing from the development cohort. Rebuild the development cohort or reinstall the complete application. Run `orchbee status client --format json` for I/O details.\n"
        } else {
            "Required native sibling `orchbee-server.exe` is missing from the development cohort. Rebuild the development cohort or reinstall the complete application.\n"
        }
    );
    assert_eq!(
        Locale::ZhCn.cli_cohort_diagnostic(Some("orchbee-server.exe")),
        if cfg!(windows) {
            "原生开发 cohort 缺少必需的同级文件 `orchbee-server.exe`。请重新构建完整 cohort 或重新安装完整应用。运行 `orchbee status client --format json` 查看 I/O 详情。\n"
        } else {
            "原生开发 cohort 缺少必需的同级文件 `orchbee-server.exe`。请重新构建完整 cohort 或重新安装完整应用。\n"
        }
    );

    // A failure without typed cohort evidence does not invent a missing file.
    let failure = Failure::new(FailureKind::CheckFailed).details(stage_details("cohort"));
    assert_eq!(
        Locale::EnUs.cli_failure(&failure),
        if cfg!(windows) {
            "The native development cohort could not be validated. Verify that all cohort files are present, readable, and unchanged. Run `orchbee status client --format json` for I/O details.\n"
        } else {
            "The native development cohort could not be validated. Verify that all cohort files are present, readable, and unchanged.\n"
        }
    );
    assert_eq!(
        Locale::ZhCn.cli_failure(&failure),
        if cfg!(windows) {
            "无法验证原生开发 cohort。请确认所有 cohort 文件均存在、可读取且未被修改。运行 `orchbee status client --format json` 查看 I/O 详情。\n"
        } else {
            "无法验证原生开发 cohort。请确认所有 cohort 文件均存在、可读取且未被修改。\n"
        }
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
        Locale::EnUs.cli_service_unavailable_for_selection(&failure, Some("nosuch")),
        "No running local session named `nosuch` was found. Start it with: orchbee --session nosuch server start.\n"
    );
    assert_eq!(
        Locale::ZhCn.cli_service_unavailable_for_selection(&failure, Some("nosuch")),
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
        let text = locale.cli_client_control(&document);
        assert!(text.contains(incomplete) && text.contains(reason), "{text}");
        assert!(
            text.contains(&format!("orchbee client control-status {id}")),
            "{text}"
        );
        document["schema"] = serde_json::json!("orchbee.client.control");
        for (state, label) in [("popup-opened", opened), ("popup-closed", closed)] {
            document["observation"]["reply"]["outcome"]["operation"]["status"]["kind"] =
                serde_json::json!(state);
            assert!(locale.cli_client_control(&document).contains(label));
        }
    }
}

#[test]
fn viewer_read_failures_explain_the_cause_without_internal_reason_codes() {
    use crate::cli::{Failure, FailureKind};

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
            let text = locale.cli_service_unavailable_for_selection(&failure, None);
            assert!(!text.contains(reason), "{reason}: {text}");
            assert!(text.contains(hint), "{reason}: {text}");
        }
    }
    assert_eq!(
        Locale::EnUs.cli_service_unavailable_for_selection(
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
        Locale::EnUs.cli_receipt(&workspace),
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
        .replace("Ungrouped", Locale::EnUs.cli_ungrouped())
    );
    let zh = Locale::ZhCn.cli_receipt(&workspace);
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
        let text = locale.cli_receipt(&split);
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
        Locale::EnUs
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
        Locale::EnUs.cli_receipt(&close),
        format!(
            "Closing tab; the session finishes the close in the background.\nTab  {}\n",
            "6".repeat(32)
        )
    );
    assert_eq!(
        Locale::ZhCn.cli_receipt(&close),
        format!(
            "正在关闭标签页；会话会在后台完成关闭。\n标签页  {}\n",
            "6".repeat(32)
        )
    );
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for receipt in [&workspace, &split, &close] {
            let text = locale.cli_receipt(receipt);
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
        Locale::EnUs.cli_close_finished("tab", "build", 1),
        "Closed tab \"build\" (1 terminal).\n"
    );
    assert_eq!(
        Locale::EnUs.cli_close_finished("workspace", "api", 3),
        "Closed workspace \"api\" (3 terminals).\n"
    );
    assert_eq!(
        Locale::ZhCn.cli_close_finished("tab", "build", 2),
        "已关闭标签页 \"build\"（2 个终端）。\n"
    );
    let id = "a".repeat(32);
    assert!(
        Locale::EnUs
            .cli_close_wait_hint(true, &id, 500)
            .ends_with(&format!(
                "Check with: orchbee workspace close-status --workspace {id}\n"
            ))
    );
    assert!(
        Locale::ZhCn
            .cli_close_wait_hint(false, &id, 500)
            .ends_with("查看进度：orchbee tab list\n")
    );
    for locale in [Locale::EnUs, Locale::ZhCn] {
        assert!(locale.cli_close_preview("tab").contains("--confirm"));
    }
}
