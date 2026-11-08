use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, layout::Rect};

use super::super::agent_sidebar::{self, RenderContext, SidebarInput};
use super::super::{SessionStatus, SidebarArea, SidebarLayer, project_shell_areas};
use crate::{
    client_view::{
        ClientAction, ClientActionDisposition, ClientView, PrefixAction, SidebarSurface,
    },
    environment_marker::{EnvironmentMarker, MarkedEnvironment},
    localization::{Locale, TooltipLabel},
    terminal_text::display_width,
    tui_hit_map::{FrameHitMap, HitMapBuilder, HitTarget, TooltipContent, TooltipText},
    tui_layout::SidebarWidth,
    tui_theme::{GlyphSet, Role, Theme},
};

fn text(buffer: &Buffer, y: u16, from: u16, to: u16) -> String {
    let mut text = String::new();
    let mut x = from;
    while x < to {
        let symbol = buffer[(x, y)].symbol();
        text.push_str(symbol);
        x += u16::try_from(display_width(symbol).max(1)).unwrap();
    }
    text
}

/// The single-row extent the map resolves to `target`, or none.
fn extent(map: &FrameHitMap, area: Rect, target: &HitTarget) -> Option<Rect> {
    let cells = (area.y..area.bottom())
        .flat_map(|y| (area.x..area.right()).map(move |x| (x, y)))
        .filter(|&(x, y)| map.hit_target_at(x, y) == Some(target))
        .collect::<Vec<_>>();
    let &(left, y) = cells.first()?;
    let right = cells.iter().map(|(x, _)| *x).max().unwrap() + 1;
    assert!(
        cells.iter().all(|(_, row)| *row == y),
        "{target:?} spans rows"
    );
    assert_eq!(
        cells.len(),
        usize::from(right - left),
        "{target:?} has holes"
    );
    Some(Rect::new(left, y, right - left, 1))
}

fn visible_view(width: u16, height: u16, preferred: u16) -> ClientView {
    let mut view = ClientView::new(width);
    view.machines.available = true;
    if view.sidebar_surface() == SidebarSurface::Hidden {
        let transition = view.decide_action(ClientAction::ToggleBusinessSidebar);
        view.commit(transition, ClientActionDisposition::KeepNavigator);
    }
    view.machines.observe_agent_sidebar(true);
    let preferred = SidebarWidth::new(preferred).unwrap();
    if preferred != SidebarWidth::DEFAULT {
        assert!(
            view.machines
                .set_sidebar_width(SidebarWidth::DEFAULT, preferred)
        );
    }
    let projected = project_shell_areas(Rect::new(0, 0, width, height), view.sidebar_surface());
    assert!(
        projected.sidebar.is_some(),
        "{width}x{height} shows the sidebar"
    );
    view
}

fn action_tooltip(
    view: &ClientView,
    locale: Locale,
    label: TooltipLabel,
    key: PrefixAction,
) -> TooltipContent {
    TooltipContent::Action {
        label: locale.tooltip_label(label).to_owned(),
        key: view
            .bindings()
            .action_labels_with_glyphs(key, Theme::terminal_native().glyphs.key_labels())
            .into_iter()
            .next(),
    }
}

