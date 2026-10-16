use super::*;
use crate::action_menu::MenuTarget;
use crate::terminal_input::{MouseAction, MouseButton, MouseEvent, UnavailableTerminalInputPort};
use crate::tui_pointer::{PointerOutcome, PointerState};
use crate::tui_theme::ColorLevel;
use orchbee_workbench_protocol::{TabGroupColor, WorkbenchTabId};
use std::num::NonZeroUsize;

fn paint(
    menu: &mut ActionMenu,
    width: u16,
    height: u16,
    locale: Locale,
    theme: &Theme,
) -> (Buffer, FrameHitMap) {
    paint_with_parent(menu, width, height, locale, theme, false)
}

fn paint_with_parent(
    menu: &mut ActionMenu,
    width: u16,
    height: u16,
    locale: Locale,
    theme: &Theme,
    has_parent: bool,
) -> (Buffer, FrameHitMap) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut hits = HitMapBuilder::new();
    let mut draw = ModalPresentation::NotPresented;
    terminal
        .draw(|frame| {
            let mut view = menu.view().unwrap();
            view.has_parent = has_parent;
            draw = render(
                frame,
                frame.area(),
                view,
                ClientView::new(width).bindings(),
                locale,
                theme,
                &mut hits,
            );
        })
        .unwrap();
    let ModalPresentation::Presented(draw) = draw else {
        panic!("the complete scoped menu fits")
    };
    assert!(menu.presented(draw));
    (terminal.backend().buffer().clone(), hits.finish())
}

fn row(map: &FrameHitMap, area: Rect, token: MenuToken, index: usize) -> Rect {
    let points: Vec<_> = (area.y..area.bottom()).flat_map(|y| (area.x..area.right()).map(move |x| (x, y))).filter(|(x, y)| matches!(map.hit_target_at(*x, *y), Some(HitTarget::ActionMenuRow { token: drawn, row }) if *drawn == token && *row == index)).collect();
    let first = points
        .first()
        .expect("painted row has an exact input region");
    assert!(points.iter().all(|point| point.1 == first.1));
    Rect::new(first.0, first.1, u16::try_from(points.len()).unwrap(), 1)
}

