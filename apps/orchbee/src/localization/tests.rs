use std::{collections::HashMap, ffi::OsString};

use crate::client_view::{BackendAction, Notice};

use super::{Locale, ShellText, select_locale};

#[cfg(unix)]
mod location_labels;

/// Flattens `(padded chord, description)` help pairs back into the single
/// rendered row string so the registry-derived text can be compared whole.
fn joined_shortcut_rows(text: &ShellText) -> Vec<String> {
    text.shortcut_rows()
        .iter()
        .map(|(chord, desc)| format!("{chord}{desc}"))
        .collect()
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
fn ships_complete_distinct_shell_copy_for_both_locales() {
    let english = Locale::EnUs.shell();
    let chinese = Locale::ZhCn.shell();

    assert_eq!(chinese.shortcut_rows().len(), english.shortcut_rows().len());
    assert_ne!(english.backend_status, chinese.backend_status);
    assert_ne!(english.terminal_unavailable, chinese.terminal_unavailable);
    #[cfg(windows)]
    for copy in [&english, &chinese] {
        assert!(
            copy.shortcut_rows()
                .iter()
                .any(|(chord, desc)| chord.trim() == "Ctrl+B { / }" && !desc.is_empty())
        );
    }

    for shortcut in [
        "Ctrl+B a",
        "Ctrl+B e",
        "Ctrl+B r",
        "Ctrl+B R",
        "Ctrl+B c",
        "Ctrl+B ,",
        "Ctrl+B x",
        "Ctrl+B < / >",
        "Ctrl+B n / p",
        "Ctrl+B [ / ]",
        "Ctrl+B 1..9",
        "Ctrl+B z",
        "Ctrl+B g",
        "Ctrl+B w",
        "Ctrl+B m",
        "Ctrl+B PageUp / PageDown",
    ] {
        assert!(
            joined_shortcut_rows(&english)
                .iter()
                .any(|line| line.starts_with(shortcut))
        );
        assert!(
            joined_shortcut_rows(&chinese)
                .iter()
                .any(|line| line.starts_with(shortcut))
        );
    }
    assert!(
        joined_shortcut_rows(&english)
            .iter()
            .any(|row| row == "Ctrl+B w                 Toggle Workbench navigator")
    );
    assert!(
        joined_shortcut_rows(&chinese)
            .iter()
            .any(|row| row == "Ctrl+B w                 切换 Workbench 导航器")
    );
    assert!(joined_shortcut_rows(&english).iter().any(|row| row
        == "Ctrl+B [ / ]             Select previous / next Workbench location (wraps; requires Site)"));
    assert!(joined_shortcut_rows(&chinese).iter().any(|row| row
        == "Ctrl+B [ / ]             选择上一个 / 下一个 Workbench 位置（首尾循环；需要 Site）"));
}

/// The help-paging row is registry-derived like every other row: the same
/// table the dispatcher matches generates the displayed chord and the
/// localized description, in both locales.
#[test]
fn help_page_row_matches_the_registry_bindings_in_both_locales() {
    use crate::client_view::{
        ClientAction, ClientTransition, ClientView, KeyCode, KeyEvent, Modifiers, Overlay,
        PrefixInput, SessionActionDisposition,
    };
    fn press(view: &mut ClientView, input: KeyCode, mods: Modifiers) -> ClientTransition {
        let transition = view.decide(PrefixInput::Key(KeyEvent::press(input, mods)));
        view.commit(transition.clone(), SessionActionDisposition::KeepNavigator);
        transition
    }

    let english = Locale::EnUs.shell();
    let chinese = Locale::ZhCn.shell();
    assert!(
        joined_shortcut_rows(&english)
            .iter()
            .any(|row| row == "Ctrl+B PageUp / PageDown Page this shortcut help while it is open")
    );
    assert!(
        joined_shortcut_rows(&chinese)
            .iter()
            .any(|row| row == "Ctrl+B PageUp / PageDown 在帮助打开时翻看此快捷键帮助的页面")
    );

    // The displayed chords resolve through the registry to the help-page
    // action — never a second hard-coded table.
    let mut view = ClientView::new(120);
    for (code, expected_page) in [(KeyCode::PageUp, 0), (KeyCode::PageDown, 1)] {
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
fn panel_shortcut_rows_match_their_client_actions_in_both_locales() {
    use crate::client_view::{
        ClientAction, ClientView, KeyCode, KeyEvent, KeyKind, Modifiers, PanelDirection,
        PrefixInput, SessionActionDisposition,
    };
    fn key(code: KeyCode, mods: Modifiers) -> PrefixInput {
        PrefixInput::Key(KeyEvent {
            code,
            mods,
            kind: KeyKind::Press,
        })
    }

    let english = Locale::EnUs.shell();
    let chinese = Locale::ZhCn.shell();
    for (en, zh, bindings) in [
        (
            "Ctrl+B h/j/k/l           Move between Panels (requires Site)",
            "Ctrl+B h/j/k/l           在 Panel 之间移动（需要 Site）",
            &[
                (
                    KeyCode::Char('h'),
                    ClientAction::MovePanelFocus(PanelDirection::Left),
                ),
                (
                    KeyCode::Char('j'),
                    ClientAction::MovePanelFocus(PanelDirection::Down),
                ),
                (
                    KeyCode::Char('k'),
                    ClientAction::MovePanelFocus(PanelDirection::Up),
                ),
                (
                    KeyCode::Char('l'),
                    ClientAction::MovePanelFocus(PanelDirection::Right),
                ),
            ][..],
        ),
        (
            "Ctrl+B - _ \\ |           Split focused Panel below/above/left/right (requires Site)",
            "Ctrl+B - _ \\ |           将焦点 Panel 向下/上/左/右分屏（需要 Site）",
            &[
                (
                    KeyCode::Char('-'),
                    ClientAction::SplitPanel(PanelDirection::Down),
                ),
                (
                    KeyCode::Char('_'),
                    ClientAction::SplitPanel(PanelDirection::Up),
                ),
                (
                    KeyCode::Char('\\'),
                    ClientAction::SplitPanel(PanelDirection::Left),
                ),
                (
                    KeyCode::Char('|'),
                    ClientAction::SplitPanel(PanelDirection::Right),
                ),
            ][..],
        ),
        (
            "Ctrl+B X                 Close focused Panel (requires Site)",
            "Ctrl+B X                 关闭焦点 Panel（需要 Site）",
            &[(KeyCode::Char('X'), ClientAction::ClosePanel)][..],
        ),
        (
            "Ctrl+B H/J/K/L           Move focused Panel to that side (requires Site)",
            "Ctrl+B H/J/K/L           将焦点 Panel 移向该侧（需要 Site）",
            &[
                (
                    KeyCode::Char('H'),
                    ClientAction::MovePanel(PanelDirection::Left),
                ),
                (
                    KeyCode::Char('J'),
                    ClientAction::MovePanel(PanelDirection::Down),
                ),
                (
                    KeyCode::Char('K'),
                    ClientAction::MovePanel(PanelDirection::Up),
                ),
                (
                    KeyCode::Char('L'),
                    ClientAction::MovePanel(PanelDirection::Right),
                ),
            ][..],
        ),
    ] {
        assert!(
            joined_shortcut_rows(&english)
                .iter()
                .any(|row| row.as_str() == en),
            "en-US row: {en}"
        );
        assert!(
            joined_shortcut_rows(&chinese)
                .iter()
                .any(|row| row.as_str() == zh),
            "zh-CN row: {zh}"
        );
        for (code, expected) in bindings {
            let mut view = ClientView::new(120);
            let prefix = view.decide(key(KeyCode::Char('b'), Modifiers::CTRL));
            view.commit(prefix, SessionActionDisposition::KeepNavigator);
            let transition = view.decide(key(*code, Modifiers::NONE));
            assert_eq!(
                transition.action(),
                Some(expected.clone()),
                "Ctrl+B {code:?}"
            );
        }
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

#[test]
fn localizes_context_navigation_backend_failures_in_both_locales() {
    let next = Some(Notice::BackendUnavailable(BackendAction::SelectNextContext));
    let previous = Some(Notice::BackendUnavailable(
        BackendAction::SelectPreviousContext,
    ));

    assert_eq!(
        Locale::EnUs.shell_notice(next),
        "Workbench location navigation unavailable · no command was sent"
    );
    assert_eq!(
        Locale::EnUs.shell_notice(previous),
        "Workbench location navigation unavailable · no command was sent"
    );
    assert_eq!(
        Locale::ZhCn.shell_notice(next),
        "Workbench 位置导航不可用 · 未发送命令"
    );
    assert_eq!(
        Locale::ZhCn.shell_notice(previous),
        "Workbench 位置导航不可用 · 未发送命令"
    );
}

#[test]
fn localizes_checkpoint_recovery_outcomes_in_both_locales() {
    assert_eq!(
        Locale::EnUs.shell_notice(Some(Notice::CheckpointRecovered)),
        "Unreadable workspace snapshots retained; restored a valid snapshot"
    );
    assert_eq!(
        Locale::ZhCn.shell_notice(Some(Notice::CheckpointRecovered)),
        "部分工作区存档不可读；已使用有效存档恢复，原始文件已保留"
    );
    assert_eq!(
        Locale::EnUs.shell_notice(Some(Notice::CheckpointEmpty)),
        "Workspace snapshots unreadable; opened an empty workspace and retained originals"
    );
    assert_eq!(
        Locale::ZhCn.shell_notice(Some(Notice::CheckpointEmpty)),
        "工作区存档均不可读；已打开空工作区，原始文件已保留"
    );
}

#[cfg(unix)]
#[test]
fn native_tab_copy_matches_the_accepted_english_table() {
    let locale = Locale::EnUs;

    assert_eq!(locale.native_creating(), "Creating Tab…");
    assert_eq!(
        locale.native_create_not_created(),
        "Tab not created · previous Tab unchanged"
    );
    assert_eq!(locale.native_refreshing(), "Refreshing Tabs…");
    assert_eq!(
        locale.native_refresh_success(u64::MAX),
        format!("Tabs refreshed · revision {}", u64::MAX)
    );
    assert_eq!(
        locale.native_refresh_failure(41),
        "Tab refresh failed · showing revision 41"
    );
    assert_eq!(locale.native_local_busy(), "Client busy · no command sent");
    assert_eq!(locale.native_no_tab(9), "No Tab at position 9");
    assert_eq!(locale.native_connecting(10), "Tab 10 · Connecting…");
    assert_eq!(
        locale.native_waiting_for_frame(10),
        "Tab 10 · Connecting · waiting for full frame"
    );
    assert_eq!(
        locale.native_display_blocked(10),
        "Tab 10 · Connecting · display blocked"
    );
    assert_eq!(
        locale.native_ready_controlling(10),
        "Tab 10 · Ready · controlling"
    );
    assert_eq!(
        locale.native_ready_read_only(10),
        if cfg!(windows) {
            "Tab 10 · Ready · view only · Ctrl+B t to take control"
        } else {
            "Tab 10 · Ready · view only"
        }
    );
    assert_eq!(
        locale.native_committed_refresh_required(),
        "Created Tab committed · refresh required"
    );
    assert_eq!(
        locale.native_terminal_pending(10),
        "Tab 10 · terminal creation pending · no replay"
    );
    assert_eq!(
        locale.native_terminal_unavailable(10),
        "Tab 10 · terminal unavailable"
    );
    assert_eq!(
        locale.native_terminal_failed(10),
        "Tab 10 · terminal creation failed"
    );
    assert_eq!(
        locale.native_terminal_outcome_unknown(10),
        "Tab 10 · terminal result unknown · no replay"
    );
}

#[cfg(unix)]
#[test]
fn native_tab_copy_matches_the_accepted_chinese_table() {
    let locale = Locale::ZhCn;

    assert_eq!(locale.native_creating(), "正在创建 Tab…");
    assert_eq!(
        locale.native_create_not_created(),
        "未创建 Tab · 原 Tab 未更改"
    );
    assert_eq!(locale.native_refreshing(), "正在刷新 Tab…");
    assert_eq!(
        locale.native_refresh_success(u64::MAX),
        format!("Tab 已刷新 · 修订 {}", u64::MAX)
    );
    assert_eq!(
        locale.native_refresh_failure(41),
        "Tab 刷新失败 · 正在显示修订 41"
    );
    assert_eq!(locale.native_local_busy(), "Client 正忙 · 未发送命令");
    assert_eq!(locale.native_no_tab(9), "第 9 个 Tab 不存在");
    assert_eq!(locale.native_connecting(10), "Tab 10 · 正在连接…");
    assert_eq!(
        locale.native_waiting_for_frame(10),
        "Tab 10 · 正在连接 · 等待完整帧"
    );
    assert_eq!(
        locale.native_display_blocked(10),
        "Tab 10 · 正在连接 · 显示已阻塞"
    );
    assert_eq!(
        locale.native_ready_controlling(10),
        "Tab 10 · 已就绪 · 可控制"
    );
    assert_eq!(
        locale.native_ready_read_only(10),
        if cfg!(windows) {
            "Tab 10 · 已就绪 · 仅查看 · Ctrl+B t 接管"
        } else {
            "Tab 10 · 已就绪 · 仅查看"
        }
    );
    assert_eq!(
        locale.native_committed_refresh_required(),
        "已提交创建的 Tab · 需要刷新"
    );
    assert_eq!(
        locale.native_terminal_pending(10),
        "Tab 10 · 终端创建等待中 · 不会重放"
    );
    assert_eq!(
        locale.native_terminal_unavailable(10),
        "Tab 10 · 终端不可用"
    );
    assert_eq!(locale.native_terminal_failed(10), "Tab 10 · 终端创建失败");
    assert_eq!(
        locale.native_terminal_outcome_unknown(10),
        "Tab 10 · 终端结果未知 · 不会重放"
    );
}

#[cfg(unix)]
#[test]
fn native_notice_and_refresh_enums_select_the_exact_copy() {
    use crate::terminal_session::{NativeRefreshStatus, NativeSessionNotice};

    assert_eq!(
        Locale::EnUs.native_notice(NativeSessionNotice::CloseRequestNotApplied),
        "Close request was not applied"
    );
    assert_eq!(
        Locale::ZhCn.native_notice(NativeSessionNotice::CloseRequestNotApplied),
        "关闭请求未执行"
    );

    assert_eq!(
        Locale::EnUs.native_notice(NativeSessionNotice::MoveNotMoved),
        "Tab was not moved"
    );
    assert_eq!(
        Locale::ZhCn.native_notice(NativeSessionNotice::MoveNotMoved),
        "标签未移动"
    );

    assert_eq!(
        Locale::EnUs.native_notice(NativeSessionNotice::LocalBusy),
        "Client busy · no command sent"
    );
    assert_eq!(
        Locale::ZhCn.native_notice(NativeSessionNotice::NoTabAtPosition(9)),
        "第 9 个 Tab 不存在"
    );
    assert_eq!(
        Locale::EnUs.native_notice(NativeSessionNotice::CreateNotCreated),
        "Tab not created · previous Tab unchanged"
    );
    assert_eq!(
        Locale::EnUs.native_notice(NativeSessionNotice::ConfirmationCancelled),
        "Close confirmation cancelled"
    );
    assert_eq!(
        Locale::ZhCn.native_notice(NativeSessionNotice::ConfirmationCancelled),
        "关闭确认已取消"
    );
    assert_eq!(
        Locale::EnUs.native_notice(NativeSessionNotice::DraftCancelled),
        "Draft cancelled"
    );
    assert_eq!(
        Locale::ZhCn.native_notice(NativeSessionNotice::DraftCancelled),
        "草稿已取消"
    );
    assert_eq!(
        Locale::EnUs.native_refresh_status(NativeRefreshStatus::Refreshing),
        "Refreshing Tabs…"
    );
    assert_eq!(
        Locale::ZhCn.native_refresh_status(NativeRefreshStatus::Success(42)),
        "Tab 已刷新 · 修订 42"
    );
    assert_eq!(
        Locale::EnUs.native_refresh_status(NativeRefreshStatus::Failure(41)),
        "Tab refresh failed · showing revision 41"
    );
}

#[cfg(unix)]
#[test]
fn native_empty_without_attachment_retains_the_manual_create_instruction() {
    use crate::terminal_session::{NativeDisplayPhase, NativeProjectionStatus};
    for (locale, expected) in [
        (Locale::EnUs, "No terminal tabs · Ctrl+B c to create a Tab"),
        (Locale::ZhCn, "没有终端标签 · 按 Ctrl+B 后按 c 创建 Tab"),
    ] {
        assert_eq!(
            locale.native_phase_status_values(
                NativeDisplayPhase::Empty,
                None,
                None,
                None,
                NativeProjectionStatus::Empty,
                None
            ),
            expected
        );
    }
}

#[cfg(unix)]
#[test]
fn settled_setup_notices_never_hide_later_failures_or_retirement() {
    use crate::terminal_session::{NativeSessionNotice, NativeSetupStatus};
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for setup in [
            NativeSetupStatus::ProjectCreated,
            NativeSetupStatus::WorkspaceCreated,
            NativeSetupStatus::DirectoryAccepted,
            NativeSetupStatus::NotApplied,
        ] {
            for notice in [
                NativeSessionNotice::CreateNotCreated,
                NativeSessionNotice::RenameNotRenamed,
                NativeSessionNotice::LocalBusy,
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
    use crate::terminal_session::{NativeSessionNotice, NativeSetupStatus};
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for setup in [
            NativeSetupStatus::CommittedNeedRefresh,
            NativeSetupStatus::OutcomeUnknown,
        ] {
            let notice = NativeSessionNotice::LocalBusy;
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
                retired.contains(if setup == NativeSetupStatus::CommittedNeedRefresh {
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
    use crate::terminal_session::NativeSetupStatus;
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let setup = NativeSetupStatus::WorkspaceCreated;
        let eligible =
            locale.native_status_with_setup("phase".to_owned(), false, None, Some(setup), true);
        assert!(eligible.contains("Ctrl+B e"));
        let configured_or_nonempty =
            locale.native_status_with_setup("phase".to_owned(), false, None, Some(setup), false);
        assert_eq!(configured_or_nonempty, locale.setup_status(setup));
        assert!(!configured_or_nonempty.contains("Ctrl+B e"));
        let retired =
            locale.native_status_with_setup("retired".to_owned(), true, None, Some(setup), true);
        assert_eq!(retired, "retired");
    }
}

#[cfg(unix)]
#[test]
fn native_prebootstrap_starting_remains_connecting_without_a_selected_tab() {
    use crate::terminal_session::{NativeProjectionStatus, TerminalStatus};

    for locale in [Locale::EnUs, Locale::ZhCn] {
        assert_eq!(
            locale.native_starting_status(None, NativeProjectionStatus::Empty),
            locale.terminal_status(TerminalStatus::Connecting)
        );
        assert_eq!(
            locale.native_switching_status(None, NativeProjectionStatus::Empty),
            locale.terminal_status(TerminalStatus::Unavailable)
        );
    }
    assert_eq!(
        Locale::EnUs.native_starting_status(Some(1), NativeProjectionStatus::WaitingForFullFrame),
        "Tab 1 · Connecting · waiting for full frame"
    );
}

#[cfg(unix)]
#[test]
fn native_prebootstrap_retirement_overrides_the_synthetic_starting_phase() {
    use crate::terminal_session::{
        NativeDisplayPhase, NativeDisplayRetirement, NativeProjectionStatus,
    };

    for (locale, unavailable, outcome_unknown) in [
        (
            Locale::EnUs,
            "Terminal disconnected · Ctrl+B then q to exit",
            "Terminal operation result unknown · no replay · Ctrl+B then q to exit",
        ),
        (
            Locale::ZhCn,
            "终端已断连 · 按 Ctrl+B 后按 q 退出",
            "终端操作结果未知 · 不会重放 · 按 Ctrl+B 后按 q 退出",
        ),
    ] {
        let status = |retirement| {
            locale.native_phase_status_values(
                NativeDisplayPhase::Starting,
                None,
                None,
                None,
                NativeProjectionStatus::Empty,
                Some(retirement),
            )
        };
        assert_eq!(status(NativeDisplayRetirement::Unavailable), unavailable);
        assert_eq!(
            status(NativeDisplayRetirement::OutcomeUnknown),
            outcome_unknown
        );
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
        version: 1,
        io_diagnostic: None,
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
        version: 1,
        io_diagnostic: None,
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
        version: 1,
        io_diagnostic: None,
        usage: None,
    };
    assert_eq!(
        Locale::EnUs.cli_failure(&failure),
        "No tab matches \"x\".\n"
    );
}

/// A cohort-check failure retains the observed `io::ErrorKind` — and the OS
/// error number when one exists — for the localized stderr diagnostic. The
/// machine document, stage detail, and exit code are the unchanged
/// `check-failed`/`cohort` contract; the diagnostic never serializes.
#[cfg(target_os = "linux")]
#[test]
fn cli_cohort_failure_reports_the_observed_io_kind() {
    use crate::cli::{Failure, FailureKind, failure_output, stage_details};
    use crate::invocation::OutputFormat;
    use serde_json::json;

    let cohort_failure = |error: &std::io::Error| {
        Failure::new(FailureKind::CheckFailed)
            .details(stage_details("cohort"))
            .io_diagnostic(error)
    };

    // A real OS error keeps its kind and its number in both locales.
    let failure = cohort_failure(&std::io::Error::from_raw_os_error(110));
    assert_eq!(
        Locale::EnUs.cli_failure(&failure),
        "The native development cohort could not be validated \
         (I/O kind: TimedOut, OS error: 110).\n"
    );
    assert_eq!(
        Locale::ZhCn.cli_failure(&failure),
        "无法验证原生开发 cohort（I/O 类型：TimedOut，OS 错误码：110）。\n"
    );

    // A permission error reports its own kind — never a missing/corrupt
    // guess. Exact equality also proves the bound: no path and none of the
    // OS message's free-form text ("Permission denied (os error 13)") leaks.
    let failure = cohort_failure(&std::io::Error::from_raw_os_error(13));
    assert_eq!(
        Locale::EnUs.cli_failure(&failure),
        "The native development cohort could not be validated \
         (I/O kind: PermissionDenied, OS error: 13).\n"
    );

    // A synthetic error without an OS number reports the observed kind only;
    // the arbitrary payload string never reaches stderr.
    let failure = cohort_failure(&std::io::Error::other(
        "arbitrary free text that must not leak",
    ));
    assert_eq!(
        Locale::EnUs.cli_failure(&failure),
        "The native development cohort could not be validated (I/O kind: Other).\n"
    );
    assert_eq!(
        Locale::ZhCn.cli_failure(&failure),
        "无法验证原生开发 cohort（I/O 类型：Other）。\n"
    );

    // A cohort failure that never carried the diagnostic (an envelope built
    // without the private data) still answers with the plain message.
    let failure = Failure::new(FailureKind::CheckFailed).details(stage_details("cohort"));
    assert_eq!(
        Locale::EnUs.cli_failure(&failure),
        "The native development cohort could not be validated.\n"
    );
    assert_eq!(
        Locale::ZhCn.cli_failure(&failure),
        "无法验证原生开发 cohort。\n"
    );

    // The emitted document and exit stay exactly the existing contract —
    // no io detail ever reaches stdout even when the diagnostic is present.
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