/// The visible sidebar ends its first row with the hide toggle and its Settings row with Help, both
/// at the list's trailing edge beside the right padding cell, two blank cells clear of the action
/// they follow; each names its action and the key that does the same.
#[test]
fn visible_sidebar_ends_its_first_and_settings_rows_with_the_toggle_and_help() {
    let mut ascii = Theme::terminal_native();
    ascii.glyphs = GlyphSet::ASCII;
    for (width, height) in [(120, 40), (80, 24), (60, 12)] {
        for preferred in [24, 30, 60] {
            for locale in [Locale::EnUs, Locale::ZhCn] {
                for (theme, hide) in [
                    (Theme::terminal_native(), "[«]"),
                    (Theme::rgb_test_dark(), "[«]"),
                    (ascii, "[<]"),
                ] {
                    let view = visible_view(width, height, preferred);
                    let sidebar =
                        project_shell_areas(Rect::new(0, 0, width, height), view.sidebar_surface())
                            .sidebar
                            .unwrap()
                            .area;
                    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                    let mut hits = HitMapBuilder::new();
                    terminal
                        .draw(|frame| {
                            super::super::render_terminal_shell(
                                frame, &view, locale, None, &theme, &mut hits,
                            );
                        })
                        .unwrap();
                    let tooltips = hits.tooltip_contents().cloned().collect::<Vec<_>>();
                    let map = hits.finish();
                    let buffer = terminal.backend().buffer();
                    // The list's own columns end before the right padding cell and divider.
                    let list_end = sidebar.right() - 2;
                    let case = format!("{width}x{height} sidebar {preferred} {locale:?}");

                    let toggle = extent(&map, sidebar, &HitTarget::SidebarHideButton)
                        .unwrap_or_else(|| panic!("{case}: hide toggle"));
                    assert_eq!(toggle, Rect::new(list_end - 3, sidebar.y, 3, 1), "{case}");
                    assert_eq!(text(buffer, sidebar.y, toggle.x, toggle.right()), hide);
                    for fixed in [HitTarget::NewWorkspaceButton, HitTarget::SearchButton] {
                        let fixed = extent(&map, sidebar, &fixed).unwrap();
                        if fixed.y == toggle.y {
                            assert!(
                                toggle.x >= fixed.right() + 2,
                                "{case}: the toggle crowds {fixed:?}"
                            );
                            assert_eq!(
                                text(buffer, toggle.y, fixed.right(), toggle.x).trim(),
                                "",
                                "{case}"
                            );
                        }
                    }

                    let last = sidebar.bottom() - 1;
                    let settings = extent(&map, sidebar, &HitTarget::SettingsButton).unwrap();
                    let help = extent(&map, sidebar, &HitTarget::ShortcutHelpButton)
                        .unwrap_or_else(|| panic!("{case}: help"));
                    assert_eq!(settings.y, last, "{case}");
                    assert_eq!(help, Rect::new(list_end - 3, last, 3, 1), "{case}");
                    assert_eq!(text(buffer, last, help.x, help.right()), "[?]");
                    assert!(
                        help.x >= settings.right() + 2,
                        "{case}: Help crowds Settings"
                    );
                    assert_eq!(
                        text(buffer, last, settings.right(), help.x).trim(),
                        "",
                        "{case}"
                    );

                    // Both rest like every icon action on the sidebar's chrome.
                    let resting = theme.content_on_surface(
                        Role::SurfaceChrome,
                        None,
                        Some(Role::TextSecondary),
                        theme.style(Role::TextSecondary),
                    );
                    for (x, y) in (toggle.x..toggle.right())
                        .map(|x| (x, toggle.y))
                        .chain((help.x..help.right()).map(|x| (x, help.y)))
                    {
                        assert_eq!(buffer[(x, y)].style(), resting, "{case} ({x}, {y})");
                    }

                    for (label, key) in [
                        (TooltipLabel::HideSidebar, PrefixAction::SidebarToggle),
                        (TooltipLabel::Shortcuts, PrefixAction::ShowHelp),
                    ] {
                        let expected = action_tooltip(&view, locale, label, key);
                        assert!(tooltips.contains(&expected), "{case}: {expected:?}");
                    }
                }
            }
        }
    }
}

/// The hidden sidebar leaves its toggle at the Tab strip's leading edge, one strip cell before
/// the Tabs, naming the restore action and its key.
#[test]
fn hidden_sidebar_leaves_its_toggle_at_the_strip_leading_edge() {
    for (width, height) in [(120, 40), (80, 24), (60, 12)] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let mut view = visible_view(width, height, 30);
            let transition = view.decide_action(ClientAction::ToggleBusinessSidebar);
            view.commit(transition, ClientActionDisposition::KeepNavigator);
            assert_eq!(view.sidebar_surface(), SidebarSurface::Hidden);
            let theme = Theme::terminal_native();
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            let mut hits = HitMapBuilder::new();
            terminal
                .draw(|frame| {
                    super::super::render_terminal_shell(
                        frame, &view, locale, None, &theme, &mut hits,
                    );
                })
                .unwrap();
            let tooltips = hits.tooltip_contents().cloned().collect::<Vec<_>>();
            let map = hits.finish();
            let area = Rect::new(0, 0, width, height);
            let toggle = extent(&map, area, &HitTarget::SidebarRestoreButton).unwrap();
            assert_eq!(toggle, Rect::new(0, 0, 3, 1));
            assert_eq!(text(terminal.backend().buffer(), 0, 0, 4), "[»] ");
            for hidden in [
                HitTarget::SidebarHideButton,
                HitTarget::ShortcutHelpButton,
                HitTarget::SettingsButton,
            ] {
                assert!(extent(&map, area, &hidden).is_none(), "{hidden:?}");
            }
            let expected = action_tooltip(
                &view,
                locale,
                TooltipLabel::ShowSidebar,
                PrefixAction::SidebarToggle,
            );
            assert!(tooltips.contains(&expected), "{expected:?}");
        }
    }
}