#[test]
fn applied_color_check_and_paired_palette_chip_survive_a_different_provisional_candidate() {
    for (width, height) in [(80, 24), (120, 40)] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for base in [
                Theme::terminal_native(),
                Theme::one_light(),
                Theme::one_dark(),
            ] {
                for level in [
                    ColorLevel::Truecolor,
                    ColorLevel::Ansi256,
                    ColorLevel::Ansi16,
                    ColorLevel::Monochrome,
                ] {
                    let theme = base.quantize(level);
                    let colors = [
                        TabGroupColor::Blue,
                        TabGroupColor::Cyan,
                        TabGroupColor::Green,
                        TabGroupColor::Grey,
                        TabGroupColor::Orange,
                        TabGroupColor::Pink,
                        TabGroupColor::Purple,
                        TabGroupColor::Red,
                        TabGroupColor::Yellow,
                    ];
                    let mut menu = ActionMenu::default();
                    assert!(menu.open_for(
                        MenuTarget::TabGroups {
                            tab_id: WorkbenchTabId::parse(&format!("{:032x}", 35)).unwrap(),
                            colors: true
                        },
                        "Build group".into(),
                        colors.into_iter().map(MenuAction::GroupColor).collect(),
                        None
                    ));
                    let scope = TabMenuScope::Group(NonZeroUsize::new(2).unwrap());
                    menu.set_tab_presentation(scope, Some(TabGroupColor::Red), |_| Some(scope));
                    menu.select_action(MenuAction::GroupColor(TabGroupColor::Red));
                    paint(&mut menu, width, height, locale, &theme);
                    let token = menu.view().unwrap().token;
                    assert!(menu.hover(token, 0));
                    let (buffer, hits) = paint(&mut menu, width, height, locale, &theme);
                    assert_eq!(menu.view().unwrap().selected, 0);
                    assert!(
                        menu.view()
                            .unwrap()
                            .tabs
                            .unwrap()
                            .checked(&MenuAction::GroupColor(TabGroupColor::Red))
                    );
                    assert!(
                        !menu
                            .view()
                            .unwrap()
                            .tabs
                            .unwrap()
                            .checked(&MenuAction::GroupColor(TabGroupColor::Blue))
                    );
                    for (index, color) in colors.iter().enumerate() {
                        let rect = row(&hits, buffer.area, token, index);
                        let navigation_end = rect.x
                            + u16::try_from(display_width(theme.glyphs.navigation_marker)).unwrap();
                        if index == 0 {
                            assert_text_at(&buffer, rect.x, rect.y, theme.glyphs.navigation_marker);
                        } else {
                            assert_eq!(buffer[(rect.x, rect.y)].symbol(), " ");
                        }
                        if index == 7 {
                            assert_text_at(
                                &buffer,
                                navigation_end,
                                rect.y,
                                theme.glyphs.active_marker,
                            );
                        } else {
                            assert_eq!(buffer[(navigation_end, rect.y)].symbol(), " ");
                        }
                        let swatch = navigation_end
                            + u16::try_from(display_width(theme.glyphs.active_marker)).unwrap()
                            + 1;
                        for x in swatch..swatch + 2 {
                            assert_eq!(
                                buffer[(x, rect.y)].bg,
                                theme.tab_group_style(*color).bg.unwrap()
                            );
                            assert!(
                                !buffer[(x, rect.y)]
                                    .modifier
                                    .intersects(Modifier::REVERSED | Modifier::HIDDEN),
                                "candidate surfaces cannot reverse or hide the palette chip"
                            );
                        }
                        assert_text_at(
                            &buffer,
                            swatch + 3,
                            rect.y,
                            locale.menu_action(&MenuAction::GroupColor(*color)),
                        );
                        for x in rect.x..rect.right() {
                            assert!(
                                matches!(hits.hit_target_at(x, rect.y), Some(HitTarget::ActionMenuRow { token: drawn, row }) if *drawn == token && *row == index)
                            );
                        }
                    }
                    assert!(
                        menu.confirmed().is_none(),
                        "showing or hovering a palette never applies it"
                    );
                    menu.cancel();
                    assert!(menu.take().is_none());
                }
            }
        }
    }
}

