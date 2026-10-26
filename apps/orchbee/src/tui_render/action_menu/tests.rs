use super::*;
use crate::{
    action_menu::{ActionMenu, MenuToken},
    client_view::{ClientAction, ClientView, KeyEvent, PrefixAction},
    tui_pointer::{FrameHitMap, HitLayer, HitTarget},
};
use orchbee_local_site_runtime::{WindowsKeyChord, WindowsKeySettings};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, style::Style, text::Span};

mod sidebar;
mod tab_scope;

#[test]
fn global_search_uses_localized_aliases_effective_chords_and_nested_labels() {
    use crate::action_menu::{MenuSearchChild, MenuTarget};
    use orchbee_workbench_protocol::{UserLaunchCommand, UserLaunchProfile};

    let profile = MenuAction::NewTabWithProfile(Box::new(UserLaunchProfile {
        id: "search-profile".into(),
        name: Some("PowerShell preview".into()),
        command: UserLaunchCommand::CustomExecutable {
            executable: r"C:\test\pwsh.exe".into(),
            arguments: Vec::new(),
        },
    }));
    let mut menu = ActionMenu::default();
    assert!(menu.open_for(
        MenuTarget::Global,
        String::new(),
        vec![
            MenuAction::SplitPanel(crate::client_view::PanelDirection::Up),
            MenuAction::Help,
            MenuAction::TabCreation,
        ],
        None,
    ));
    menu.set_search_children(vec![MenuSearchChild {
        parent: MenuAction::TabCreation,
        source: MenuTarget::Global,
        actions: vec![profile],
        labels: vec![None],
    }]);
    let client = ClientView::new(80);
    let theme = Theme::terminal_native();
    let glyphs = key_label_glyphs(&theme);

    for (locale, query) in [(Locale::EnUs, "split"), (Locale::ZhCn, "分屏")] {
        menu.set_query(Some(query));
        let view = menu.view().unwrap();
        let bindings = client.bindings();
        let binding_labels = view
            .actions
            .iter()
            .map(|action| effective_binding(bindings, &view, action, glyphs))
            .collect::<Vec<_>>();
        assert_eq!(
            search_result_actions(&view, bindings, locale, glyphs, &binding_labels),
            [0],
            "{locale:?} semantic alias"
        );
    }

    menu.set_query(Some("powershell"));
    let view = menu.view().unwrap();
    let bindings = client.bindings();
    let binding_labels = view
        .actions
        .iter()
        .map(|action| effective_binding(bindings, &view, action, glyphs))
        .collect::<Vec<_>>();
    assert_eq!(
        search_result_actions(&view, bindings, Locale::EnUs, glyphs, &binding_labels),
        [2],
        "a matching nested action keeps its owning TabCreation row"
    );

    let chord = effective_binding(bindings, &view, &MenuAction::Help, glyphs)
        .expect("Help has its existing effective shortcut");
    menu.set_query(Some(&chord));
    let view = menu.view().unwrap();
    let binding_labels = view
        .actions
        .iter()
        .map(|action| effective_binding(bindings, &view, action, glyphs))
        .collect::<Vec<_>>();
    assert_eq!(
        search_result_actions(&view, bindings, Locale::EnUs, glyphs, &binding_labels),
        [1],
        "the same displayed effective chord is searchable"
    );
}

#[test]
fn restored_filtered_selection_is_visible_on_the_first_small_frame() {
    use crate::action_menu::MenuTarget;

    let mut menu = ActionMenu::default();
    assert!(menu.open_for(
        MenuTarget::Global,
        String::new(),
        vec![
            MenuAction::Help,
            MenuAction::Detach,
            MenuAction::ClosePane,
            MenuAction::Zoom,
            MenuAction::RenameTab,
            MenuAction::Reload,
            MenuAction::Settings,
            MenuAction::AgentBoard,
        ],
        None,
    ));
    menu.set_query(Some("e"));
    let selected_action = {
        let view = menu.view().unwrap();
        let client = ClientView::new(80);
        let theme = Theme::terminal_native();
        let glyphs = key_label_glyphs(&theme);
        let binding_labels = view
            .actions
            .iter()
            .map(|action| effective_binding(client.bindings(), &view, action, glyphs))
            .collect::<Vec<_>>();
        let results = search_result_actions(
            &view,
            client.bindings(),
            Locale::EnUs,
            glyphs,
            &binding_labels,
        );
        assert!(
            results.len() >= 5,
            "the selected result is below the first page"
        );
        view.actions[*results.last().unwrap()].clone()
    };
    menu.select_action(selected_action.clone());

    let client = ClientView::new(80);
    let theme = Theme::terminal_native();
    let width = 80;
    let height = 8;
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
                &theme,
                &mut hits,
            );
        })
        .unwrap();
    let ModalPresentation::Presented(draw) = presentation else {
        panic!("the compact global menu fits");
    };
    assert!(draw.selected >= 4);
    assert!(draw.first > 0);
    assert!((draw.first..draw.first.saturating_add(draw.visible_rows)).contains(&draw.selected));
    let map = hits.finish();
    let buffer = terminal.backend().buffer();
    let _row = action_row_rect_for(&map, buffer.area, draw.token, draw.selected);
    assert!(menu.presented(draw));
    assert_eq!(
        menu.input_context().unwrap().readiness(),
        crate::action_menu::MenuInputReadiness::ActivationReady
    );
    assert!(menu.confirm());
    assert_eq!(menu.confirmed(), Some(selected_action));
}

