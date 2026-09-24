use orchbee_workbench_protocol::{PanelCustomLabel, PanelLabelUpdate, Presentation};
use serde_json::json;

#[test]
fn updates_normalize_bounded_unicode_whitespace_and_clear_without_losing_bom() {
    for value in [None, Some(""), Some(" \t\r\n\u{85}\u{a0}\u{3000}")] {
        let update = PanelLabelUpdate::parse(value).unwrap();
        assert_eq!(update.custom_label(), None);
        assert_eq!(serde_json::to_value(&update).unwrap(), json!(null));
    }
    for (input, expected) in [
        ("  工作 shell \n", "工作 shell"),
        ("\u{85}x\u{a0}", "x"),
        (" \u{feff}x ", "\u{feff}x"),
        (" \u{200b}x ", "\u{200b}x"),
        ("a\nb", "a\nb"),
    ] {
        let update: PanelLabelUpdate = serde_json::from_value(json!(input)).unwrap();
        assert_eq!(update.custom_label().unwrap().as_str(), expected);
        assert_eq!(serde_json::to_value(&update).unwrap(), json!(expected));
    }
    assert!(PanelLabelUpdate::parse(Some(&"😀".repeat(512))).is_ok());
    assert!(PanelLabelUpdate::parse(Some(&"😀".repeat(513))).is_err());
    assert!(PanelLabelUpdate::parse(Some(&" ".repeat(513))).is_err());
    for invalid in [json!(0), json!(false), json!([]), json!({"label":"x"})] {
        assert!(serde_json::from_value::<PanelLabelUpdate>(invalid).is_err());
    }
}

#[test]
fn stored_overrides_have_one_absent_spelling_and_keep_the_creation_fallback() {
    let original = json!({
        "binding": null, "label": "creation fallback",
        "presentationId": "11111111111111111111111111111111"
    });
    let absent: Presentation = serde_json::from_value(original.clone()).unwrap();
    assert_eq!(absent.custom_label, None);
    assert_eq!(absent.display_label(), "creation fallback");
    assert_eq!(serde_json::to_value(&absent).unwrap(), original);
    let mut renamed = original.clone();
    renamed["customLabel"] = json!("user title");
    let stored: Presentation = serde_json::from_value(renamed.clone()).unwrap();
    assert_eq!(stored.label, "creation fallback");
    assert_eq!(stored.display_label(), "user title");
    assert_eq!(stored.custom_label.unwrap().as_str(), "user title");
    for invalid in [
        json!(null),
        json!(""),
        json!(" x"),
        json!("x\u{85}"),
        json!("x\n"),
        json!("x\r"),
        json!(0),
    ] {
        let mut document = original.clone();
        document["customLabel"] = invalid;
        assert!(serde_json::from_value::<Presentation>(document).is_err());
    }
    assert!(PanelCustomLabel::parse("a\nb").is_ok());
    assert!(PanelCustomLabel::parse("😀").is_ok());
}
