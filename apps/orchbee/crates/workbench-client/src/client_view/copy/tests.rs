use super::*;
#[test]
fn every_discovered_binding_is_unique_exact_and_repeatable() {
    let mut seen = Vec::new();
    for group in GROUPS {
        for key in group.bindings {
            let spec = key.spec();
            let binding = spec.binding;
            let expected = spec.normal.expect("normal group contains a normal action");
            assert!(
                !seen.contains(&binding),
                "duplicate copy binding: {binding:?}"
            );
            seen.push(binding);
            let mut event = KeyEvent::press(binding.code, binding.mods);
            assert_eq!(decide(event), Some(expected));
            event.kind = KeyKind::Repeat;
            assert_eq!(decide(event), Some(expected));
            event.kind = KeyKind::Release;
            assert_eq!(decide(event), None);
            event.kind = KeyKind::Press;
            event.mods = event.mods.with_super_();
            assert_eq!(decide(event), None);
        }
    }
    let help = help_rows();
    assert_eq!(help.len(), GROUPS.len() + 1);
    let repeat = help
        .iter()
        .position(|(_, desc)| *desc == PrefixDesc::CopyRepeatSearch)
        .unwrap();
    let reverse = &help[repeat + 1];
    assert_eq!(reverse.0, "Ctrl+r");
    assert_eq!(reverse.1, PrefixDesc::CopyReverseSearch);
}

#[test]
fn search_commands_and_footer_hints_share_their_exact_binding_specs() {
    let expected = [
        (LocalBinding::SearchBack, CopySearchCommand::Back),
        (LocalBinding::CopyEnter, CopySearchCommand::Submit),
        (LocalBinding::SearchReverse, CopySearchCommand::Reverse),
    ];
    let mut seen = Vec::new();
    for (key, command) in expected {
        let binding = key.spec().binding;
        assert!(!seen.contains(&binding));
        seen.push(binding);
        let mut event = KeyEvent::press(binding.code, binding.mods);
        assert_eq!(search_command(event), Some(command));
        event.kind = KeyKind::Repeat;
        assert_eq!(search_command(event), Some(command));
        event.kind = KeyKind::Release;
        assert_eq!(search_command(event), None);
        event.kind = KeyKind::Press;
        event.mods = event.mods.with_super_();
        assert_eq!(search_command(event), None);
    }

    assert_eq!(footer_bindings(false)[0].label(), CopyHintLabel::Exit);
    assert_eq!(footer_bindings(true)[0].label(), CopyHintLabel::Exit);
    assert!(footer_bindings(true).iter().any(|hint| {
        hint.label() == CopyHintLabel::Reverse
            && hint
                .bindings()
                .any(|binding| binding == LocalBinding::SearchReverse.spec().binding)
    }));
}
#[test]
fn capital_motions_and_search_do_not_alias_lowercase_or_help() {
    assert_eq!(
        decide(KeyEvent::press(KeyCode::Char('W'), Modifiers::NONE)),
        Some(CopyModeKey::Motion(CopyMotion::NextBigWordStart))
    );
    assert_eq!(
        decide(KeyEvent::press(KeyCode::Char('?'), Modifiers::NONE)),
        Some(CopyModeKey::Search(CopySearchDirection::Backward))
    );
    assert_eq!(
        decide(KeyEvent::press(KeyCode::Char('u'), Modifiers::NONE)),
        None
    );
}