#[test]
fn no_results_keeps_a_drawn_query_surface_without_an_activatable_row() {
    use crate::action_menu::MenuTarget;

    let mut menu = ActionMenu::default();
    assert!(menu.open_for(
        MenuTarget::Global,
        String::new(),
        vec![MenuAction::Help],
        None
    ));
    menu.set_query(Some("no-such-action"));
    let client = ClientView::new(80);
    let theme = Theme::terminal_native();
    let width = 80;
    let height = 24;
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
                &theme,
                &mut hits,
            );
        })
        .unwrap();
    let ModalPresentation::Presented(draw) = presentation else {
        panic!("the empty result still paints its query and explanation");
    };
    let token = draw.token;
    assert!(draw.result_actions.is_empty());
    assert!(menu.presented(draw.clone()));
    assert!(!menu.confirm());
    assert_eq!(
        menu.input_context().unwrap().readiness(),
        crate::action_menu::MenuInputReadiness::Continuation
    );
    let text = visible_buffer_text(terminal.backend().buffer());
    assert!(text.contains(Locale::EnUs.menu_search_no_results()));
    let count = Locale::EnUs.menu_search_count(0);
    assert!(text.contains(count.as_str()));
    let map = hits.finish();
    assert!((0..height).any(|y| {
        (0..width).any(|x| map.hit_target_at(x, y) == Some(&HitTarget::OverlaySurface))
    }));
    assert!((0..height).all(|y| {
        (0..width).all(|x| {
            !matches!(
                map.hit_target_at(x, y),
                Some(HitTarget::ActionMenuRow { .. })
            )
        })
    }));
    assert!(menu.view().is_some_and(|view| view.token == token));
    menu.key(KeyEvent::press(
        KeyCode::Esc,
        crate::client_view::Modifiers::NONE,
    ));
    assert!(
        !menu.active(),
        "Escape closes an empty search without activation"
    );
    assert!(menu.confirmed().is_none());
}

#[test]
fn object_rename_rows_use_short_localized_copy_for_their_exact_actions() {
    use crate::{action_menu::MenuTarget, workbench_client::NativeContextLocation};
    use orchbee_workbench_protocol::{
        WorkbenchProjectId, WorkbenchTabId, WorkbenchTerminalContextId, WorkbenchWorkspaceId,
    };

    let tab = WorkbenchTabId::parse(&format!("{:032x}", 1)).unwrap();
    let cases = [
        (
            MenuTarget::Workspace(NativeContextLocation::Workspace {
                workspace_id: WorkbenchWorkspaceId::parse(&format!("{:032x}", 2)).unwrap(),
                terminal_context_id: WorkbenchTerminalContextId::parse(&format!("{:032x}", 3))
                    .unwrap(),
            }),
            MenuAction::RenameWorkspace,
        ),
        (
            MenuTarget::Project(WorkbenchProjectId::parse(&format!("{:032x}", 4)).unwrap()),
            MenuAction::RenameProject,
        ),
        (MenuTarget::Tab(tab.clone()), MenuAction::RenameTab),
        (
            MenuTarget::TabGroups {
                tab_id: tab,
                colors: false,
            },
            MenuAction::RenameGroup,
        ),
    ];
    for (target, action) in cases {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let expected = locale.menu_action(&action).replace(' ', "");
            for theme in [Theme::terminal_native(), Theme::one_light()] {
                for (width, height) in [(120, 40), (80, 24), (60, 12), (50, 10), (15, 4)] {
                    let mut menu = ActionMenu::default();
                    assert!(menu.open_for(
                        target.clone(),
                        "Context".into(),
                        vec![action.clone()],
                        None
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
                                ClientView::new(width).bindings(),
                                locale,
                                &theme,
                                &mut hits,
                            );
                        })
                        .unwrap();
                    let map = hits.finish();
                    if width < 16 {
                        assert_eq!(presentation, ModalPresentation::NotPresented);
                        assert!(
                            (0..height)
                                .all(|y| (0..width).all(|x| map.hit_target_at(x, y).is_none()))
                        );
                        continue;
                    }
                    let ModalPresentation::Presented(draw) = presentation else {
                        panic!("one rename row fits the context menu");
                    };
                    assert!(menu.presented(draw.clone()));
                    let buffer = terminal.backend().buffer();
                    let row = action_row_rect(&map, buffer.area, draw.token);
                    let text: String = (row.x..row.right())
                        .map(|x| buffer[(x, row.y)].symbol())
                        .collect();
                    let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
                    assert!(compact.contains(&expected), "{text}");
                    assert_eq!(menu.view().unwrap().actions, std::slice::from_ref(&action));
                }
            }
        }
    }
}

