use super::*;
use crate::tui_theme::GlyphSet;

const AREA: Rect = Rect::new(0, 0, 64, 22);

fn text(lines: &[Line<'_>]) -> Vec<String> {
    lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect()
        })
        .collect()
}

#[test]
fn the_box_opens_wholly_below_else_above_from_the_pointer_column_and_never_covers_what_it_keeps() {
    let row = Rect::new(3, 4, 26, 1);
    assert_eq!(
        place(AREA, row, (9, 4), (20, 3), &[]),
        Some(Rect::new(9, 5, 20, 3))
    );
    // It shifts left to stay on screen.
    assert_eq!(
        place(AREA, Rect::new(40, 4, 24, 1), (62, 4), (20, 3), &[]),
        Some(Rect::new(44, 5, 20, 3))
    );
    // No room below the foot of the screen, or over the floating status: it opens above.
    let foot = Rect::new(25, 19, 3, 1);
    assert_eq!(
        place(AREA, foot, (26, 19), (20, 3), &[]),
        Some(Rect::new(26, 16, 20, 3))
    );
    let status = Rect::new(30, 7, 10, 1);
    assert_eq!(
        place(AREA, row, (9, 4), (24, 3), &[status]),
        Some(Rect::new(9, 1, 24, 3))
    );
    // A two-row target keeps both rows clear.
    assert_eq!(
        place(AREA, Rect::new(0, 4, 29, 2), (6, 4), (20, 3), &[]),
        Some(Rect::new(6, 6, 20, 3))
    );
    // With room in neither direction, or wider than the screen, it does not open.
    assert_eq!(
        place(
            Rect::new(0, 0, 64, 5),
            Rect::new(3, 2, 26, 1),
            (9, 2),
            (20, 3),
            &[]
        ),
        None
    );
    assert_eq!(place(AREA, row, (9, 4), (65, 3), &[]), None);
}

#[test]
fn text_wraps_within_two_lines_and_keeps_an_action_key_whole() {
    let theme = Theme::rgb_test_dark();
    let action = TooltipContent::Action {
        label: "Close Workspace…".to_owned(),
        key: Some("Ctrl+b Shift+D".to_owned()),
    };
    assert_eq!(
        text(&lines(&action, 40, &theme)),
        ["Close Workspace…  Ctrl+b Shift+D"]
    );
    assert_eq!(
        text(&lines(&action, 24, &theme)),
        ["Close W…  Ctrl+b Shift+D"]
    );

    let name = |text: &str, middle| TooltipText {
        text: text.to_owned(),
        middle,
    };
    let describe = |text, legend| TooltipContent::Describe {
        text: Some(text),
        legend,
    };
    // Spaces break lines; whatever two lines cannot hold ends the last with the ellipsis.
    assert_eq!(
        text(&lines(
            &describe(
                name(
                    "Claude Code · tooltip placement rules for every edge",
                    false
                ),
                vec![]
            ),
            20,
            &theme
        )),
        ["Claude Code ·", "tooltip placement r…"]
    );
    // Chinese may break between characters, two cells each.
    assert_eq!(
        text(&lines(
            &describe(name("侧栏提示的放置规则与关闭方式", false), vec![]),
            10,
            &theme
        )),
        ["侧栏提示的", "放置规则…"]
    );
    // A path keeps both ends.
    assert_eq!(
        text(&lines(
            &describe(name("~/code/orchbee-worktrees/orc-217/apps", true), vec![]),
            12,
            &theme
        )),
        ["~/code/orchb", "…rc-217/apps"]
    );
}

#[test]
fn a_legend_keeps_two_cells_between_items_and_ends_a_cut_line_with_a_secondary_ellipsis() {
    let theme = Theme::rgb_test_dark().with_glyphs(GlyphSet::ASCII);
    let item = |glyph: &str, label: &str| LegendItem {
        glyph: glyph.to_owned(),
        role: Role::StatusWorking,
        style: theme.style(Role::StatusWorking),
        label: label.to_owned(),
    };
    let legend = vec![
        item("!", "Needs attention"),
        item("x", "Failed"),
        item("v", "Completed"),
        item("*", "Unread activity"),
    ];
    let content = TooltipContent::Describe {
        text: None,
        legend: legend.clone(),
    };
    assert_eq!(
        text(&lines(&content, 40, &theme)),
        [
            "! Needs attention  x Failed  v Completed",
            "* Unread activity"
        ]
    );
    // A cut name keeps its line; the legend takes the one left and says more were left out.
    let content = TooltipContent::Describe {
        text: Some(TooltipText {
            text: "release-notes".to_owned(),
            middle: false,
        }),
        legend,
    };
    let painted = lines(&content, 30, &theme);
    assert_eq!(text(&painted), ["release-notes", "! Needs attention  .."]);
    let more = painted[1].spans.last().unwrap();
    assert_eq!(more.style.fg, theme.style(Role::TextSecondary).fg);
}

#[test]
fn a_roster_lists_one_agent_per_line_and_counts_the_ones_it_cuts() {
    let theme = Theme::rgb_test_dark().with_glyphs(GlyphSet::ASCII);
    let agent = |index: usize| RosterItem {
        state: LegendItem {
            glyph: "!".to_owned(),
            role: Role::StatusWorking,
            style: theme.style(Role::StatusWorking),
            label: "Working".to_owned(),
        },
        identity: format!("agent {index}"),
    };
    let roster = |count: usize| TooltipContent::Roster {
        items: (0..count).map(&agent).collect(),
    };
    // A roster that fits lists every Agent in its order, one per line: glyph, state, identity.
    let fits = text(&lines(&roster(3), 40, &theme));
    assert_eq!(fits.len(), 3);
    for (index, line) in fits.iter().enumerate() {
        assert!(
            line.starts_with("! Working") && line.ends_with(&format!("agent {index}")),
            "{line}"
        );
    }
    // A row that runs out of room cuts its identity first, so its state word stays whole.
    let narrow = text(&lines(&roster(1), 16, &theme));
    assert!(
        narrow[0].starts_with("! Working")
            && narrow[0].ends_with(theme.glyphs.ellipsis)
            && display_width(&narrow[0]) <= 16,
        "{narrow:?}"
    );
    // A longer one stops at eight lines, and its last line counts the Agents it leaves out.
    let cut = text(&lines(&roster(10), 40, &theme));
    assert_eq!(cut.len(), 8);
    assert_eq!(cut[7], "+3");
    assert!(cut[6].ends_with("agent 6"), "{cut:?}");
}
