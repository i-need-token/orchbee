use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, layout::Rect};

use super::super::agent_sidebar::{self, RenderContext, SidebarInput};
use super::super::{SessionStatus, SidebarArea, SidebarLayer, project_shell_areas};
use crate::{
    client_view::{
        ClientAction, ClientActionDisposition, ClientView, PrefixAction, SidebarSurface,
    },
    localization::{Locale, TooltipLabel},
    terminal_text::display_width,
    tui_hit_map::{FrameHitMap, HitMapBuilder, HitTarget, TooltipContent},
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