#[test]
fn saved_launch_rows_localize_names_and_keep_existing_menu_geometry_and_cancel() {
    use orchbee_workbench_protocol::{UserLaunchCommand, UserLaunchProfile, UserLaunchProfiles};
    let custom = UserLaunchProfile {
        id: "literal".into(),
        name: Some("PowerShell 中文\u{1b}[31m".into()),
        command: UserLaunchCommand::CustomExecutable {
            executable: r"C:\test.exe".into(),
            arguments: vec!["a b".into()],
        },
    };
    for locale in [Locale::EnUs, Locale::ZhCn] {
        assert_eq!(
            action_label(
                locale,
                &MenuAction::NewTabWithProfile(Box::new(custom.clone())),
                None,
                None,
                "·",
                ">"
            ),
            format!(
                "{} · PowerShell 中文",
                locale.menu_action(&MenuAction::NewTab)
            )
        );
        for (width, height) in [(120, 40), (80, 24), (72, 18), (48, 10), (14, 4)] {
            for theme in [Theme::terminal_native(), Theme::one_light()] {
                let client = ClientView::new(width);
                let mut menu = ActionMenu::default();
                assert!(menu.open_for(
                    crate::action_menu::MenuTarget::Create(None),
                    String::new(),
                    vec![
                        MenuAction::NewTabWithProfile(Box::new(
                            UserLaunchProfiles::default().profiles.remove(0)
                        )),
                        MenuAction::NewTabWithProfile(Box::new(custom.clone())),
                    ],
                    None
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
                assert_eq!(
                    matches!(presentation, ModalPresentation::Presented(_)),
                    width >= 16
                );
                if let ModalPresentation::Presented(draw) = presentation {
                    assert_eq!(draw.visible_rows, 2);
                    assert!(menu.presented(draw.clone()));
                    if width >= 80 {
                        let text = terminal
                            .backend()
                            .buffer()
                            .content
                            .iter()
                            .map(|cell| cell.symbol())
                            .collect::<String>();
                        let visible: String = text
                            .chars()
                            .filter(|character| !character.is_whitespace())
                            .collect();
                        let system: String = locale
                            .launch_profile_name(&UserLaunchProfiles::default().profiles[0])
                            .chars()
                            .filter(|character| !character.is_whitespace())
                            .collect();
                        assert!(visible.contains(&system));
                        assert!(text.contains("PowerShell"));
                        assert!(!text.contains('\u{1b}'));
                    }
                }
                menu.key(KeyEvent::press(KeyCode::Esc, Modifiers::NONE));
                assert!(!menu.active());
                assert!(menu.confirmed().is_none());
            }
        }
    }
}

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
                    assert!(menu.presented(draw.clone()));
                    let buffer = terminal.backend().buffer();
                    let text = visible_buffer_text(buffer);
                    assert!(text.contains("QZX"));
                    assert_eq!(text.contains("Esc"), !context);
                    assert_eq!(text.contains("1/2"), !context);
                    assert_eq!(text.contains(locale.menu_search_label()), !context);
                    let search_row = (buffer.area.y..buffer.area.bottom()).find(|y| {
                        visible_row_text(buffer, Rect::new(buffer.area.x, *y, buffer.area.width, 1))
                            .contains(locale.menu_search_label())
                    });
                    assert_eq!(search_row.is_some(), !context);
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
                    let row = action_row_rect(&map, buffer.area, draw.token);
                    if let Some(search_row) = search_row {
                        assert!(
                            row.y > search_row,
                            "the first action row follows the localized query row"
                        );
                    } else {
                        assert!(row.y > buffer.area.y, "the first row is inside the frame");
                    }
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
                    assert!(menu.presented(draw.clone()));
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
            ..
        })) if drawn == token
    ));
    (terminal.backend().buffer().clone(), hits.finish(), token)
}

fn action_row_rect(map: &FrameHitMap, area: Rect, token: MenuToken) -> Rect {
    action_row_rect_for(map, area, token, 0)
}

