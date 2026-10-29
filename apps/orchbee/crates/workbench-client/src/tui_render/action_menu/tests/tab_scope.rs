use super::*;
use crate::action_menu::MenuTarget;
use crate::tui_theme::ColorLevel;
use orchbee_workbench_protocol::{TabGroupColor, WorkbenchTabId};
use ratatui::style::Modifier;
use std::num::NonZeroUsize;

fn paint(
    menu: &mut ActionMenu,
    width: u16,
    height: u16,
    locale: Locale,
    theme: &Theme,
) -> (Buffer, FrameHitMap) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut hits = HitMapBuilder::new();
    let mut draw = ModalPresentation::NotPresented;
    terminal
        .draw(|frame| {
            draw = render(
                frame,
                frame.area(),
                menu.view().unwrap(),
                &[],
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
                                theme.glyphs.checked_marker,
                            );
                        } else {
                            assert_eq!(buffer[(navigation_end, rect.y)].symbol(), " ");
                        }
                        let swatch = navigation_end
                            + u16::try_from(display_width(theme.glyphs.checked_marker)).unwrap()
                            + 1;
                        for x in swatch..swatch + 2 {
                            assert_eq!(buffer[(x, rect.y)].bg, theme.tab_groups.accent(*color));
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
fn typed_single_and_selected_scopes_paint_explicit_rows_and_title_the_tab() {
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
                assert!(menu.open_for(MenuTarget::Tab(id), "Build".into(), actions.clone(), None));
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
                // The scope lives in the row labels; the frame's title names only the Tab.
                let first = row(&hits, buffer.area, menu.view().unwrap().token, 0);
                let border = (first.x..first.right())
                    .map(|x| buffer[(x, first.y - 1)].symbol())
                    .collect::<String>();
                assert!(border.contains("Build"), "{border}");
                assert!(!border.contains(locale.menu_title()), "{border}");
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
                assert_eq!(title, "Build");
                menu.cancel();
            }
        }
    }
}

#[test]
fn group_title_drops_its_tab_count_before_truncating_the_name() {
    let theme = Theme::rgb_test_dark();
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let mut menu = ActionMenu::default();
        assert!(menu.open_for(
            MenuTarget::TabGroups {
                tab_id: WorkbenchTabId::parse(&format!("{:032x}", 35)).unwrap(),
                colors: false
            },
            "后端服务".into(),
            vec![MenuAction::RenameGroup],
            None
        ));
        let scope = TabMenuScope::Group(NonZeroUsize::new(1).unwrap());
        menu.set_tab_presentation(scope, None, |_| Some(scope));
        let view = menu.view().unwrap();
        let count = match locale {
            Locale::EnUs => "Group (1 Tab)",
            Locale::ZhCn => "分组（1 个标签页）",
        };
        let whole = format!("后端服务 {} {count}", theme.glyphs.middle_dot);
        let fits = display_width(&whole);
        assert_eq!(tab_title(&view, locale, &theme, fits), whole);
        // One cell short drops the count whole; the name truncates only once it alone overflows.
        for width in [fits - 1, 8] {
            assert_eq!(tab_title(&view, locale, &theme, width), "后端服务");
        }
        assert_eq!(tab_title(&view, locale, &theme, 5), "后端…");
    }
}
