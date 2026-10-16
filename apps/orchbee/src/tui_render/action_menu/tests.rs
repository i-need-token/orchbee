use super::*;
use crate::{
    action_menu::{ActionMenu, MenuToken},
    client_view::{ClientAction, ClientView, PrefixAction},
    tui_pointer::{FrameHitMap, HitLayer, HitTarget},
};
use orchbee_local_site_runtime::{WindowsKeyChord, WindowsKeySettings};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, style::Style, text::Span};

mod tab_scope;

#[test]
fn context_menus_omit_chrome_and_hidden_controls_without_changing_global_discovery() {
    for (width, height) in [(120, 40), (80, 24)] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for theme in [
                Theme::terminal_native(),
                Theme::one_light(),
                Theme::one_dark(),
            ] {
                for context in [false, true] {
                    let client = ClientView::new(width);
                    let mut menu = ActionMenu::default();
                    assert!(menu.open_for(
                        if context {
                            crate::action_menu::MenuTarget::Create(None)
                        } else {
                            crate::action_menu::MenuTarget::Global
                        },
                        "QZX".into(),
                        vec![MenuAction::Help, MenuAction::Reload],
                        Some((3, 2)),
                    ));
                    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                    let mut hits = HitMapBuilder::new();
                    let mut presentation = ModalPresentation::NotPresented;
                    terminal
                        .draw(|frame| {
                            presentation = render(
                                frame,
                                frame.area(),
                                menu.view().unwrap(),
                                client.bindings(),
                                locale,
                                &theme,
                                &mut hits,
                            );
                        })
                        .unwrap();
                    let ModalPresentation::Presented(draw) = presentation else {
                        panic!("both menu kinds fit the declared geometry")
                    };
                    assert_eq!(draw.visible_rows, 2);
                    assert!(menu.presented(draw));
                    let text = terminal
                        .backend()
                        .buffer()
                        .content
                        .iter()
                        .map(|cell| cell.symbol())
                        .collect::<String>();
                    assert_eq!(text.contains("QZX"), !context);
                    assert_eq!(text.contains("Esc"), !context);
                    assert_eq!(text.contains("1/2"), !context);
                    let map = hits.finish();
                    let has_chrome_control = (0..height).any(|y| {
                        (0..width).any(|x| {
                            matches!(
                                map.hit_target_at(x, y),
                                Some(HitTarget::ActionMenuControl { .. })
                            )
                        })
                    });
                    assert_eq!(has_chrome_control, !context);
                    let row = action_row_rect(&map, terminal.backend().buffer().area, draw.token);
                    assert_eq!(row.y, 3, "the first row follows the anchored top border");
                    menu.key(crate::client_view::KeyEvent::press(
                        KeyCode::Esc,
                        Modifiers::NONE,
                    ));
                    assert!(
                        !menu.active(),
                        "removing chrome preserves local Esc cancellation"
                    );
                }
            }
        }
    }
}

#[test]
fn context_menu_admission_requires_one_action_and_its_reserved_reason_without_footer_rows() {
    for width in [15, 16, 17] {
        for height in [2, 3, 4] {
            for unavailable in [false, true] {
                let client = ClientView::new(width);
                let mut menu = ActionMenu::default();
                assert!(menu.open_for(
                    crate::action_menu::MenuTarget::Create(None),
                    "QZX".into(),
                    vec![MenuAction::Help],
                    None,
                ));
                menu.refresh_availability(|_| {
                    unavailable.then_some(crate::action_menu::MenuUnavailable::OperationBusy)
                });
                let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                let mut hits = HitMapBuilder::new();
                let mut presentation = ModalPresentation::NotPresented;
                terminal
                    .draw(|frame| {
                        presentation = render(
                            frame,
                            frame.area(),
                            menu.view().unwrap(),
                            client.bindings(),
                            Locale::EnUs,
                            &Theme::terminal_native(),
                            &mut hits,
                        );
                    })
                    .unwrap();
                let expected = width >= 16 && height >= if unavailable { 4 } else { 3 };
                assert_eq!(
                    matches!(presentation, ModalPresentation::Presented(_)),
                    expected,
                    "{width}x{height}, unavailable={unavailable}"
                );
                let map = hits.finish();
                let has_row = (0..height).any(|y| {
                    (0..width).any(|x| {
                        matches!(
                            map.hit_target_at(x, y),
                            Some(HitTarget::ActionMenuRow { .. })
                        )
                    })
                });
                assert_eq!(
                    has_row, expected,
                    "refused geometry publishes no row authority"
                );
            }
        }
    }
}