/// The mobile return affordance must keep both brackets and its seven-cell target when the
/// English action and shortcut compete for the first row at phone width.
#[test]
fn mobile_return_keeps_its_complete_control_before_the_shortcut_hint() {
    let (width, height) = (34, 30);
    let mut view = visible_view(width, height, 30);
    view.machines.enter_sidebar_navigation();
    let theme = Theme::orchbee_light();
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut hits = HitMapBuilder::new();
    terminal
        .draw(|frame| {
            super::super::render_terminal_shell(
                frame,
                &view,
                Locale::EnUs,
                None,
                &theme,
                &mut hits,
            );
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    assert!(text(buffer, 1, 0, width).contains("[ « ]"));
    let map = hits.finish();
    for y in 0..3 {
        let target: Vec<_> = (0..width)
            .filter(|x| map.hit_target_at(*x, y) == Some(&HitTarget::SidebarHideButton))
            .collect();
        assert_eq!(target.len(), 7);
    }
}

/// Narrower sidebar regions than the admitted width drop Help before Settings and the toggle before
/// the action sharing its row; nothing ever overlaps.
#[test]
fn narrow_sidebar_regions_drop_the_icons_before_their_actions_without_overlap() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let settings_width = u16::try_from(display_width(locale.settings_title())).unwrap() + 4;
        for width in 3..=24 {
            let height = 12;
            let view = visible_view(120, 40, 30);
            let theme = Theme::terminal_native();
            let area = Rect::new(0, 0, width, height);
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            let mut hits = HitMapBuilder::new();
            terminal
                .draw(|frame| {
                    agent_sidebar::render(
                        frame,
                        SidebarArea {
                            area,
                            layer: SidebarLayer::Docked,
                        },
                        RenderContext {
                            native: None,
                            view: &view,
                            sidebar_surface: view.sidebar_surface(),
                            locale,
                            backend_status: SessionStatus {
                                text: "",
                                role: Role::StatusSuccess,
                            },
                            theme: &theme,
                            input: SidebarInput::Terminal,
                            creation: None,
                            held: None,
                        },
                        Some(&mut hits),
                    );
                })
                .unwrap();
            let map = hits.finish();
            let list = width - 2;
            let toggle = extent(&map, area, &HitTarget::SidebarHideButton);
            let new_workspace = extent(&map, area, &HitTarget::NewWorkspaceButton);
            let settings = extent(&map, area, &HitTarget::SettingsButton);
            let help = extent(&map, area, &HitTarget::ShortcutHelpButton);
            assert_eq!(
                settings.is_some(),
                list >= settings_width,
                "{locale:?} width {width}"
            );
            assert_eq!(
                help.is_some(),
                list >= settings_width + 2 + 3,
                "{locale:?} width {width}"
            );
            if let (Some(settings), Some(help)) = (settings, help) {
                assert!(help.x >= settings.right() + 2);
                assert!(help.right() <= list);
            }
            if let Some(toggle) = toggle {
                assert_eq!(toggle.y, 0);
                assert!(toggle.right() <= list);
                if let Some(new_workspace) = new_workspace.filter(|action| action.y == toggle.y) {
                    assert!(
                        toggle.x >= new_workspace.right() + 2,
                        "{locale:?} width {width}"
                    );
                }
            }
        }
    }
}

/// Paints the sidebar alone into `area` for `view`, returning the buffer and the candidate map.
fn paint_sidebar(
    view: &ClientView,
    locale: Locale,
    area: Rect,
    layer: SidebarLayer,
) -> (Buffer, HitMapBuilder) {
    let theme = Theme::terminal_native();
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    let mut hits = HitMapBuilder::new();
    terminal
        .draw(|frame| {
            agent_sidebar::render(
                frame,
                SidebarArea { area, layer },
                RenderContext {
                    native: None,
                    view,
                    sidebar_surface: view.sidebar_surface(),
                    locale,
                    backend_status: SessionStatus {
                        text: "",
                        role: Role::StatusSuccess,
                    },
                    theme: &theme,
                    input: SidebarInput::Terminal,
                    creation: None,
                    held: None,
                },
                Some(&mut hits),
            );
        })
        .unwrap();
    (terminal.backend().buffer().clone(), hits)
}

