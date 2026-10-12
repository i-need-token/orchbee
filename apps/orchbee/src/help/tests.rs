use super::{Document, Row, Section, cells, render_at_width};

#[test]
fn wrapping_preserves_graphemes_and_content_in_both_help_languages() {
    let summary = "Create a workspace with Cafe\u{301}, 中文说明和 👩‍💻; preserve every documented option and value.";
    let document = Document {
        title: String::new(),
        summary: summary.into(),
        sections: Vec::new(),
        footer: Vec::new(),
    };
    for width in [40, 78, 120] {
        let text = render_at_width(&document, width);
        assert!(text.lines().all(|line| cells(line) <= width));
        assert!(text.contains("Cafe\u{301}"));
        assert!(text.contains("👩‍💻"));
        assert_eq!(
            text.chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>(),
            summary
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>()
        );
    }
}

#[test]
fn long_labels_take_their_own_line_and_descriptions_keep_their_column() {
    let document = Document {
        title: "Options".into(),
        summary: String::new(),
        sections: vec![Section::Rows {
            heading: "Choose".into(),
            rows: vec![
                Row {
                    label: "--label <name>".into(),
                    description:
                        "A short display name with a description that spans several terminal rows."
                            .into(),
                },
                Row {
                    label: "--after-revision <revision-number>".into(),
                    description: "Continue after the specified revision.".into(),
                },
            ],
        }],
        footer: Vec::new(),
    };
    let text = render_at_width(&document, 78);
    assert!(text.contains("  --after-revision <revision-number>\n    Continue"));
    assert!(text.lines().all(|line| cells(line) <= 78));
    let continuation = text
        .lines()
        .find(|line| line.contains("several terminal rows."))
        .unwrap();
    assert!(continuation.starts_with(&" ".repeat(28)));
}
