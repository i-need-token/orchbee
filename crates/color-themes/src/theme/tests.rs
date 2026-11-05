use super::{Theme, ThemeFileError};

const FILE: &str = include_str!("../../themes/orchbee-dark.toml");

/// A hand-imported theme with a typo must fail to load rather than ship silently derived or
/// default colors.
#[test]
fn malformed_theme_files_are_rejected_instead_of_guessed() {
    assert!(Theme::parse(FILE).is_ok());
    for (from, to, invalid_field) in [
        // A misspelled optional key would otherwise leave the selection to be derived.
        ("selection_background =", "selection_backgroud =", None),
        ("bright_white = \"#e5e5e5\"", "", None),
        (
            "cursor = \"#57bef8\"",
            "cursor = \"#57bef\"",
            Some("canvas.cursor"),
        ),
        (
            "cursor = \"#57bef8\"",
            "cursor = { value = \"#57bef8\", original = \"#57bef8\" }",
            Some("canvas.cursor"),
        ),
        ("id = \"orchbee-dark\"", "id = \"Orchbee Dark\"", Some("id")),
        (
            "url = \"https://github.com/microsoft/vscode\"",
            "url = \"github.com/microsoft/vscode\"",
            Some("source.url"),
        ),
    ] {
        assert!(FILE.contains(from), "fixture lacks {from:?}");
        let result = Theme::parse(&FILE.replacen(from, to, 1));
        match (invalid_field, result) {
            (None, Err(ThemeFileError::Syntax(_))) => {}
            (Some(expected), Err(ThemeFileError::Invalid(field))) if field == expected => {}
            (_, other) => panic!("{from:?} -> {to:?} gave {other:?}"),
        }
    }
}
