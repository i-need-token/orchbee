use super::*;

fn press(code: KeyCode) -> KeyEvent {
    KeyEvent::press(code, Modifiers::NONE)
}

fn press_mods(code: KeyCode, mods: Modifiers) -> KeyEvent {
    KeyEvent::press(code, mods)
}

#[test]
fn every_existing_prefix_key_resolves_to_its_action() {
    let cases = [
        ('a', ClientAction::RequestWorkspaceCreate),
        ('e', ClientAction::RequestWorkspaceDirectory),
        ('r', ClientAction::RequestWorkspaceRename),
        ('x', ClientAction::RequestCloseTab),
        (',', ClientAction::RequestRenameTab),
        ('c', ClientAction::CreateTab),
        ('<', ClientAction::MovePreviousTab),
        ('>', ClientAction::MoveNextTab),
        ('n', ClientAction::SelectNextTab),
        ('p', ClientAction::SelectPreviousTab),
        ('[', ClientAction::SelectPreviousContext),
        (']', ClientAction::SelectNextContext),
    ];
    for (c, action) in cases {
        let member = match_prefix(press(KeyCode::Char(c))).unwrap_or_else(|| panic!("{c}"));
        assert_eq!(member.action, PrefixAction::Client(action));
    }
    let member = match_prefix(press_mods(KeyCode::Char('r'), Modifiers::NONE.with_shift()))
        .expect("shift+r");
    assert_eq!(
        member.action,
        PrefixAction::Client(ClientAction::RequestProjectRename)
    );
    let member = match_prefix(press(KeyCode::Char('R'))).expect("uppercase R");
    assert_eq!(
        member.action,
        PrefixAction::Client(ClientAction::RequestProjectRename)
    );
}
#[cfg(windows)]
#[test]
fn worktree_prefixes_resolve_through_the_same_registry_as_help() {
    for (key, action, description) in [
        (
            'W',
            ClientAction::RequestWorktreeCreate,
            PrefixDesc::WorktreeCreate,
        ),
        (
            'O',
            ClientAction::RequestWorktreeOpen,
            PrefixDesc::WorktreeOpen,
        ),
    ] {
        let member = match_prefix(press(KeyCode::Char(key))).unwrap();
        assert_eq!(member.action, PrefixAction::Client(action));
        assert!(
            PREFIX_GROUPS
                .iter()
                .any(|group| group.desc == description && group.members.contains(member))
        );
    }
}

#[test]
fn digit_range_resolves_as_one_member() {
    for d in '1'..='9' {
        let member = match_prefix(press(KeyCode::Char(d))).unwrap_or_else(|| panic!("{d}"));
        assert_eq!(member.action, PrefixAction::SelectTabDigit);
    }
    assert!(match_prefix(press(KeyCode::Char('0'))).is_none());
}

#[cfg(windows)]
#[test]
fn sidebar_focus_is_an_explicit_prefix_action_distinct_from_visibility() {
    assert_eq!(
        match_prefix(press(KeyCode::Tab)).unwrap().action,
        PrefixAction::Client(ClientAction::FocusBusinessSidebar)
    );
    assert_eq!(
        match_prefix(press(KeyCode::Char('B'))).unwrap().action,
        PrefixAction::Client(ClientAction::FocusBusinessSidebar)
    );
    assert_eq!(
        match_prefix(press(KeyCode::Char('b'))).unwrap().action,
        PrefixAction::SidebarToggle
    );
    assert!(match_prefix(press_mods(KeyCode::Char('b'), Modifiers::CTRL.with_shift())).is_none());
}

#[test]
fn escape_dismisses_and_literal_prefix_forwards() {
    let esc = match_prefix(press(KeyCode::Esc)).expect("esc");
    assert_eq!(
        esc.action,
        PrefixAction::Client(ClientAction::DismissPrefix)
    );
    let literal = match_prefix(press_mods(KeyCode::Char('b'), Modifiers::CTRL)).expect("ctrl+b");
    assert_eq!(
        literal.action,
        PrefixAction::Client(ClientAction::ForwardLiteralPrefix)
    );
    assert_eq!(literal.shell, ShellCommand::ForwardTerminal);
}

#[test]
fn modifier_supersets_match_nothing() {
    assert!(match_prefix(press_mods(KeyCode::Char('x'), Modifiers::CTRL)).is_none());
    assert!(
        match_prefix(press_mods(
            KeyCode::Char('x'),
            Modifiers::NONE.with_shift().with_alt()
        ))
        .is_none()
    );
    // Shift+X is bound (ClosePanel); Ctrl+Shift+X must not reach it.
    let member = match_prefix(press_mods(KeyCode::Char('x'), Modifiers::NONE.with_shift()))
        .expect("shift+x");
    assert_eq!(
        member.action,
        PrefixAction::Client(ClientAction::ClosePanel)
    );
    assert!(
        match_prefix(press_mods(
            KeyCode::Char('x'),
            Modifiers::NONE.with_shift().with_ctrl()
        ))
        .is_none()
    );
    // Alt+a is not plain a.
    assert!(match_prefix(press_mods(KeyCode::Char('a'), Modifiers::NONE.with_alt())).is_none());
}

#[test]
fn shortcut_discovery_uses_the_resolved_owner_keys() {
    let bindings = Bindings::default();
    let entries = bindings.discovery_entries(crate::client_view::KeyLabelGlyphs::Unicode);
    for (action, suffix) in [
        (ClientAction::MovePreviousTab, " <"),
        (ClientAction::MoveNextTab, " >"),
    ] {
        let entry = entries
            .iter()
            .find(|entry| entry.action == PrefixAction::Client(action.clone()))
            .expect("tab movement remains discoverable");
        assert!(entry.key.ends_with(suffix), "{}", entry.key);
    }
    let digits = entries
        .iter()
        .find(|entry| entry.action == PrefixAction::SelectTabDigit)
        .expect("ordered Tab selection remains discoverable");
    assert!(digits.key.ends_with(" 1..9"), "{}", digits.key);
}

#[test]
fn resolved_help_inventory_contains_only_projected_actions() {
    let bindings = Bindings::default();
    let entries = bindings.discovery_entries(crate::client_view::KeyLabelGlyphs::Unicode);
    assert!(!entries.is_empty());
    for entry in entries {
        assert!(
            crate::action_projection::binding_metadata(&entry.action)
                .is_some_and(|metadata| metadata.same_discovery_entry(&entry.metadata)),
            "missing resolved projection for {:?}",
            entry.action
        );
        assert!(!entry.key.is_empty());
    }
}
