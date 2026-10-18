use super::*;
use orchbee_workbench_protocol::PanelAgentState;
use ratatui::{Terminal, backend::TestBackend, style::Modifier};
use std::borrow::Cow;

fn paint(width: u16, summary: CompactAgentSummary<'_>, locale: Locale) -> (String, Rect) {
    let mut terminal = Terminal::new(TestBackend::new(width, 1)).unwrap();
    let theme = Theme::rgb_test_dark();
    let mut tabs = Rect::default();
    terminal
        .draw(|frame| tabs = render(frame, frame.area(), summary, locale, &theme))
        .unwrap();
    let text = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>()
        .trim()
        .to_owned();
    (text, tabs)
}

#[test]
fn compact_widths_keep_the_status_distinct_and_leave_unused_cells_for_tabs() {
    let mut row = crate::machine_agents::tests::row(61);
    row.state = PanelAgentState::Blocked;
    for (width, expected) in [
        (23, "Agent: Blocked!"),
        (43, "source unavailable"),
        (62, "Observed Agent: Blocked"),
    ] {
        let (text, tabs) = paint(
            width,
            CompactAgentSummary::Observed {
                row: &row,
                coverage: AgentSourceCoverage::Unavailable,
            },
            Locale::EnUs,
        );
        assert!(text.contains(expected), "{width}: {text:?}");
        assert!(tabs.width >= MIN_TAB_STRIP_CELLS, "{width}: {tabs:?}");
    }

    let (text, tabs) = paint(23, CompactAgentSummary::Empty, Locale::EnUs);
    assert_eq!(text, "Agent: None");
    assert_eq!(usize::from(tabs.width) + display_width(&text), 23);
}

#[test]
fn known_none_loading_unavailable_and_no_matches_have_distinct_localized_fallbacks() {
    let none = summary_text(
        CompactAgentSummary::Empty,
        Locale::EnUs,
        16,
        &Theme::rgb_test_dark(),
    );
    let loading = summary_text(
        CompactAgentSummary::Loading,
        Locale::EnUs,
        16,
        &Theme::rgb_test_dark(),
    );
    let unavailable = summary_text(
        CompactAgentSummary::Unavailable,
        Locale::EnUs,
        16,
        &Theme::rgb_test_dark(),
    );
    let no_matches = summary_text(
        CompactAgentSummary::NoMatches,
        Locale::EnUs,
        16,
        &Theme::rgb_test_dark(),
    );
    assert_eq!(none, "Agent: None");
    assert_eq!(loading, "Agent: Loading");
    assert_eq!(unavailable, "Unavailable");
    assert_eq!(no_matches, "No matches");

    let mut row = crate::machine_agents::tests::row(62);
    row.state = PanelAgentState::Blocked;
    let zh = summary_text(
        CompactAgentSummary::Observed {
            row: &row,
            coverage: AgentSourceCoverage::Unavailable,
        },
        Locale::ZhCn,
        16,
        &Theme::rgb_test_dark(),
    );
    assert!(zh.contains("受阻"), "{zh:?}");
    assert!(zh.ends_with('!'), "{zh:?}");
}

#[test]
fn narrow_summary_keeps_a_drawn_active_tab_target_after_create_space_is_reserved() {
    let mut row = crate::machine_agents::tests::row(63);
    row.state = PanelAgentState::Blocked;
    let mut terminal = Terminal::new(TestBackend::new(23, 1)).unwrap();
    let theme = Theme::rgb_test_dark();
    let mut painted = Vec::new();
    let mut tabs_area = Rect::default();
    terminal
        .draw(|frame| {
            tabs_area = render(
                frame,
                frame.area(),
                CompactAgentSummary::Observed {
                    row: &row,
                    coverage: AgentSourceCoverage::Unavailable,
                },
                Locale::EnUs,
                &theme,
            );
            let tab_area = Rect::new(
                tabs_area.x,
                tabs_area.y,
                tabs_area.width.saturating_sub(4),
                tabs_area.height,
            );
            let tabs = [super::super::tabs::groups::GroupTabDisplay {
                tab: super::super::tabs::TabDisplay {
                    position: 1,
                    active: true,
                    label: Cow::Borrowed("active tab"),
                },
                group: None,
                collapsed: false,
                selected: false,
            }];
            let mut project = |_: &Vec<Rect>, _: usize, _: bool, _: Rect| {
                crate::tui_pointer::ProjectedPointer::Rest
            };
            let mut record = |painted: &mut Vec<Rect>, _: usize, is_group: bool, rect| {
                if !is_group {
                    painted.push(rect);
                }
            };
            super::super::tabs::groups::render(
                frame,
                tab_area,
                &tabs,
                &theme,
                "Group",
                super::super::tabs::groups::GroupInteractions::new(
                    &mut painted,
                    &mut project,
                    &mut record,
                ),
            );
        })
        .unwrap();
    assert!(tabs_area.width >= MIN_TAB_STRIP_CELLS);
    assert_eq!(painted.len(), 1);
    assert!(!painted[0].is_empty());
    let buffer = terminal.backend().buffer();
    assert!((painted[0].x..painted[0].right()).any(|x| {
        buffer
            .get(x, painted[0].y)
            .style()
            .add_modifier
            .contains(Modifier::BOLD)
    }));
}