#[test]
fn typed_single_and_selected_scopes_paint_explicit_rows_and_keep_name_and_child_path() {
    for (width, height) in [(80, 24), (120, 40)] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for selected in [false, true] {
                let scope = if selected {
                    TabMenuScope::Selected(NonZeroUsize::new(2).unwrap())
                } else {
                    TabMenuScope::Addressed
                };
                let id = WorkbenchTabId::parse(&format!("{:032x}", 35)).unwrap();
                let mut menu = ActionMenu::default();
                let actions = vec![
                    MenuAction::NewTabToRight,
                    MenuAction::RenameTab,
                    MenuAction::PinTab,
                    MenuAction::CloseSelectedTabs,
                ];
                assert!(menu.open_for(
                    MenuTarget::Tab(id.clone()),
                    "Build".into(),
                    actions.clone(),
                    None
                ));
                menu.set_tab_presentation(scope, None, |action| {
                    Some(
                        if matches!(action, MenuAction::NewTabToRight | MenuAction::RenameTab) {
                            TabMenuScope::Addressed
                        } else {
                            scope
                        },
                    )
                });
                let theme = Theme::terminal_native();
                let (buffer, hits) = paint(&mut menu, width, height, locale, &theme);
                for (index, action) in actions.iter().enumerate() {
                    let rect = row(&hits, buffer.area, menu.view().unwrap().token, index);
                    let actual = menu.view().unwrap().tabs.unwrap().rows[index];
                    assert_text_at(
                        &buffer,
                        rect.x + 1,
                        rect.y,
                        &locale.menu_tab_action(action, actual),
                    );
                }
                let expected_pin = match (locale, selected) {
                    (Locale::EnUs, false) => "Pin Tab",
                    (Locale::EnUs, true) => "Pin selected 2 Tabs",
                    (Locale::ZhCn, false) => "固定标签页",
                    (Locale::ZhCn, true) => "固定所选 2 个标签页",
                };
                let pin = row(&hits, buffer.area, menu.view().unwrap().token, 2);
                assert_text_at(&buffer, pin.x + 1, pin.y, expected_pin);
                let expected_close = match (locale, selected) {
                    (Locale::EnUs, false) => "Close Tab…",
                    (Locale::EnUs, true) => "Close selected 2 Tabs…",
                    (Locale::ZhCn, false) => "关闭标签页…",
                    (Locale::ZhCn, true) => "关闭所选 2 个标签页…",
                };
                let close = row(&hits, buffer.area, menu.view().unwrap().token, 3);
                assert_text_at(&buffer, close.x + 1, close.y, expected_close);
                assert_eq!(
                    buffer[(close.x + 1, close.y)].fg,
                    theme.style(Role::AccentDestructive).fg.unwrap(),
                    "Close uses intent tone rather than an error-status role"
                );
                let header = tab_title(
                    &menu.view().unwrap(),
                    locale,
                    &theme,
                    usize::from(pin.width - 2),
                );
                let first = row(&hits, buffer.area, menu.view().unwrap().token, 0);
                assert_text_at(&buffer, first.x + 1, first.y - 1, &header);
                assert_eq!(
                    locale.menu_tab_action(&MenuAction::RenameTab, scope),
                    locale.menu_action(&MenuAction::RenameTab)
                );
                assert_ne!(
                    locale.menu_tab_action(
                        &MenuAction::PinTab,
                        TabMenuScope::Selected(NonZeroUsize::new(2).unwrap())
                    ),
                    locale.menu_tab_action(&MenuAction::PinTab, TabMenuScope::Addressed)
                );
                let title = tab_title(&menu.view().unwrap(), locale, &theme, 40);
                assert!(title.contains("Build"));
                menu.cancel();
                assert!(menu.open_for(
                    MenuTarget::TabGroupChoices(id),
                    "Build".into(),
                    vec![MenuAction::CreateGroup],
                    None
                ));
                menu.set_tab_presentation(scope, None, |_| Some(scope));
                let title = tab_title(&menu.view().unwrap(), locale, &theme, 40);
                assert!(title.contains("Build"));
                assert!(title.contains(&locale.menu_tab_path(
                    menu.view().unwrap().source,
                    scope,
                    theme.glyphs.disclosure_closed
                )));
                let title = tab_title(&menu.view().unwrap(), locale, &theme, 20);
                assert!(
                    display_width(&title) <= 20,
                    "semantic path obeys the same cell bound"
                );
            }
        }
    }
}

