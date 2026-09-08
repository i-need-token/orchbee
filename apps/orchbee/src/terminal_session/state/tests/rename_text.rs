use super::super::rename_text::{RenameText, RenameTextLimit};
use orchbee_workbench_protocol::{ContractError, MAXIMUM_LABEL_LENGTH, WorkbenchLabel};

#[test]
fn seed_and_append_preserve_exact_schema_text() {
    let seed = WorkbenchLabel::parse("  e\u{301} 标签\t\r\n\u{1b}\u{85} ").unwrap();
    let mut draft = RenameText::from_label(&seed);
    assert_eq!(draft.as_str(), seed.as_str());
    for suffix in [" 🚀", "é", "\u{2028}", "  "] {
        let expected = format!("{}{suffix}", draft.as_str());
        assert_eq!(draft.append(suffix), Ok(()));
        assert_eq!(draft.as_str(), expected);
        assert_eq!(draft.label().unwrap().as_str(), expected);
    }
}

#[test]
fn empty_is_editable_but_only_nonempty_labels_are_returned() {
    let mut draft = RenameText::from_label(&WorkbenchLabel::parse("seed").unwrap());
    draft.clear();
    draft.clear();
    assert_eq!(draft.as_str(), "");
    assert_eq!(draft.label(), Err(ContractError::InvalidDocument));
    assert!(!draft.remove_last_scalar());
    assert_eq!(draft.append(""), Ok(()));
    assert_eq!(draft.label(), Err(ContractError::InvalidDocument));
    assert_eq!(draft.append("🚀"), Ok(()));
    assert_eq!(draft.label().unwrap().as_str(), "🚀");
    draft.clear();
    assert_eq!(draft.append("  \t"), Ok(()));
    assert_eq!(draft.label().unwrap().as_str(), "  \t");
}

#[test]
fn scalar_capacity_refuses_the_entire_append_without_truncation() {
    let seed = "🚀".repeat(MAXIMUM_LABEL_LENGTH - 1);
    let mut draft = RenameText::from_label(&WorkbenchLabel::parse(&seed).unwrap());
    assert_eq!(draft.append("雪é"), Err(RenameTextLimit));
    assert_eq!(draft.as_str(), seed);
    assert_eq!(draft.append("雪"), Ok(()));
    let full = format!("{seed}雪");
    assert_eq!(draft.as_str(), full);
    assert_eq!(draft.append(""), Ok(()));
    assert_eq!(draft.append("x"), Err(RenameTextLimit));
    assert_eq!(draft.as_str(), full);
    assert_eq!(draft.label().unwrap().as_str(), full);
    assert!(draft.remove_last_scalar());
    assert_eq!(draft.as_str(), seed);
    assert_eq!(draft.append("é"), Ok(()));
    assert_eq!(draft.label().unwrap().as_str(), format!("{seed}é"));
}

#[test]
fn removal_deletes_one_scalar_without_splitting_utf8_or_collapsing_graphemes() {
    let mut draft = RenameText::from_label(&WorkbenchLabel::parse("ée\u{301}🚀").unwrap());
    for expected in ["ée\u{301}", "ée", "é", ""] {
        assert!(draft.remove_last_scalar());
        assert_eq!(draft.as_str(), expected);
    }
    assert!(!draft.remove_last_scalar());
}

#[test]
fn returned_label_does_not_follow_later_draft_mutations() {
    let mut draft = RenameText::from_label(&WorkbenchLabel::parse(" original ").unwrap());
    let retained = draft.label().unwrap();
    assert_eq!(draft.append("changed"), Ok(()));
    draft.clear();
    assert_eq!(draft.append("replacement"), Ok(()));
    assert_eq!(retained.as_str(), " original ");
    assert_eq!(draft.label().unwrap().as_str(), "replacement");
}
