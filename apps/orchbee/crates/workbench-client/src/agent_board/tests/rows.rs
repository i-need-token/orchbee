//! Painted Board rows name sessions without exposing internal identifiers.
use super::*;
use crate::{localization::Locale, tui_hit_map::HitMapBuilder, tui_theme::Theme};

fn painted_lines(board: &AgentBoard, index: &PresentationIndex, locale: Locale) -> Vec<String> {
    let (width, height) = (110, 16);
    let mut terminal =
        ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height)).unwrap();
    let mut hits = HitMapBuilder::new();
    terminal
        .draw(|frame| {
            crate::tui_render::render_agent_board_for_test(
                frame,
                locale,
                &AgentBoardView::new(board, index),
                &Theme::orchbee_light(),
                &mut hits,
            );
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    (0..height)
        .map(|y| {
            let mut line = String::new();
            let mut x = 0;
            while x < width {
                let symbol = buffer[(x, y)].symbol();
                line.push_str(symbol);
                // A wide glyph owns its following cells; their blanks are not text.
                x += u16::try_from(crate::terminal_text::display_width(symbol).max(1)).unwrap();
            }
            line
        })
        .collect()
}

/// The sidebar's row for an Agent that has no canonical session: started by the picker or typed
/// by hand, without a name, a conversation id or a hook integration.
fn sidebar_agent() -> crate::machine_agents::checkpoint::Row {
    let mut row = crate::machine_agents::test_support::row(23);
    row.state = orchbee_workbench_protocol::PanelAgentState::Idle;
    row
}

fn settle_empty_list(board: &mut AgentBoard, now: Instant) {
    assert!(board.drive_intent(now, accepted));
    let episode = board.episode();
    board.settle_read(
        AgentBoardReadEvent::Answer(AgentBoardReadAnswer::List {
            episode,
            result: Ok(AgentListRound {
                items: Vec::new(),
                complete: true,
            }),
        }),
        now,
        &mut no_preference,
    );
}

#[test]
fn every_agent_the_sidebar_shows_is_a_row_without_a_canonical_session() {
    let mut board = AgentBoard::new(true);
    board.observe_session_agents(SessionAgents::from_rows(&[sidebar_agent()]));
    board.open();
    settle_empty_list(&mut board, Instant::now());
    let index = PresentationIndex::rebuild(&crate::machine_agents::test_support::fixture());
    let text = painted_lines(&board, &index, Locale::EnUs).join("\n");
    assert!(!text.contains("No Agent sessions"), "{text}");
    assert_eq!(board.rows().len(), 1, "{text}");
    assert!(text.contains("Codex"), "{text}");
    // No canonical session exists to Get: the jump goes straight to the sidebar's Presentation.
    board.request_jump();
    assert!(board.drive_intent(Instant::now(), |request| {
        panic!("a sidebar row reads nothing: {request:?}")
    }));
    let Some(JumpStage::Navigating { candidates, .. }) = board.jump().map(|jump| &jump.stage)
    else {
        panic!("navigating jump");
    };
    assert_eq!(candidates[0].presentation_id.as_str(), hex(23));
}

#[test]
fn rows_show_title_provider_and_workspace_or_a_placeholder_never_the_session_id() {
    let mut board = AgentBoard::new(true);
    board.rows = vec![
        AgentBoardRow::from_summary(summary(
            "desktop:agent:session:1",
            vec![attachment(&hex(42), true)],
        )),
        AgentBoardRow::from_summary(summary("desktop:agent:session:2", vec![])),
    ];
    board.open();
    board.intent = None;
    let mut index = PresentationIndex {
        ready: true,
        ..PresentationIndex::default()
    };
    index.placements.insert(
        hex(42),
        PresentationPlacement {
            tab_label: "Fix parser".into(),
            panel_name: NameFacts::default(),
            panel_position: 1,
            workspace_label: Some("orchbee".into()),
            ..placement(workspace_location(1), 2, 3)
        },
    );
    for (locale, placeholder) in [
        (Locale::EnUs, "No navigable panel"),
        (Locale::ZhCn, "无可跳转面板"),
    ] {
        let lines = painted_lines(&board, &index, locale);
        let text = lines.join("\n");
        assert!(!text.contains("desktop:agent:session"), "{text}");
        let titled = lines
            .iter()
            .find(|line| line.contains("Fix parser"))
            .unwrap_or_else(|| panic!("titled row: {text}"));
        assert!(
            titled.contains("Devin") && titled.contains("orchbee"),
            "{titled}"
        );
        assert_eq!(titled.matches("Fix parser").count(), 1, "{titled}");
        let unplaced = lines
            .iter()
            .find(|line| line.contains(placeholder))
            .unwrap_or_else(|| panic!("placeholder row: {text}"));
        assert!(unplaced.contains("Devin"), "{unplaced}");
    }
}