#[test]
fn tab_children_paint_only_their_measured_back_action_and_semantic_path_above_rows() {
    for (width, height) in [(80, 24), (120, 40)] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for has_parent in [false, true] {
                let theme = Theme::terminal_native();
                let mut menu = ActionMenu::default();
                let source = MenuTarget::TabGroups {
                    tab_id: WorkbenchTabId::parse(&format!("{:032x}", 35)).unwrap(),
                    colors: true,
                };
                assert!(menu.open_for(
                    source.clone(),
                    "Build".into(),
                    vec![MenuAction::GroupColor(TabGroupColor::Red)],
                    None,
                ));
                let scope = TabMenuScope::Group(NonZeroUsize::new(2).unwrap());
                menu.set_tab_presentation(scope, Some(TabGroupColor::Red), |_| Some(scope));
                let (buffer, hits) =
                    paint_with_parent(&mut menu, width, height, locale, &theme, has_parent);
                let token = menu.view().unwrap().token;
                let action = row(&hits, buffer.area, token, 0);
                let controls: Vec<_> = (0..height)
                    .flat_map(|y| (0..width).map(move |x| (x, y)))
                    .filter(|(x, y)| {
                        matches!(
                            hits.hit_target_at(*x, *y),
                            Some(HitTarget::ActionMenuControl { .. })
                        )
                    })
                    .collect();
                assert_eq!(!controls.is_empty(), has_parent);
                let label_x = if has_parent {
                    let &(left, y) = controls.first().unwrap();
                    let right = controls.iter().map(|(x, _)| *x).max().unwrap() + 1;
                    for (x, row) in &controls {
                        assert_eq!(*row, action.y - 1);
                        assert!(matches!(
                            hits.hit_target_at(*x, *row),
                            Some(HitTarget::ActionMenuControl { token: t, back: true })
                                if *t == token
                        ));
                        // The full source identity belongs to the existing generic interaction
                        // record. Exercise its public release path rather than inspect internals.
                        let mut pointer = PointerState::default();
                        pointer.set_capture(true);
                        pointer.publish(hits.clone());
                        let mut port = UnavailableTerminalInputPort;
                        let event = |action| MouseEvent {
                            action,
                            button: MouseButton::Left,
                            mods: Modifiers::NONE,
                            column: *x,
                            row: *row,
                        };
                        assert_eq!(
                            pointer.action_menu_event(&mut port, event(MouseAction::Press)),
                            PointerOutcome::Handled,
                        );
                        assert_eq!(
                            pointer.action_menu_event(&mut port, event(MouseAction::Release)),
                            PointerOutcome::Interaction {
                                target: InteractionTarget::ActionMenuControl {
                                    source: source.clone(),
                                    token,
                                    control: MenuControl::Back,
                                },
                                contract: PrimaryContract::ActivateOnRelease,
                            },
                        );
                    }
                    let focus = u16::try_from(display_width(theme.glyphs.active_marker)).unwrap();
                    assert_text_at(
                        &buffer,
                        left + focus,
                        y,
                        &format!("[ {}  Esc ]", locale.menu_back_button(),),
                    );
                    right + 1
                } else {
                    action.x + 1
                };
                let path = locale.menu_tab_path(&source, scope, theme.glyphs.disclosure_closed);
                assert_text_at(
                    &buffer,
                    label_x,
                    action.y - 1,
                    &format!("Build {} {path}", theme.glyphs.middle_dot),
                );
                let text = buffer
                    .content
                    .iter()
                    .map(|cell| cell.symbol())
                    .collect::<String>();
                assert!(!text.contains("1/1"), "a child has no generic counter");
                assert!(!text.contains(locale.menu_title()), "no generic menu title");
                assert!(menu.confirmed().is_none());
            }
        }
    }
}

#[test]
fn tab_header_back_path_reason_and_one_action_must_all_fit_before_paint_or_hits() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for (width, height, reason, fits) in [
            (24, 8, false, false),
            (80, 3, false, false),
            (80, 4, false, true),
            (80, 4, true, false),
            (80, 5, true, true),
        ] {
            let mut menu = ActionMenu::default();
            assert!(menu.open_for(
                MenuTarget::TabGroups {
                    tab_id: WorkbenchTabId::parse(&format!("{:032x}", 35)).unwrap(),
                    colors: true,
                },
                "Build".into(),
                vec![MenuAction::GroupColor(TabGroupColor::Red)],
                None,
            ));
            let scope = TabMenuScope::Group(NonZeroUsize::new(2).unwrap());
            menu.set_tab_presentation(scope, Some(TabGroupColor::Red), |_| Some(scope));
            menu.refresh_availability(|_| {
                reason.then_some(crate::action_menu::MenuUnavailable::OperationBusy)
            });
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            let mut hits = HitMapBuilder::new();
            terminal
                .draw(|frame| {
                    frame.render_widget(Paragraph::new("underlay"), frame.area());
                    let before = frame.buffer_mut().clone();
                    let mut view = menu.view().unwrap();
                    view.has_parent = true;
                    let result = render(
                        frame,
                        frame.area(),
                        view,
                        ClientView::new(width).bindings(),
                        locale,
                        &Theme::terminal_native(),
                        &mut hits,
                    );
                    assert_eq!(
                        matches!(result, ModalPresentation::Presented(_)),
                        fits,
                        "{locale:?} {width}x{height} reason={reason}"
                    );
                    if !fits {
                        assert_eq!(
                            &*frame.buffer_mut(),
                            &before,
                            "refusal cannot paint a partial child"
                        );
                    }
                })
                .unwrap();
            let map = hits.finish();
            let has_menu = (0..height).any(|y| {
                (0..width).any(|x| {
                    matches!(
                        map.hit_target_at(x, y),
                        Some(HitTarget::ActionMenuRow { .. } | HitTarget::ActionMenuControl { .. })
                    )
                })
            });
            assert_eq!(
                has_menu, fits,
                "no hidden Back or row target survives refusal"
            );
        }
    }
}
