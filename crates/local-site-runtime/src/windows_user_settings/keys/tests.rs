use super::*;

fn read(body: &str) -> Result<WindowsKeySettings, WindowsSettingsError> {
    let value: toml::Value = toml::from_str(&format!("[keys]\n{body}")).unwrap();
    WindowsKeySettings::parse(&value["keys"])
}

#[test]
fn key_configuration_accepts_alternatives_disabling_ranges_and_aliases() {
    let keys = read("prefix='ctrl+a'\nhelp=['prefix+?', 'f1']\nnew_tab=[]\nswitch_tab='prefix+alt+1..9'\nfullscreen='alt+z'").unwrap();
    assert_eq!(keys.prefix, parse_chord("control+a").unwrap());
    assert_eq!(
        keys.overrides
            .iter()
            .find(|item| item.action == "help")
            .unwrap()
            .triggers
            .len(),
        2
    );
    assert!(
        keys.overrides
            .iter()
            .find(|item| item.action == "new_tab")
            .unwrap()
            .triggers
            .is_empty()
    );
    assert_eq!(
        keys.overrides
            .iter()
            .find(|item| item.action == "switch_tab")
            .unwrap()
            .triggers
            .len(),
        9
    );
    assert!(keys.overrides.iter().any(|item| item.action == "zoom"));
}

#[test]
fn key_configuration_rejects_ambiguous_reserved_unsafe_and_unimplemented_values() {
    for (body, reason) in [
        (
            "help='prefix+Q'\nnew_tab='prefix+shift+q'",
            "duplicate-binding",
        ),
        ("help='f01'\nnew_tab='f1'", "duplicate-binding"),
        ("zoom='alt+z'\nfullscreen='f4'", "duplicate-binding"),
        (
            "prefix='ctrl+a'\nhelp='prefix+control+a'",
            "reserved-binding",
        ),
        ("help='ctrl+b'", "reserved-binding"),
        ("help='prefix+esc'", "reserved-binding"),
        ("help='ctrl+d'", "unsafe-binding"),
        ("help='enter'", "unsafe-binding"),
        ("help='Z'", "unsafe-binding"),
        ("help='tab'", "unsafe-binding"),
        ("help='backspace'", "unsafe-binding"),
        ("help='esc'", "unsafe-binding"),
        ("help='prefix+1..9'", "invalid-binding"),
        ("switch_tab='prefix+q'", "invalid-binding"),
        ("help='prefix+f25'", "invalid-binding"),
        ("help='prefix+'", "invalid-binding"),
        ("help='prefix+hyper+x'", "invalid-binding"),
        ("help=3", "wrong-type"),
        ("help=['f1', false]", "wrong-type"),
        ("command=[]", "unknown-key"),
        ("reload_config='prefix+R'", "unknown-key"),
    ] {
        assert_eq!(read(body).unwrap_err().reason, reason, "{body}");
    }
}

#[test]
fn key_configuration_is_bounded_and_preserves_complete_modifiers() {
    assert_eq!(
        parse_chord("shift+CTRL+a").unwrap(),
        parse_chord("ctrl+A").unwrap()
    );
    assert_ne!(
        parse_chord("ctrl+a").unwrap(),
        parse_chord("ctrl+shift+a").unwrap()
    );
    assert_eq!(
        parse_chord("return").unwrap(),
        parse_chord("ctrl+m").unwrap()
    );
    assert_eq!(
        parse_chord("ctrl+5").unwrap(),
        parse_chord("ctrl+]").unwrap()
    );
    assert!(read("help='alt+shift+a'").is_ok());
    let body = format!("help=[{}]", vec!["'f1'"; 17].join(","));
    assert_eq!(read(&body).unwrap_err().reason, "too-many-bindings");
    assert!(parse_chord(&"x".repeat(129)).is_err());
    assert!(parse_chord("ctrl+\nq").is_err());
}