#[test]
fn hovered_candidate_uses_the_full_row_selection_and_marker_in_every_usable_tier() {
    for (width, height) in [(120, 40), (80, 24), (65, 16), (40, 10)] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for theme in [
                Theme::terminal_native(),
                Theme::one_light(),
                Theme::one_dark(),
            ] {
                let client = ClientView::new(width);
                let mut menu = ActionMenu::default();
                menu.open(
                    "Actions".into(),
                    vec![MenuAction::Help, MenuAction::Reload],
                    None,
                );
                let paint = |menu: &mut ActionMenu| {
                    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                    let mut hits = HitMapBuilder::new();
                    let mut draw = ModalPresentation::NotPresented;
                    let buffer = terminal
                        .draw(|frame| {
                            draw = render(
                                frame,
                                frame.area(),
                                menu.view().unwrap(),
                                client.bindings(),
                                locale,
                                &theme,
                                &mut hits,
                            );
                        })
                        .unwrap()
                        .buffer
                        .clone();
                    let ModalPresentation::Presented(draw) = draw else {
                        panic!("menu fits the tier")
                    };
                    assert!(menu.presented(draw));
                    (buffer, hits.finish(), draw.token)
                };
                let (before, map, token) = paint(&mut menu);
                let first = action_row_rect(&map, before.area, token);
                assert!(menu.hover(token, 1));
                let (after, _, _) = paint(&mut menu);
                assert_text_at(&after, first.x, first.y + 1, theme.glyphs.navigation_marker);
                assert_eq!(after[(first.x, first.y)].symbol(), " ");
                for x in first.x..first.right() {
                    assert_eq!(
                        after[(x, first.y + 1)].bg,
                        theme.color(Role::SurfaceSelection)
                    );
                    assert_ne!(after[(x, first.y)].bg, theme.color(Role::SurfaceSelection));
                }
                assert_eq!(menu.confirmed(), None);
                assert_eq!(menu.view().unwrap().first, 0);
            }
        }
    }
}

fn render_machines_row(
    width: u16,
    height: u16,
    client: &ClientView,
    locale: Locale,
    theme: &Theme,
) -> (Buffer, FrameHitMap, MenuToken) {
    let mut menu = ActionMenu::default();
    assert!(menu.open(String::new(), vec![MenuAction::Machines], None));
    let token = menu.view().unwrap().token;
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut hits = HitMapBuilder::new();
    let mut presentation = None;
    terminal
        .draw(|frame| {
            presentation = Some(render(
                frame,
                Rect::new(0, 0, width, height),
                menu.view().unwrap(),
                client.bindings(),
                locale,
                theme,
                &mut hits,
            ));
        })
        .unwrap();
    assert!(matches!(
        presentation,
        Some(ModalPresentation::Presented(MenuDraw {
            token: drawn,
            first: 0,
            visible_rows: 1,
            selected: 0,
        })) if drawn == token
    ));
    (terminal.backend().buffer().clone(), hits.finish(), token)
}

