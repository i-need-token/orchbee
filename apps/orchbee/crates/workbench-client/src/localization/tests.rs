use std::{collections::HashMap, ffi::OsString};

use super::{Locale, select_locale};

#[test]
fn compact_terminal_end_copy_keeps_complete_results_within_twenty_cells_in_both_locales() {
    use crate::terminal_projection::TerminalEnd;
    use orchbee_terminal_protocol::ExitResult;
    for end in [
        TerminalEnd::Exited(ExitResult::ExitCode(0)),
        TerminalEnd::Exited(ExitResult::ExitCode(u32::MAX)),
        TerminalEnd::Exited(ExitResult::Signal(u32::MAX)),
        TerminalEnd::Exited(ExitResult::Unknown(0)),
        TerminalEnd::Failed,
    ] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let text = locale.terminal_end_compact(end);
            assert!(crate::terminal_text::display_width(&text) <= 20);
        }
    }
}

#[cfg(unix)]
mod location_labels;

#[cfg(windows)]
#[test]
fn tab_menu_copy_localizes_scope_and_preserves_close_and_rename_consequences() {
    use std::num::NonZeroUsize;

    use crate::action_menu::{MenuAction, TabMenuScope};

    let locale = Locale::ZhCn;
    let count = NonZeroUsize::new(2).unwrap();
    assert_eq!(MenuAction::PinTab.title(locale), "固定标签页");
    assert_eq!(MenuAction::CloseSelectedTabs.title(locale), "关闭标签页…");
    assert_eq!(
        MenuAction::CloseSelectedTabs.scoped_title(locale, TabMenuScope::Selected(count)),
        "关闭所选 2 个标签页…"
    );
    assert_eq!(
        MenuAction::CloseGroup.scoped_title(locale, TabMenuScope::Group(count)),
        "关闭分组（2 个标签页）…"
    );
    assert_eq!(
        locale.menu_group_path(TabMenuScope::Group(count)),
        "分组（2 个标签页）"
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
    assert_eq!(locale.rename_title(), "重命名标签页");
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
fn panel_shortcuts_dispatch_their_actions() {
    use crate::client_view::{
        ClientAction, ClientActionDisposition, ClientView, KeyCode, KeyEvent, KeyKind, Modifiers,
        PanelDirection, PrefixInput,
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
        (KeyCode::Char('X'), ClientAction::ClosePanel),
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
    ];
    for (code, expected) in cases {
        let mut view = ClientView::new(120);
        let prefix = view.decide(key(KeyCode::Char('b'), Modifiers::CTRL));
        view.commit(prefix, ClientActionDisposition::KeepNavigator);
        let transition = view.decide(key(code, Modifiers::NONE));
        assert_eq!(transition.action(), Some(expected), "Ctrl+B {code:?}");
    }
}

/// Each handshake refusal renders in both locales without internal terms, names both versions when
/// the Session stated its own, and leaves out a Session version it was not told.
#[test]
fn handshake_refusals_name_the_older_side_in_both_locales() {
    use crate::startup_status::{RefusedVersions, StartupFailure};
    let versions = |session: Option<&str>| {
        Box::new(RefusedVersions {
            client: "1.2.0".to_owned(),
            session: session.map(str::to_owned),
        })
    };
    for failure in [
        StartupFailure::ClientTooOld(versions(Some("1.4.0"))),
        StartupFailure::SessionTooOld(versions(Some("1.0.0"))),
        StartupFailure::SessionTooOld(versions(None)),
        StartupFailure::OtherEnvironment,
        StartupFailure::DevelopmentBuildMismatch,
    ] {
        let texts = [Locale::EnUs, Locale::ZhCn].map(|locale| locale.startup_failure(&failure));
        assert_ne!(texts[0], texts[1]);
        for text in &texts {
            assert!(text.ends_with('\n'), "{text}");
            assert!(
                !text.contains("cohort") && !text.contains("generation"),
                "{text}"
            );
            if let StartupFailure::ClientTooOld(known) | StartupFailure::SessionTooOld(known) =
                &failure
            {
                if let Some(session) = &known.session {
                    assert!(text.contains(&known.client), "{text}");
                    assert!(text.contains(session.as_str()), "{text}");
                }
                assert!(!text.contains("()") && !text.contains("（）"), "{text}");
            }
        }
    }
}

/// A notice is built without the binding owner, so it names a key through a slot. The rule: what
/// the status line draws carries the effective label in the one label spelling, never the slot.
#[test]
fn a_start_notice_names_the_picker_key_by_its_effective_label() {
    use super::{AgentStartStatus, WorkbenchClientNotice};
    use crate::terminal_events::keys::tests::host_label;
    let notice = WorkbenchClientNotice::AgentStart(AgentStartStatus::NotFound("Codex"));
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let keys = crate::client_view::ClientView::new(80)
            .bindings()
            .shell_bindings(locale.shell().binding_unbound);
        let mut forms = locale.native_notice_forms(notice, None);
        for form in &mut forms {
            keys.fill_notice(form);
            assert!(!form.contains(super::shell::START_AGENT_KEY_SLOT), "{form}");
        }
        assert!(
            forms[0].contains(&host_label("Ctrl+b Shift+G")),
            "{forms:?}"
        );
    }
}
