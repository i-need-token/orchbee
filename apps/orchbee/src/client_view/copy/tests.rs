use super::*;
#[test]
fn every_discovered_binding_is_unique_exact_and_repeatable() {
    let mut seen = Vec::new();
    for group in GROUPS {
        for &(binding, expected) in group.bindings {
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
    assert_eq!(help_rows().len(), GROUPS.len());
}
#[test]
fn capital_motions_and_search_do_not_alias_lowercase_or_help() {
    assert_eq!(
        decide(KeyEvent::press(KeyCode::Char('W'), Modifiers::NONE)),
        Some(K::Motion(M::NextBigWordStart))
    );
    assert_eq!(
        decide(KeyEvent::press(KeyCode::Char('?'), Modifiers::NONE)),
        Some(K::Search(S::Backward))
    );
    assert_eq!(
        decide(KeyEvent::press(KeyCode::Char('u'), Modifiers::NONE)),
        None
    );
}
