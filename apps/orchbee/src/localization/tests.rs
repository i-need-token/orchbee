use std::{collections::HashMap, ffi::OsString};

use crate::client_view::{BackendAction, Notice};

use super::{Locale, select_locale};

#[cfg(unix)]
mod location_labels;

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

    assert_eq!(english.shortcuts.len(), 21);
    assert_eq!(chinese.shortcuts.len(), english.shortcuts.len());
    assert_ne!(english.backend_status, chinese.backend_status);
    assert_ne!(english.terminal_unavailable, chinese.terminal_unavailable);

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
        "Ctrl+B 1 … 9",
        "Ctrl+B w",
    ] {
        assert!(
            english
                .shortcuts
                .iter()
                .any(|line| line.starts_with(shortcut))
        );
        assert!(
            chinese
                .shortcuts
                .iter()
                .any(|line| line.starts_with(shortcut))
        );
    }
    assert!(
        english
            .shortcuts
            .contains(&"Ctrl+B w       Toggle Workbench navigator")
    );
    assert!(
        chinese
            .shortcuts
            .contains(&"Ctrl+B w       切换 Workbench 导航器")
    );
    assert!(english.shortcuts.contains(
        &"Ctrl+B [ / ]   Select previous / next Workbench location (wraps; requires Site)"
    ));
    assert!(
        chinese
            .shortcuts
            .contains(&"Ctrl+B [ / ]   选择上一个 / 下一个 Workbench 位置（首尾循环；需要 Site）")
    );
}

#[test]
fn panel_shortcut_rows_match_their_client_actions_in_both_locales() {
    use crate::client_view::{
        ClientAction, ClientKey, ClientView, PanelDirection, SessionActionDisposition,
    };

    let english = Locale::EnUs.shell();
    let chinese = Locale::ZhCn.shell();
    for (en, zh, bindings) in [
        (
            "Ctrl+B h/j/k/l Move between Panels (requires Site)",
            "Ctrl+B h/j/k/l 在 Panel 之间移动（需要 Site）",
            &[
                ('h', ClientAction::MovePanelFocus(PanelDirection::Left)),
                ('j', ClientAction::MovePanelFocus(PanelDirection::Down)),
                ('k', ClientAction::MovePanelFocus(PanelDirection::Up)),
                ('l', ClientAction::MovePanelFocus(PanelDirection::Right)),
            ][..],
        ),
        (
            "Ctrl+B - _ \\ | Split focused Panel below/above/left/right (requires Site)",
            "Ctrl+B - _ \\ | 将焦点 Panel 向下/上/左/右分屏（需要 Site）",
            &[
                ('-', ClientAction::SplitPanel(PanelDirection::Down)),
                ('_', ClientAction::SplitPanel(PanelDirection::Up)),
                ('\\', ClientAction::SplitPanel(PanelDirection::Left)),
                ('|', ClientAction::SplitPanel(PanelDirection::Right)),
            ][..],
        ),
        (
            "Ctrl+B X       Close focused Panel (requires Site)",
            "Ctrl+B X       关闭焦点 Panel（需要 Site）",
            &[('X', ClientAction::ClosePanel)][..],
        ),
        (
            "Ctrl+B H/J/K/L Move focused Panel to that side (requires Site)",
            "Ctrl+B H/J/K/L 将焦点 Panel 移向该侧（需要 Site）",
            &[
                ('H', ClientAction::MovePanel(PanelDirection::Left)),
                ('J', ClientAction::MovePanel(PanelDirection::Down)),
                ('K', ClientAction::MovePanel(PanelDirection::Up)),
                ('L', ClientAction::MovePanel(PanelDirection::Right)),
            ][..],
        ),
    ] {
        assert!(english.shortcuts.contains(&en), "en-US row: {en}");
        assert!(chinese.shortcuts.contains(&zh), "zh-CN row: {zh}");
        for (key, expected) in bindings {
            let mut view = ClientView::new(120);
            let prefix = view.decide(ClientKey::ControlB);
            view.commit(prefix, SessionActionDisposition::KeepNavigator);
            let transition = view.decide(ClientKey::Plain(*key));
            assert_eq!(transition.action(), Some(*expected), "Ctrl+B {key}");
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
        "Tab 10 · Ready · view only"
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
        "Tab 10 · 已就绪 · 仅查看"
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
