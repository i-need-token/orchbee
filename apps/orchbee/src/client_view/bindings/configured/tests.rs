use super::*;
use crate::client_view::KeyLabelGlyphs;
use orchbee_local_site_runtime::{
    WindowsKeyChord, WindowsKeyOverride, WindowsKeySettings, WindowsKeyTrigger,
};

fn chord(key: &str, ctrl: bool) -> WindowsKeyChord {
    WindowsKeyChord {
        key: key.into(),
        ctrl,
        alt: false,
        shift: false,
        super_key: false,
    }
}

#[test]
fn effective_action_labels_apply_glyph_fallback_to_all_complete_overridden_chords() {
    let settings = WindowsKeySettings {
        commands: Vec::new(),
        prefix: chord("a", true),
        overrides: vec![WindowsKeyOverride {
            action: "menu",
            triggers: vec![
                WindowsKeyTrigger {
                    prefix: true,
                    chord: chord("left", false),
                },
                WindowsKeyTrigger {
                    prefix: false,
                    chord: chord("right", true),
                },
            ],
        }],
    };
    let bindings = Bindings::configured(&settings);
    let action = PrefixAction::Client(ClientAction::OpenActionMenu);

    assert_eq!(
        bindings.action_labels_with_glyphs(action.clone(), KeyLabelGlyphs::Ascii),
        ["Ctrl+a Left", "Ctrl+Right"]
    );
    assert_eq!(bindings.action_labels(action), ["Ctrl+a ←", "Ctrl+→"]);
}