fn action_row_rect_for(map: &FrameHitMap, area: Rect, token: MenuToken, row_index: usize) -> Rect {
    let points = (area.y..area.bottom())
        .flat_map(|y| (area.x..area.right()).map(move |x| (x, y)))
        .filter(|(x, y)| {
            matches!(
                map.hit_target_at(*x, *y),
                Some(HitTarget::ActionMenuRow { token: drawn, row })
                    if *drawn == token && *row == row_index
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
            Some(HitTarget::ActionMenuRow { token: drawn, row })
                if *drawn == token && *row == row_index
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

fn visible_row_text(buffer: &Buffer, row: Rect) -> String {
    let mut text = String::new();
    let mut x = row.x;
    while x < row.right() {
        let symbol = buffer[(x, row.y)].symbol();
        text.push_str(symbol);
        x = x.saturating_add(u16::try_from(display_width(symbol).max(1)).unwrap_or(u16::MAX));
    }
    text
}

fn visible_buffer_text(buffer: &Buffer) -> String {
    let mut text = String::new();
    for y in buffer.area.y..buffer.area.bottom() {
        let mut x = buffer.area.x;
        while x < buffer.area.right() {
            let symbol = buffer[(x, y)].symbol();
            text.push_str(symbol);
            x = x.saturating_add(u16::try_from(display_width(symbol).max(1)).unwrap_or(u16::MAX));
        }
        text.push('\n');
    }
    text
}

#[test]
fn global_tab_and_panel_rows_render_the_current_binding_for_each_localized_action() {
    use crate::{
        action_menu::{GLOBAL_ACTIONS, MenuTarget},
        client_view::PanelDirection as Direction,
    };

    let cases = [
        (
            MenuAction::RenameTab,
            PrefixAction::Client(ClientAction::RequestRenameTab),
        ),
        (
            MenuAction::SplitPanel(Direction::Up),
            PrefixAction::Client(ClientAction::SplitPanel(Direction::Up)),
        ),
        (
            MenuAction::SplitPanel(Direction::Down),
            PrefixAction::Client(ClientAction::SplitPanel(Direction::Down)),
        ),
        (
            MenuAction::SplitPanel(Direction::Left),
            PrefixAction::Client(ClientAction::SplitPanel(Direction::Left)),
        ),
        (
            MenuAction::SplitPanel(Direction::Right),
            PrefixAction::Client(ClientAction::SplitPanel(Direction::Right)),
        ),
        (
            MenuAction::ClosePane,
            PrefixAction::Client(ClientAction::ClosePanel),
        ),
        (
            MenuAction::Zoom,
            PrefixAction::Client(ClientAction::ToggleZoom),
        ),
        (
            MenuAction::MovePanel(Direction::Up),
            PrefixAction::Client(ClientAction::MovePanel(Direction::Up)),
        ),
        (
            MenuAction::MovePanel(Direction::Down),
            PrefixAction::Client(ClientAction::MovePanel(Direction::Down)),
        ),
        (
            MenuAction::MovePanel(Direction::Left),
            PrefixAction::Client(ClientAction::MovePanel(Direction::Left)),
        ),
        (
            MenuAction::MovePanel(Direction::Right),
            PrefixAction::Client(ClientAction::MovePanel(Direction::Right)),
        ),
        (
            MenuAction::Copy,
            PrefixAction::Client(ClientAction::EnterCopyMode),
        ),
        (
            MenuAction::Resize,
            PrefixAction::Client(ClientAction::EnterResizeMode),
        ),
    ];

    let width = 140;
    let height = 60;
    let client = ClientView::new(width);
    let theme = Theme::terminal_native();
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let mut menu = ActionMenu::default();
        assert!(menu.open_for(
            MenuTarget::Global,
            String::new(),
            GLOBAL_ACTIONS.to_vec(),
            None,
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
            panic!("the global menu fits at 140x60");
        };
        assert_eq!(draw.visible_rows, GLOBAL_ACTIONS.len());
        assert!(menu.presented(draw.clone()));
        let map = hits.finish();
        let buffer = terminal.backend().buffer();

        for (action, binding_action) in &cases {
            let index = GLOBAL_ACTIONS
                .iter()
                .position(|candidate| candidate == action)
                .expect("the global menu includes each requested action");
            let row = action_row_rect_for(&map, buffer.area, draw.token, index);
            let text = visible_row_text(buffer, row);
            let chord = client
                .bindings()
                .action_labels_with_glyphs(binding_action.clone(), key_label_glyphs(&theme))
                .into_iter()
                .next()
                .expect("the keyboard action has an effective binding");
            assert!(
                text.contains(locale.menu_action(action)),
                "{locale:?}: {text}"
            );
            assert!(text.contains(&chord), "{locale:?}: {text}");
        }
    }
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
