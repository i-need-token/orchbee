use std::{collections::HashMap, ffi::OsString};

use crate::client_view::{BackendAction, Notice};

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

    use crate::{
        action_menu::{MenuAction, MenuUnavailable, TabMenuScope},
        workbench_client::CloseConfirmationKind,
    };

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
    assert_eq!(
        locale.menu_group_path(TabMenuScope::Group(count)),
        "分组（2 个标签页）"
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
    assert_eq!(locale.rename_title(), "重命名标签页");
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
    let english = crate::tui_shortcut_help::entries(
        crate::localization::Locale::EnUs,
        &inventory,
        true,
        KeyLabelGlyphs::Unicode,
    );
    let chinese = crate::tui_shortcut_help::entries(
        crate::localization::Locale::ZhCn,
        &inventory,
        true,
        KeyLabelGlyphs::Unicode,
    );
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