fn action_row_rect(map: &FrameHitMap, area: Rect, token: MenuToken) -> Rect {
    let points = (area.y..area.bottom())
        .flat_map(|y| (area.x..area.right()).map(move |x| (x, y)))
        .filter(|(x, y)| {
            matches!(
                map.hit_target_at(*x, *y),
                Some(HitTarget::ActionMenuRow { token: drawn, row: 0 }) if *drawn == token
            )
        })
        .collect::<Vec<_>>();
    assert!(
        !points.is_empty(),
        "the painted action row owns a hit region"
    );
    let y = points[0].1;
    assert!(points.iter().all(|(_, candidate_y)| *candidate_y == y));
    let left = points.iter().map(|(x, _)| *x).min().unwrap();
    let right = points.iter().map(|(x, _)| *x).max().unwrap() + 1;
    let row = Rect::new(left, y, right - left, 1);
    for x in row.x..row.right() {
        assert!(matches!(
            map.hit_target_at(x, row.y),
            Some(HitTarget::ActionMenuRow { token: drawn, row: 0 }) if *drawn == token
        ));
    }
    row
}

fn assert_text_at(buffer: &Buffer, mut x: u16, y: u16, text: &str) -> u16 {
    for grapheme in Span::raw(text).styled_graphemes(Style::default()) {
        assert_eq!(buffer[(x, y)].symbol(), grapheme.symbol);
        let width = u16::try_from(display_width(grapheme.symbol)).unwrap();
        for trailing in 1..width {
            assert_eq!(buffer[(x + trailing, y)].symbol(), " ");
        }
        x += width;
    }
    x
}

fn assert_bound_row_layout(
    buffer: &Buffer,
    map: &FrameHitMap,
    token: MenuToken,
    locale: Locale,
    theme: &Theme,
    chord: &str,
    expect_truncated: bool,
) {
    let row = action_row_rect(map, buffer.area, token);
    let marker_end = assert_text_at(buffer, row.x, row.y, theme.glyphs.navigation_marker);
    let chord_width = u16::try_from(display_width(chord)).unwrap();
    let chord_x = row.right() - chord_width;
    assert_eq!(assert_text_at(buffer, chord_x, row.y, chord), row.right());
    let label_right = chord_x - ROW_BINDING_GAP;
    let label_budget = usize::from(label_right - marker_end);
    let full_label = locale.menu_action(&MenuAction::Machines);
    let label = truncate_display(full_label, label_budget, theme.glyphs.ellipsis);
    assert!(display_width(&label) <= label_budget);
    assert_eq!(display_width(full_label) > label_budget, expect_truncated);
    let label_end = assert_text_at(buffer, marker_end, row.y, &label);
    for x in label_end..chord_x {
        assert_eq!(buffer[(x, row.y)].symbol(), " ");
        assert!(matches!(
            map.hit_target_at(x, row.y),
            Some(HitTarget::ActionMenuRow { token: drawn, row: 0 }) if *drawn == token
        ));
    }
    assert_eq!(chord_x - label_right, ROW_BINDING_GAP);
    assert!(chord_x - label_end >= ROW_BINDING_GAP);
}

fn configured_prefix(
    key: &str,
    ctrl: bool,
    alt: bool,
    shift: bool,
    super_key: bool,
) -> WindowsKeySettings {
    WindowsKeySettings {
        commands: Vec::new(),
        prefix: WindowsKeyChord {
            key: key.into(),
            ctrl,
            alt,
            shift,
            super_key,
        },
        overrides: Vec::new(),
    }
}

#[test]
fn rejected_menu_paints_nothing_and_does_not_claim_the_pointer_backdrop() {
    let mut menu = ActionMenu::default();
    assert!(menu.open("Actions".into(), vec![MenuAction::Help], None));
    let client = ClientView::new(16);
    let theme = Theme::terminal_native();
    let mut hits = HitMapBuilder::new();
    hits.record(
        HitLayer::Foreground,
        Rect::new(0, 0, 1, 1),
        HitTarget::MenuButton,
    );
    let mut terminal = Terminal::new(TestBackend::new(16, 5)).unwrap();

    // This passes the outer size floor, then fails the required exit-hint admission.
    terminal
        .draw(|frame| {
            frame
                .buffer_mut()
                .set_string(0, 0, "underlay remains", Style::default());
            let before = frame.buffer_mut().clone();
            assert_eq!(
                render(
                    frame,
                    Rect::new(0, 0, 16, 5),
                    menu.view().unwrap(),
                    client.bindings(),
                    Locale::EnUs,
                    &theme,
                    &mut hits,
                ),
                ModalPresentation::NotPresented
            );
            assert_eq!(
                *frame.buffer_mut(),
                before,
                "a rejected menu must not mutate the candidate frame"
            );
        })
        .unwrap();

    let map = hits.finish();
    assert!(matches!(
        map.hit_target_at(0, 0),
        Some(HitTarget::MenuButton)
    ));
}

