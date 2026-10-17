use super::{paginate_rows, render, render_refused, wrap_display};
#[cfg(windows)]
use crate::tui_theme::GlyphSet;
use crate::{
    client_view::{
        ClientView, KeyCode, KeyEvent, Modifiers, Overlay, PrefixInput, SessionActionDisposition,
        help::{HelpControl, HelpDrawSource},
    },
    localization::Locale,
    terminal_text::display_width,
    tui_pointer::{FrameHitMap, HitLayer, HitMapBuilder, HitTarget, ListScrollTarget},
    tui_theme::{Role, Theme},
};
use ratatui::{
    Terminal,
    backend::TestBackend,
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
};

fn press(view: &mut ClientView, code: KeyCode, mods: Modifiers) {
    let transition = view.decide(PrefixInput::Key(KeyEvent::press(code, mods)));
    view.commit(transition, SessionActionDisposition::KeepNavigator);
}

fn open_help(view: &mut ClientView) {
    press(view, KeyCode::Char('b'), Modifiers::CTRL);
    press(view, KeyCode::Char('?'), Modifiers::NONE);
    assert_eq!(view.overlay(), Some(Overlay::ShortcutHelp));
}

fn page_forward(view: &mut ClientView) {
    press(view, KeyCode::Char('b'), Modifiers::CTRL);
    press(view, KeyCode::PageDown, Modifiers::NONE);
}