/// The environment marker takes only the cells Settings and Help leave on their row: it never
/// moves or drops either, keeps two blank cells from each, and shows the checkout name only whole.
/// It is not part of Settings' target, and its tooltip names the build and the checkout.
#[test]
fn environment_marker_takes_only_what_settings_and_help_leave() {
    let development = EnvironmentMarker {
        environment: MarkedEnvironment::Development,
        checkout: Some("orchbee"),
    };
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let word = locale.environment_label(MarkedEnvironment::Development);
        let named = format!("{word} · orchbee");
        let tooltip = TooltipContent::Describe {
            text: Some(TooltipText {
                text: locale.environment_checkout(MarkedEnvironment::Development, "orchbee"),
                middle: false,
            }),
            legend: Vec::new(),
        };
        let settings_width = u16::try_from(display_width(locale.settings_title())).unwrap() + 4;
        for width in 3..=40 {
            let mut view = visible_view(120, 40, 30);
            view.set_environment_marker(Some(development));
            let area = Rect::new(0, 0, width, 12);
            let (buffer, hits) = paint_sidebar(&view, locale, area, SidebarLayer::Docked);
            let last = area.bottom() - 1;
            let list = width - 2;
            let expected = (list >= settings_width + 2 + 3).then(|| {
                let room = usize::from((list - 3 - 2).saturating_sub(settings_width + 2));
                [named.as_str(), word]
                    .into_iter()
                    .find(|text| display_width(text) <= room)
                    .unwrap_or("")
            });
            let case = format!("{locale:?} width {width}");
            let offered = expected.filter(|text| !text.is_empty()).map(|_| {
                hits.tooltip_content_at(list - 3 - 2 - 1, last)
                    .cloned()
                    .unwrap_or_else(|| panic!("{case}: tooltip"))
            });
            let map = hits.finish();
            let settings = extent(&map, area, &HitTarget::SettingsButton);
            let help = extent(&map, area, &HitTarget::ShortcutHelpButton);
            assert_eq!(settings.is_some(), list >= settings_width, "{case}");
            assert_eq!(help.is_some(), expected.is_some(), "{case}");
            let (Some(settings), Some(help), Some(expected)) = (settings, help, expected) else {
                continue;
            };
            assert_eq!(help, Rect::new(list - 3, last, 3, 1), "{case}");
            let between = text(&buffer, last, settings.right(), help.x);
            assert_eq!(between.trim(), expected, "{case}");
            assert!(
                between.starts_with("  ") && between.ends_with("  "),
                "{case}"
            );
            if let Some(offered) = offered {
                assert_eq!(offered, tooltip, "{case}");
            }
        }

        // On the touch-spaced list Settings' target stops a blank cell short of the marker.
        let mut view = visible_view(120, 40, 30);
        view.set_environment_marker(Some(development));
        let area = Rect::new(0, 0, 46, 28);
        let (buffer, hits) = paint_sidebar(&view, locale, area, SidebarLayer::Screen);
        let map = hits.finish();
        let row = area.bottom() - 2;
        let line = text(&buffer, row, 0, area.width);
        let at = line
            .find(&named)
            .unwrap_or_else(|| panic!("{locale:?}: {line:?}"));
        let marker_x = u16::try_from(display_width(&line[..at])).unwrap();
        for x in marker_x - 1..marker_x + u16::try_from(display_width(&named)).unwrap() {
            assert_ne!(
                map.hit_target_at(x, row),
                Some(&HitTarget::SettingsButton),
                "{locale:?} at {x}"
            );
        }
        assert_eq!(
            map.hit_target_at(0, row),
            Some(&HitTarget::SettingsButton),
            "{locale:?}"
        );
    }
}

/// A test build packaged from a home, not a checkout, shows only its word and offers no tooltip.
#[test]
fn environment_marker_without_a_checkout_shows_only_its_word() {
    let mut view = visible_view(120, 40, 30);
    view.set_environment_marker(Some(EnvironmentMarker {
        environment: MarkedEnvironment::Test,
        checkout: None,
    }));
    let area = Rect::new(0, 0, 30, 12);
    let (buffer, hits) = paint_sidebar(&view, Locale::EnUs, area, SidebarLayer::Docked);
    let last = area.bottom() - 1;
    // The list's 28 columns end with Help in the last three, two blank cells after the word.
    assert_eq!(text(&buffer, last, 17, 25), "  Test  ");
    assert_eq!(hits.tooltip_content_at(22, last), None);
}
