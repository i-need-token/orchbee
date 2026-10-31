use super::{render, render_refused};
use crate::{
    client_view::{
        ClientAction, ClientActionDisposition, ClientView, KeyCode, KeyEvent, Modifiers, Overlay,
        PrefixInput,
        help::{HelpControl, HelpDrawSource},
    },
    localization::Locale,
    tui_hit_map::{FrameHitMap, HitLayer, HitMapBuilder, HitTarget, ListScrollTarget},
    tui_theme::{Role, Theme},
};
use ratatui::{
    Terminal,
    backend::TestBackend,
    buffer::{Buffer, CellWidth},
    layout::Rect,
};

fn press(view: &mut ClientView, code: KeyCode, mods: Modifiers) -> Option<ClientAction> {
    let transition = view.decide(PrefixInput::Key(KeyEvent::press(code, mods)));
    let action = transition.action();
    view.commit(transition, ClientActionDisposition::KeepNavigator);
    action
}

fn open_help(view: &mut ClientView) {
    press(view, KeyCode::Char('b'), Modifiers::CTRL);
    press(view, KeyCode::Char('?'), Modifiers::NONE);
    assert_eq!(view.overlay(), Some(Overlay::ShortcutHelp));
}

fn render_panel(
    width: u16,
    height: u16,
    view: &ClientView,
    locale: Locale,
    session_available: bool,
) -> (Buffer, FrameHitMap, Option<Rect>) {
    let candidate = view.help_draw_candidate().expect("Help must be open");
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut hits = HitMapBuilder::new();
    let mut placed = None;
    terminal
        .draw(|frame| {
            placed = render(
                frame,
                frame.area(),
                super::RenderContext::new(
                    locale.shell(),
                    locale,
                    view,
                    session_available,
                    candidate,
                ),
                &Theme::rgb_test_dark(),
                &mut hits,
            );
            if let Some(area) = placed {
                hits.mark_overlay_active();
                hits.record(HitLayer::OverlaySurface, area, HitTarget::OverlaySurface);
            }
        })
        .unwrap();
    (terminal.backend().buffer().clone(), hits.finish(), placed)
}