#[test]
fn wide_english_row_keeps_the_full_label_and_effective_chord_in_separate_slots() {
    let width = 120;
    let height = 24;
    let client = ClientView::new(width);
    let theme = Theme::one_dark();
    let chord = client
        .bindings()
        .action_labels_with_glyphs(
            PrefixAction::Client(ClientAction::OpenMachines),
            key_label_glyphs(&theme),
        )
        .into_iter()
        .next()
        .unwrap();
    assert_eq!(chord, "Ctrl+b Shift+M");
    let (buffer, map, token) = render_machines_row(width, height, &client, Locale::EnUs, &theme);

    assert_bound_row_layout(&buffer, &map, token, Locale::EnUs, &theme, &chord, false);
    assert_ne!(
        theme.color(Role::SurfaceOverlay),
        theme.color(Role::SurfaceChrome)
    );
    let close_row = (0..height)
        .find(|&y| {
            (0..width).any(|x| {
                matches!(
                    map.hit_target_at(x, y),
                    Some(HitTarget::ActionMenuControl { token: drawn, back: false })
                        if *drawn == token
                )
            })
        })
        .expect("top close target");
    let close_cells = (0..width)
        .filter(|&x| {
            matches!(
                map.hit_target_at(x, close_row),
                Some(HitTarget::ActionMenuControl { token: drawn, back: false })
                    if *drawn == token
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(close_cells.len(), 3, "fixed [x] close extent");
    assert!(
        close_cells
            .iter()
            .all(|&x| { buffer[(x, close_row)].bg == theme.color(Role::SurfaceOverlay) })
    );
}

#[test]
fn compact_chinese_row_truncates_only_the_label_and_keeps_a_custom_chord_whole() {
    let width = 60;
    let height = 12;
    let mut client = ClientView::new(width);
    client.configure_keys(&configured_prefix("pagedown", true, true, true, true));
    let theme = Theme::terminal_native();
    let chord = client
        .bindings()
        .action_labels_with_glyphs(
            PrefixAction::Client(ClientAction::OpenMachines),
            key_label_glyphs(&theme),
        )
        .into_iter()
        .next()
        .unwrap();
    assert_eq!(chord, "Ctrl+Alt+Shift+Super+PageDown Shift+M");
    let (buffer, map, token) = render_machines_row(width, height, &client, Locale::ZhCn, &theme);

    assert_bound_row_layout(&buffer, &map, token, Locale::ZhCn, &theme, &chord, true);
}

#[test]
fn minimal_row_omits_an_unfit_chord_as_one_unit_without_shrinking_its_hit_geometry() {
    let width = 26;
    let height = 8;
    let mut client = ClientView::new(width);
    client.configure_keys(&configured_prefix("pagedown", true, true, true, true));
    let theme = Theme::terminal_native();
    let chord = "Ctrl+Alt+Shift+Super+PageDown Shift+M";
    let (buffer, map, token) = render_machines_row(width, height, &client, Locale::ZhCn, &theme);
    let row = action_row_rect(&map, buffer.area, token);
    let text = (row.x..row.right())
        .map(|x| buffer[(x, row.y)].symbol())
        .collect::<String>();

    assert!(!text.contains(chord));
    assert!(
        !text.contains("PageDown"),
        "no clipped chord fragment: {text:?}"
    );
    assert_text_at(&buffer, row.x, row.y, theme.glyphs.navigation_marker);
}