fn render_panel(
    width: u16,
    height: u16,
    view: &ClientView,
    locale: Locale,
    theme: &Theme,
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
                super::RenderContext::new(locale.shell(), locale, view, candidate),
                theme,
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

fn rows(buffer: &Buffer) -> Vec<String> {
    (buffer.area.y..buffer.area.bottom())
        .map(|y| {
            (buffer.area.x..buffer.area.right())
                .map(|x| buffer[(x, y)].symbol())
                .collect()
        })
        .collect()
}

fn glyph_rows(buffer: &Buffer) -> Vec<String> {
    (buffer.area.y..buffer.area.bottom())
        .map(|y| {
            let mut row = String::new();
            let mut x = buffer.area.x;
            while x < buffer.area.right() {
                let symbol = buffer[(x, y)].symbol();
                row.push_str(symbol);
                x = x.saturating_add(u16::try_from(display_width(symbol).max(1)).unwrap());
            }
            row
        })
        .collect()
}

#[cfg(windows)]
#[test]
fn detach_lifecycle_explanation_survives_help_wrapping_and_matches_the_menu() {
    for (locale, explanation) in [
        (Locale::EnUs, "Detach this Client; programs keep running"),
        (Locale::ZhCn, "分离当前 Client；终端程序继续运行"),
    ] {
        assert_eq!(
            locale.menu_action(&crate::action_menu::MenuAction::Detach),
            explanation
        );
        let compact = |text: &str| {
            text.chars()
                .filter(|character| !character.is_whitespace())
                .collect::<String>()
        };
        for (width, height) in [(60, 12), (80, 24), (120, 40)] {
            let mut view = ClientView::new(width);
            open_help(&mut view);
            let mut found = false;
            let mut reached_last_page = false;
            for _ in 0..128 {
                let (buffer, map, placed) =
                    render_panel(width, height, &view, locale, &Theme::one_light());
                assert!(
                    placed.is_some(),
                    "the complete help remains available at {width}x{height}"
                );
                found |= compact(&glyph_rows(&buffer).join(" ")).contains(&compact(explanation));
                let HelpDrawSource::Presented { page, last, .. } = *map.help_draw_source().unwrap()
                else {
                    panic!("the lifecycle explanation must be pageable");
                };
                if page == last {
                    reached_last_page = true;
                    break;
                }
                page_forward(&mut view);
            }
            assert!(reached_last_page, "help paging must reach the final page");
            assert!(
                found,
                "detach lifecycle copy was lost at {width}x{height} in {locale:?}"
            );
        }
    }
}

#[test]
fn complete_panel_publishes_exact_source_and_all_three_control_slots() {
    let mut view = ClientView::new(60);
    open_help(&mut view);
    let theme = Theme::rgb_test_dark();
    let (buffer, map, placed) = render_panel(60, 12, &view, Locale::EnUs, &theme);
    let area = placed.expect("60x12 must admit paged Help");
    let source = *map
        .help_draw_source()
        .expect("a complete panel publishes its exact source");
    let HelpDrawSource::Presented { page, last, .. } = source else {
        panic!("a complete panel cannot publish refused authority");
    };
    assert_eq!(page, 0);
    assert!(last > 0, "this geometry must exercise paging");

    let close = Rect::new(area.right() - 4, area.y, 3, 1);
    let next = Rect::new(close.x - 4, area.y, 3, 1);
    let previous = Rect::new(next.x - 4, area.y, 3, 1);
    for (rect, action) in [
        (close, HelpControl::Close),
        (previous, HelpControl::Previous),
        (next, HelpControl::Next),
    ] {
        assert!(matches!(
            map.hit_target_at(rect.x, rect.y),
            Some(HitTarget::Help { page: 0, action: actual }) if *actual == action
        ));
    }
    assert_eq!(buffer[(previous.x, previous.y)].symbol(), "[");
    assert_eq!(buffer[(next.x + 1, next.y)].symbol(), ">");
    assert_eq!(buffer[(close.x + 1, close.y)].symbol(), "x");

    let content = Rect::new(area.x + 3, area.y + 1, area.width - 6, area.height - 4);
    assert!(matches!(
        map.hit_target_at(content.x, content.y),
        Some(HitTarget::ListScroll(ListScrollTarget::Help(actual))) if *actual == source
    ));
}

#[test]
fn requested_page_clamps_only_through_the_presented_source() {
    let mut view = ClientView::new(60);
    open_help(&mut view);
    for _ in 0..64 {
        page_forward(&mut view);
    }
    let theme = Theme::rgb_test_dark();
    let (buffer, map, placed) = render_panel(60, 12, &view, Locale::EnUs, &theme);
    placed.expect("Help must remain renderable after an oversized page request");
    let HelpDrawSource::Presented { page, last, .. } = *map.help_draw_source().unwrap() else {
        panic!("complete Help must publish a presented source");
    };
    assert_eq!(page, last);
    assert!(last > 0);
    let rendered = rows(&buffer).join("\n");
    assert!(rendered.contains(&format!("{}/{}", last + 1, last + 1)));
    assert!(rendered.contains("Typing continues"));
    assert!(rendered.contains("Previous page"));
}

#[test]
fn english_and_chinese_wrap_complete_descriptions_without_clipping() {
    let theme = Theme::rgb_test_dark();
    for (locale, expected) in [
        (Locale::EnUs, "Typing continues"),
        (Locale::ZhCn, "输入继续传递"),
    ] {
        let mut view = ClientView::new(46);
        open_help(&mut view);
        let mut rendered = String::new();
        for _ in 0..24 {
            let (buffer, map, placed) = render_panel(46, 20, &view, locale, &theme);
            placed.expect("46x20 must retain whole-row Help paging");
            rendered.push_str(&glyph_rows(&buffer).join("\n"));
            let HelpDrawSource::Presented { page, last, .. } = *map.help_draw_source().unwrap()
            else {
                panic!("full Help must publish a presented source");
            };
            if page == last {
                break;
            }
            page_forward(&mut view);
        }
        assert!(rendered.contains(expected), "{locale:?}: {rendered}");
        assert!(
            rendered.contains("PageDown"),
            "paging key was clipped: {rendered}"
        );
    }
}

#[cfg(windows)]
#[test]
fn global_footer_uses_default_overridden_and_unbound_effective_actions() {
    use orchbee_local_site_runtime::{
        WindowsKeyChord, WindowsKeyOverride, WindowsKeySettings, WindowsKeyTrigger,
    };

    fn chord(key: &str, ctrl: bool, alt: bool) -> WindowsKeyChord {
        WindowsKeyChord {
            key: key.into(),
            ctrl,
            alt,
            shift: false,
            super_key: false,
        }
    }

    let theme = Theme::rgb_test_dark().with_glyphs(GlyphSet::ASCII);
    let mut view = ClientView::new(100);
    open_help(&mut view);
    let (buffer, _, area) = render_panel(100, 22, &view, Locale::EnUs, &theme);
    let area = area.unwrap();
    let default = rows(&buffer);
    assert!(default[usize::from(area.bottom() - 3)].contains("Ctrl+b ? Close help"));
    assert!(default[usize::from(area.bottom() - 2)].contains("Ctrl+b PageUp Previous page"));
    assert!(default[usize::from(area.bottom() - 2)].contains("Ctrl+b PageDown Next page"));

    view.configure_keys(&WindowsKeySettings {
        prefix: chord("a", true, false),
        overrides: vec![WindowsKeyOverride {
            action: "help",
            triggers: vec![WindowsKeyTrigger {
                prefix: false,
                chord: chord("h", false, true),
            }],
        }],
        commands: Vec::new(),
    });
    let (buffer, _, area) = render_panel(100, 22, &view, Locale::EnUs, &theme);
    let area = area.unwrap();
    let overridden = rows(&buffer);
    assert!(overridden[usize::from(area.bottom() - 3)].contains("Alt+h Close help"));
    assert!(overridden[usize::from(area.bottom() - 2)].contains("Ctrl+a PageUp Previous page"));
    assert!(overridden[usize::from(area.bottom() - 2)].contains("Ctrl+a PageDown Next page"));

    view.configure_keys(&WindowsKeySettings {
        prefix: chord("a", true, false),
        overrides: vec![WindowsKeyOverride {
            action: "help",
            triggers: Vec::new(),
        }],
        commands: Vec::new(),
    });
    let (buffer, _, area) = render_panel(100, 22, &view, Locale::EnUs, &theme);
    let area = area.unwrap();
    let unbound = rows(&buffer);
    let top = &unbound[usize::from(area.bottom() - 3)];
    assert!(!top.contains("Close help"), "no fallback chord: {top:?}");
    assert!(top.contains("Typing continues"));
    assert!(unbound[usize::from(area.bottom() - 2)].contains("Ctrl+a PageUp Previous page"));
}

#[cfg(windows)]
#[test]
fn copy_mode_footer_uses_its_complete_local_binding_groups() {
    let mut view = ClientView::new(100);
    view.settle_copy_mode(true);
    assert!(view.copy_help_key(KeyEvent::press(KeyCode::F(1), Modifiers::NONE)));
    let theme = Theme::rgb_test_dark();
    let (buffer, _, area) = render_panel(100, 22, &view, Locale::EnUs, &theme);
    let area = area.unwrap();
    let rows = rows(&buffer);
    let top = &rows[usize::from(area.bottom() - 3)];
    let paging = &rows[usize::from(area.bottom() - 2)];
    assert!(top.contains("F1 Close help"), "{top:?}");
    assert!(top.contains("q / Esc Exit"), "{top:?}");
    assert!(!top.contains("Typing continues"), "{top:?}");
    assert!(
        paging.contains("PageUp / PageDown Turn pages"),
        "{paging:?}"
    );
}

#[cfg(windows)]
#[test]
fn resize_mode_footer_uses_its_typed_registry_and_key_hint_layers() {
    let mut view = ClientView::new(100);
    press(&mut view, KeyCode::Char('b'), Modifiers::CTRL);
    press(&mut view, KeyCode::Char('P'), Modifiers::NONE);
    assert!(view.resize_mode());
    press(&mut view, KeyCode::Char('?'), Modifiers::NONE);

    let theme = Theme::rgb_test_dark().with_glyphs(GlyphSet::ASCII);
    let (buffer, map, area) = render_panel(100, 22, &view, Locale::EnUs, &theme);
    let area = area.expect("resize Help must present a complete panel");
    assert!(matches!(
        map.help_draw_source(),
        Some(HelpDrawSource::Presented { .. })
    ));
    let top_y = area.bottom() - 3;
    let paging_y = area.bottom() - 2;
    let rendered = rows(&buffer);
    let top = &rendered[usize::from(top_y)];
    let paging = &rendered[usize::from(paging_y)];
    assert!(top.contains("? Close help"), "{top:?}");
    assert!(top.contains("Esc Leave resize mode"), "{top:?}");
    assert!(!top.contains("Typing continues"), "{top:?}");
    assert!(
        paging.contains("PageUp / PageDown Turn pages"),
        "{paging:?}"
    );

    for (y, key, label_offset) in [
        (top_y, "?", 2_u16),
        (top_y, "E", 4_u16),
        (paging_y, "P", 18_u16),
    ] {
        let x = (area.x..area.right())
            .find(|x| buffer[(*x, y)].symbol() == key)
            .expect("each typed resize hint key must be painted");
        assert_eq!(buffer[(x, y)].style().fg, theme.style(Role::HintKey).fg);
        assert_eq!(
            buffer[(x + label_offset, y)].style().fg,
            theme.style(Role::HintLabel).fg
        );
    }
}

#[test]
fn rejected_panel_mutates_neither_underlay_nor_existing_hits() {
    let mut view = ClientView::new(16);
    open_help(&mut view);
    let candidate = view.help_draw_candidate().unwrap();
    let theme = Theme::rgb_test_dark();
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
                cell.set_symbol("q").set_style(
                    Style::default()
                        .fg(Color::Yellow)
                        .bg(Color::Blue)
                        .add_modifier(Modifier::BOLD),
                );
            }
            let before = frame.buffer_mut().clone();
            placed = render(
                frame,
                frame.area(),
                super::RenderContext::new(Locale::EnUs.shell(), Locale::EnUs, &view, candidate),
                &theme,
                &mut hits,
            );
            assert_eq!(&*frame.buffer_mut(), &before);
        })
        .unwrap();
    assert_eq!(placed, None);
    let map = hits.finish();
    assert!(map.help_draw_source().is_none());
    for y in 0..7 {
        for x in 0..16 {
            assert_eq!(map.hit_target_at(x, y), Some(&HitTarget::OverlaySurface));
        }
    }
}

