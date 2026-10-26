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

#[test]
fn default_config_action_rows_match_resolved_bindings_after_round_trip() {
    use std::fmt::Write as _;

    let registered = WindowsKeySettings::registered_actions();
    let defaults = Bindings::default();
    let generated = Bindings::default_windows_key_action_bindings();
    let rendered_actions = generated
        .iter()
        .map(|entry| entry.action)
        .collect::<Vec<_>>();
    assert_eq!(rendered_actions.as_slice(), registered);
    for (&name, rendered) in registered.iter().zip(&generated) {
        let (action, _, _) = action(name);
        let expected = defaults
            .entries
            .iter()
            .filter(|entry| entry.enabled && entry.member.action == action)
            .map(configuration_trigger)
            .collect::<Vec<_>>();
        assert_eq!(rendered.bindings, expected, "default for {name}");
    }

    let mut profile = String::from("[keys]\nprefix = 'ctrl+b'\n");
    for entry in &generated {
        let value = if entry.bindings.is_empty() {
            "[]".to_owned()
        } else {
            let values = entry
                .bindings
                .iter()
                .map(|binding| toml_edit::Value::from(binding.as_str()).to_string())
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{values}]")
        };
        writeln!(profile, "{} = {value}", entry.action).unwrap();
    }
    let settings = WindowsKeySettings::from_profile(&profile).unwrap();
    let round_trip = Bindings::configured(&settings);

    let resolved = |bindings: &Bindings| {
        let mut resolved = Vec::new();
        for &name in registered {
            let (action, _, _) = action(name);
            for entry in bindings
                .entries
                .iter()
                .filter(|entry| entry.enabled && entry.member.action == action)
            {
                let triggers = match entry.member.keys {
                    KeySet::One(_) => vec![configuration_trigger(entry)],
                    KeySet::CharRange(first, last) => (first..=last)
                        .map(|key| {
                            let expanded = Entry {
                                member: PrefixMember {
                                    keys: KeySet::One(Binding::new(
                                        KeyCode::Char(key),
                                        Modifiers::NONE,
                                    )),
                                    ..entry.member.clone()
                                },
                                ..entry.clone()
                            };
                            configuration_trigger(&expanded)
                        })
                        .collect(),
                };
                resolved.extend(
                    triggers
                        .into_iter()
                        .map(|trigger| format!("{name}={trigger}")),
                );
            }
        }
        resolved.sort();
        resolved
    };
    assert_eq!(resolved(&defaults), resolved(&round_trip));

    let switch_tab = generated
        .iter()
        .find(|entry| entry.action == "switch_tab")
        .unwrap();
    assert_eq!(switch_tab.bindings, vec!["prefix+1..9".to_owned()]);
    let focus_agent = generated
        .iter()
        .find(|entry| entry.action == "focus_agent")
        .unwrap();
    assert!(focus_agent.bindings.is_empty());
}