fn rows(buffer: &Buffer) -> String {
    (buffer.area.y..buffer.area.bottom())
        .map(|y| {
            let mut row = String::new();
            let mut hidden_tail_cells = 0;
            for x in buffer.area.x..buffer.area.right() {
                if hidden_tail_cells > 0 {
                    hidden_tail_cells -= 1;
                    continue;
                }
                let symbol = buffer[(x, y)].symbol();
                row.push_str(symbol);
                hidden_tail_cells = symbol.cell_width().saturating_sub(1);
            }
            row
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn grouped_help_localizes_and_fits_standard_terminal_sizes() {
    for (width, height) in [(80, 24), (120, 40)] {
        for (locale, labels) in [
            (
                Locale::EnUs,
                ["Workspace", "Tab", "Panel", "Agent", "Client", "Other"],
            ),
            (
                Locale::ZhCn,
                ["工作区", "标签页", "面板", "智能体", "客户端", "其他"],
            ),
        ] {
            let mut view = ClientView::new(width);
            open_help(&mut view);
            press(&mut view, KeyCode::Tab, Modifiers::NONE);
            assert!(view.help_expanded(), "Tab expands every group");
            let mut rendered = String::new();
            let mut complete = false;
            for _ in 0..128 {
                let (buffer, map, area) = render_panel(width, height, &view, locale, true);
                area.expect("Help should fit a standard terminal size");
                rendered.push_str(&rows(&buffer));
                rendered.push('\n');
                let source = *map.help_draw_source().unwrap();
                let HelpDrawSource::Presented { page, last, .. } = source else {
                    panic!("complete Help must publish a scroll source");
                };
                view.settle_help_draw(Some(source));
                if page == last {
                    complete = true;
                    break;
                }
                press(&mut view, KeyCode::Down, Modifiers::NONE);
            }
            assert!(complete, "scrolling must reach the final line");
            for label in labels {
                assert!(
                    rendered.contains(label),
                    "{locale:?} {width}x{height}: {rendered}"
                );
            }
            assert!(rendered.contains(locale.shortcut_close_action()));
            assert!(rendered.contains(locale.shortcut_scroll_action()));
        }
    }
}

#[test]
fn word_prefix_filter_uses_semantic_names_and_clear_restores_the_list() {
    let mut view = ClientView::new(80);
    open_help(&mut view);
    for character in "work dir".chars() {
        press(&mut view, KeyCode::Char(character), Modifiers::NONE);
    }
    let (buffer, _, _) = render_panel(80, 24, &view, Locale::EnUs, true);
    let filtered = rows(&buffer);
    assert!(
        filtered.contains("Ctrl+b e") && filtered.contains("Set Directory…"),
        "{filtered}"
    );
    assert!(
        filtered.contains("Workspace"),
        "the result keeps its group: {filtered}"
    );
    assert!(!filtered.contains("New Tab"), "{filtered}");

    press(&mut view, KeyCode::Char('u'), Modifiers::CTRL);
    assert!(view.help_filter().is_empty());
    let (buffer, _, _) = render_panel(80, 24, &view, Locale::EnUs, true);
    assert!(rows(&buffer).contains("New Tab"));
}

#[test]
fn key_and_alias_filtering_work_in_both_locales() {
    for (locale, query, expected) in [
        (Locale::EnUs, "ctrl b", "New Workspace"),
        (Locale::EnUs, "shift n", "New Workspace"),
        (Locale::ZhCn, "工作区", "新建工作区"),
    ] {
        let mut view = ClientView::new(120);
        open_help(&mut view);
        for character in query.chars() {
            press(&mut view, KeyCode::Char(character), Modifiers::NONE);
        }
        let (buffer, _, _) = render_panel(120, 40, &view, locale, true);
        let rendered = rows(&buffer);
        assert!(rendered.contains(expected), "{locale:?}: {rendered}");
        assert!(rendered.contains(locale.shortcut_search_label()));
    }
}

#[test]
fn unavailable_actions_show_a_localized_reason_in_the_disabled_role() {
    let mut view = ClientView::new(80);
    open_help(&mut view);
    let (buffer, _, _) = render_panel(80, 24, &view, Locale::EnUs, false);
    let rendered = rows(&buffer);
    assert!(rendered.contains("Connect to an Orchbee session"));
    let disabled = Theme::rgb_test_dark().style(Role::TextDisabled).fg;
    assert!(
        buffer
            .content
            .iter()
            .any(|cell| { cell.style().fg == disabled && !cell.symbol().trim().is_empty() })
    );
}

#[test]
fn scrolling_publishes_exact_line_offset_and_stale_controls_are_rejected() {
    let mut view = ClientView::new(80);
    open_help(&mut view);
    press(&mut view, KeyCode::Tab, Modifiers::NONE);
    let (_, map, _) = render_panel(80, 24, &view, Locale::EnUs, true);
    let source = *map.help_draw_source().unwrap();
    let HelpDrawSource::Presented {
        page, last, rows, ..
    } = source
    else {
        panic!("complete Help must publish scroll bounds");
    };
    assert_eq!(page, 0);
    assert!(last > 0, "80x24 should require scrolling");
    view.settle_help_draw(Some(source));

    assert_eq!(
        press(&mut view, KeyCode::Down, Modifiers::NONE),
        Some(ClientAction::PageShortcutHelp)
    );
    assert_eq!(view.help_page(), 1);
    assert!(
        !view.activate_drawn_help(&crate::client_view::help::DrawnHelpControl::new(
            source,
            HelpControl::Next,
        ))
    );

    press(&mut view, KeyCode::Home, Modifiers::NONE);
    assert_eq!(view.help_page(), 0);
    press(&mut view, KeyCode::PageDown, Modifiers::NONE);
    assert!(rows > 1, "the published viewport has several rows");
    assert_eq!(
        view.help_page(),
        rows,
        "PageDown moves one published viewport"
    );
}

#[test]
fn filtered_help_prints_each_group_heading_once() {
    for query in ["tab", "new", "panel"] {
        let mut view = ClientView::new(120);
        open_help(&mut view);
        for character in query.chars() {
            press(&mut view, KeyCode::Char(character), Modifiers::NONE);
        }
        let (buffer, _, _) = render_panel(120, 40, &view, Locale::EnUs, true);
        let rendered = rows(&buffer);
        for heading in ["Common tasks", "Workspace", "Tab", "Panel"] {
            let headings = rendered
                .lines()
                .filter(|line| {
                    line.trim_matches(|c: char| c == '│' || c.is_whitespace()) == heading
                })
                .count();
            assert!(headings <= 1, "{query}: {heading} x{headings}\n{rendered}");
        }
    }
}

#[test]
fn drawn_controls_and_scroll_region_share_the_presented_source() {
    let mut view = ClientView::new(80);
    open_help(&mut view);
    let (_, map, area) = render_panel(80, 24, &view, Locale::EnUs, true);
    let area = area.expect("Help should fit");
    let source = *map.help_draw_source().expect("Help publishes a source");

    // `─ [<] [>] [x] ─┐`: one rule cell and one blank keep the close away from the corner.
    let close_x = area.right() - 6;
    let next_x = close_x - 4;
    let previous_x = next_x - 4;
    for (x, control) in [
        (previous_x, HelpControl::Previous),
        (next_x, HelpControl::Next),
        (close_x, HelpControl::Close),
    ] {
        assert!(matches!(
            map.hit_target_at(x, area.y),
            Some(HitTarget::Help { action, .. }) if *action == control
        ));
    }
    assert!(matches!(
        // The top edge, its blank row, the filter, and its divider precede the scroll region.
        map.hit_target_at(area.x + 3, area.y + 4),
        Some(HitTarget::ListScroll(ListScrollTarget::Help(actual))) if *actual == source
    ));
}

#[test]
fn no_match_and_backspace_are_visible_without_leaking_input() {
    let mut view = ClientView::new(80);
    open_help(&mut view);
    for character in "nomatchzz".chars() {
        assert_eq!(
            press(&mut view, KeyCode::Char(character), Modifiers::NONE),
            None
        );
    }
    let (buffer, _, _) = render_panel(80, 24, &view, Locale::EnUs, true);
    assert!(rows(&buffer).contains(Locale::EnUs.shortcut_no_matches()));
    press(&mut view, KeyCode::Backspace, Modifiers::NONE);
    assert_eq!(view.help_filter(), "nomatchz");
}

#[test]
fn bidi_filter_text_is_sanitized_for_display_while_raw_query_still_matches() {
    let mut view = ClientView::new(80);
    open_help(&mut view);
    for character in ['n', 'e', 'w', '\u{202e}', 'w', 'o'] {
        press(&mut view, KeyCode::Char(character), Modifiers::NONE);
    }
    assert_eq!(view.help_filter(), "new\u{202e}wo");

    let (buffer, _, _) = render_panel(80, 24, &view, Locale::EnUs, true);
    let rendered = rows(&buffer);
    assert!(rendered.contains("New Workspace"), "{rendered}");
    assert!(rendered.contains("Filter: newwo"), "{rendered}");
    assert!(!rendered.contains('\u{202e}'), "{rendered}");
}

#[test]
fn long_alias_lists_wrap_between_complete_chords_and_keep_other_rows_visible() {
    let mut view = ClientView::new(120);
    open_help(&mut view);
    let mut entries = crate::tui_shortcut_help::entries(
        crate::localization::Locale::EnUs,
        &view,
        true,
        crate::client_view::KeyLabelGlyphs::Unicode,
    );
    let aliases = [
        "Ctrl+Shift+Alt+Super+F12",
        "Ctrl+Shift+Alt+Super+F11",
        "Ctrl+Shift+Alt+Super+F10",
    ];
    let entry = entries
        .iter_mut()
        .find(|entry| entry.title.starts_with("Settings"))
        .expect("the Help list has a short action to use as a long-alias witness");
    entry.key = aliases.join(" / ");
    let workspace_title = entries
        .iter()
        .find(|entry| entry.key.contains("Ctrl+b a"))
        .expect("workspace creation remains discoverable beside the long aliases")
        .title
        .clone();

    let lines = super::shortcut_lines(
        &entries,
        "",
        Locale::EnUs,
        30,
        &Theme::rgb_test_dark(),
        true,
    )
    .expect("a long alias list must not refuse the complete Help surface");
    let rendered = lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    for alias in aliases {
        assert!(rendered.contains(alias), "{rendered}");
    }
    assert!(
        rendered.contains("Other"),
        "the Settings action keeps its group: {rendered}"
    );
    assert!(
        rendered.contains("New Tab"),
        "an unaffected action row remains intact: {rendered}"
    );
    let without_whitespace = |text: &str| {
        text.chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>()
    };
    assert!(
        without_whitespace(&rendered).contains(&without_whitespace(&workspace_title)),
        "the complete Workspace title survives line wrapping: {rendered}"
    );
}

#[test]
fn a_too_small_panel_leaves_the_underlay_and_hits_untouched() {
    let mut view = ClientView::new(16);
    open_help(&mut view);
    let candidate = view.help_draw_candidate().unwrap();
    let mut terminal = Terminal::new(TestBackend::new(16, 7)).unwrap();
    let mut hits = HitMapBuilder::new();
    hits.record(
        HitLayer::Status,
        Rect::new(0, 0, 16, 7),
        HitTarget::OverlaySurface,
    );
    let mut placed = Some(Rect::default());
    terminal
        .draw(|frame| {
            for cell in &mut frame.buffer_mut().content {
                cell.set_symbol("q");
            }
            let before = frame.buffer_mut().clone();
            placed = render(
                frame,
                frame.area(),
                super::RenderContext::new(
                    Locale::EnUs.shell(),
                    Locale::EnUs,
                    &view,
                    true,
                    candidate,
                ),
                &Theme::rgb_test_dark(),
                &mut hits,
            );
            assert_eq!(&*frame.buffer_mut(), &before);
        })
        .unwrap();
    assert_eq!(placed, None);
    assert!(hits.finish().help_draw_source().is_none());
}

#[test]
fn refused_surface_publishes_only_its_close_control() {
    let mut view = ClientView::new(7);
    open_help(&mut view);
    let candidate = view.help_draw_candidate().unwrap();
    let mut terminal = Terminal::new(TestBackend::new(7, 1)).unwrap();
    let mut hits = HitMapBuilder::new();
    let mut close = None;
    terminal
        .draw(|frame| {
            close = render_refused(
                frame,
                frame.area(),
                Locale::EnUs.shell(),
                candidate,
                &Theme::rgb_test_dark(),
                &mut hits,
            );
        })
        .unwrap();
    let close = close.expect("seven cells admit a complete close control");
    let map = hits.finish();
    assert!(matches!(
        map.help_draw_source(),
        Some(HelpDrawSource::Refused { .. })
    ));
    assert!(matches!(
        map.hit_target_at(close.x, close.y),
        Some(HitTarget::Help {
            action: HelpControl::Close,
            ..
        })
    ));
    assert!(!matches!(
        map.hit_target_at(close.x, close.y),
        Some(HitTarget::ListScroll(ListScrollTarget::Help(_)))
    ));
}