#[test]
fn narrow_refusal_publishes_only_its_exact_close_and_refused_source() {
    let mut view = ClientView::new(7);
    open_help(&mut view);
    let candidate = view.help_draw_candidate().unwrap();
    let theme = Theme::rgb_test_dark();
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
                &theme,
                &mut hits,
            );
        })
        .unwrap();
    let close = close.expect("seven cells admit a complete three-cell close");
    assert_eq!(close, Rect::new(4, 0, 3, 1));
    let row = rows(terminal.backend().buffer()).remove(0);
    assert!(row.ends_with("[x]"), "{row:?}");
    assert!(!row.contains("[<]") && !row.contains("[>]"), "{row:?}");
    let map = hits.finish();
    assert!(matches!(
        map.help_draw_source(),
        Some(HelpDrawSource::Refused { page: 0, .. })
    ));
    assert!(matches!(
        map.hit_target_at(close.x, close.y),
        Some(HitTarget::Help {
            page: 0,
            action: HelpControl::Close
        })
    ));
    assert!(
        !matches!(
            map.hit_target_at(close.x, close.y),
            Some(HitTarget::ListScroll(_))
        ),
        "refusal must not invent paging authority"
    );
}

#[test]
fn pure_wrapping_and_pagination_preserve_graphemes_and_logical_rows() {
    assert_eq!(wrap_display("ab", 0), None);
    assert_eq!(wrap_display("", 0), Some(vec![String::new()]));
    assert_eq!(wrap_display("ab", 1), Some(vec!["a".into(), "b".into()]));
    assert_eq!(wrap_display("中", 1), None);
    assert_eq!(wrap_display("中", 2), Some(vec!["中".into()]));
    assert_eq!(wrap_display("a\u{301}", 1), Some(vec!["a\u{301}".into()]));

    assert_eq!(
        paginate_rows(&[1; 21], 6),
        Some(vec![(0, 6), (6, 12), (12, 18), (18, 21)])
    );
    assert_eq!(
        paginate_rows(&[3, 3, 1, 4], 5),
        Some(vec![(0, 1), (1, 3), (3, 4)])
    );
    assert_eq!(paginate_rows(&[1, 6, 1], 4), None);
    assert_eq!(paginate_rows(&[2], 0), None);
}
