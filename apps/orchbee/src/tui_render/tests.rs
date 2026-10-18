use std::convert::Infallible;

use ratatui::{Terminal, backend::TestBackend};

use crate::{
    client_view::{
        BackendAction, ClientView, KeyCode, KeyEvent, Modifiers, PrefixInput,
        SessionActionDisposition,
    },
    localization::Locale,
    tui_layout::ViewportTier,
    tui_theme::Role,
};

use super::render_shell;

#[test]
fn close_request_notice_does_not_replace_the_exact_status_text() {
    let mut terminal = infallible(Terminal::new(TestBackend::new(100, 1)));
    let view = ClientView::new(100);
    let mut visible = None;
    infallible(terminal.draw(|frame| {
        visible = super::render_status(
            frame,
            frame.area(),
            &view,
            None,
            Locale::EnUs,
            "terminal · Close cleanup blocked",
            Some("Close request was not applied"),
            false,
            false,
            false,
            ViewportTier::Full,
            &crate::tui_theme::Theme::rgb_test_dark(),
            None,
        );
    }));
    assert!(visible.is_some());
    let text = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(text.contains("terminal · Close cleanup blocked"));
    assert!(text.contains("Close request was not applied"));
}

#[cfg(windows)]
#[test]
fn ux_status_matching_native_notice_is_painted_once_and_keeps_hints() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let notice =
            locale.native_notice(crate::terminal_session::NativeSessionNotice::MenuUnavailable);
        for status in [notice.clone(), format!("{notice} · recovery")] {
            for shell_notice in [
                None,
                Some(crate::client_view::Notice::ConfigurationPublished),
            ] {
                let mut terminal = infallible(Terminal::new(TestBackend::new(120, 1)));
                let mut view = ClientView::new(120);
                if let Some(shell_notice) = shell_notice {
                    view.configuration_notice(shell_notice);
                }
                infallible(terminal.draw(|frame| {
                    super::render_status(
                        frame,
                        frame.area(),
                        &view,
                        None,
                        locale,
                        &status,
                        Some(&notice),
                        false,
                        false,
                        false,
                        ViewportTier::Full,
                        &crate::tui_theme::Theme::one_light(),
                        None,
                    );
                }));
                let text = buffer_rows(&terminal, 120, 1).concat();
                let needle = match locale {
                    Locale::EnUs => "Menu action",
                    Locale::ZhCn => "菜单动作",
                };
                assert_eq!(
                    without_cell_padding(&text)
                        .matches(&without_cell_padding(needle))
                        .count(),
                    1,
                    "{locale:?}: {text:?}"
                );
                if shell_notice.is_some() {
                    assert!(
                        without_cell_padding(&text)
                            .contains(&without_cell_padding(locale.shell_notice(shell_notice)))
                    );
                } else {
                    assert!(without_cell_padding(&text).contains(&without_cell_padding(&status)));
                    let idle_hint = format!(
                        "{} {}",
                        crate::client_view::format_binding_with_glyphs(
                            &view.bindings().prefix,
                            crate::client_view::LabelStyle::for_host(),
                            crate::client_view::KeyLabelGlyphs::Unicode,
                        ),
                        locale.status().shortcuts,
                    );
                    assert!(
                        without_cell_padding(&text).contains(&without_cell_padding(&idle_hint)),
                        "{locale:?} missing idle hint: {text:?}"
                    );
                }
            }
        }
    }
}

#[cfg(windows)]
#[test]
fn sidebar_status_keeps_canonical_whole_hints_and_drops_oversized_facts() {
    use crate::{
        machine_picker::{SidebarCommand, sidebar_binding_label},
        terminal_text::display_width,
        tui_theme::{GlyphSet, Theme},
    };

    for width in [70, 120] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for glyphs in [GlyphSet::UTF8, GlyphSet::ASCII] {
                let theme = Theme::rgb_test_dark().with_glyphs(glyphs);
                let mut view = ClientView::new(width);
                view.machines.available = true;
                assert!(view.machines.enter_sidebar_navigation());
                let mut commands = vec![SidebarCommand::Exit];
                if width == 120 {
                    commands.extend([
                        SidebarCommand::Activate,
                        SidebarCommand::OpenMenu,
                        SidebarCommand::Help,
                    ]);
                }
                let key_glyphs = if glyphs == GlyphSet::ASCII {
                    crate::client_view::KeyLabelGlyphs::Ascii
                } else {
                    crate::client_view::KeyLabelGlyphs::Unicode
                };
                let hints = commands
                    .into_iter()
                    .map(|command| {
                        format!(
                            "{} {}",
                            sidebar_binding_label(command, key_glyphs).unwrap(),
                            locale.sidebar_navigation_action(command),
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(&format!(" {} ", theme.glyphs.middle_dot));
                let expected = format!(
                    "{} {} {hints}",
                    locale.status().sidebar,
                    theme.glyphs.middle_dot,
                );
                for (status, fits) in [
                    ("Ready".to_owned(), true),
                    ("Q".repeat(usize::from(width)), false),
                    (format!("{}界", "Q".repeat(usize::from(width - 2))), false),
                ] {
                    let mut terminal = infallible(Terminal::new(TestBackend::new(width, 1)));
                    let mut visible = None;
                    infallible(terminal.draw(|frame| {
                        visible = super::render_status(
                            frame,
                            frame.area(),
                            &view,
                            None,
                            locale,
                            &status,
                            None,
                            false,
                            false,
                            false,
                            ViewportTier::from_size(width, 30),
                            &theme,
                            None,
                        );
                    }));
                    assert_eq!(
                        visible,
                        fits.then_some(ratatui::layout::Rect::new(
                            0,
                            0,
                            u16::try_from(display_width(&status)).unwrap(),
                            1,
                        )),
                        "{locale:?} {width}: {status}"
                    );
                    let buffer = terminal.backend().buffer();
                    let retained = (0..width)
                        .map(|x| buffer[(x, 0)].symbol())
                        .collect::<String>();
                    if fits {
                        assert!(retained.starts_with(&status));
                    } else {
                        assert!(!retained.starts_with('Q'));
                        assert!(!retained.contains(glyphs.ellipsis));
                    }
                    assert!(
                        without_cell_padding(&retained).ends_with(&without_cell_padding(&expected)),
                        "{retained:?}"
                    );
                    if glyphs == GlyphSet::ASCII {
                        assert!(
                            !['↑', '↓', '←', '→']
                                .into_iter()
                                .any(|arrow| retained.contains(arrow))
                        );
                    }
                }
            }
        }
    }
}

#[cfg(windows)]
#[test]
fn action_menu_emits_borders_without_leaving_crossing_underlay_glyphs() {
    use crate::{
        action_menu::{ActionMenu, MenuAction, MenuUnavailable},
        terminal_session::ModalPresentation,
        terminal_text::display_width,
        tui_pointer::{HitMapBuilder, HitTarget},
        tui_theme::{Role, Theme},
    };
    use ratatui::{
        TerminalOptions, Viewport,
        buffer::{Buffer, CellDiffOption},
        layout::Rect,
        style::Modifier,
    };

    for (theme_name, theme) in [
        ("native", Theme::terminal_native()),
        ("dark", Theme::one_dark()),
        ("light", Theme::one_light()),
    ] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for (origin, anchor) in [
                ((0, 0), (10, 12)),
                ((3, 2), (13, 14)),
                ((3, 2), (3, 14)),
                ((3, 2), (82, 14)),
            ] {
                let viewport = Rect::new(origin.0, origin.1, 80, 24);
                let case = format!("{theme_name} {locale:?} {viewport:?} {anchor:?}");
                let mut menu = ActionMenu::default();
                assert!(menu.open(
                    "Empty Project".into(),
                    vec![
                        MenuAction::NewWorkspace,
                        MenuAction::QuickWorkspace,
                        MenuAction::RenameProject,
                        MenuAction::NewWorktree,
                        MenuAction::OpenWorktree,
                    ],
                    Some(anchor),
                ));
                menu.refresh_availability(|_| Some(MenuUnavailable::ProjectWorkspaceRequired));
                let token = menu.view().unwrap().token;
                let mut terminal = infallible(Terminal::with_options(
                    TestBackend::new(viewport.right(), viewport.bottom()),
                    TerminalOptions {
                        viewport: Viewport::Fixed(viewport),
                    },
                ));
                // Obtain the canonical menu projection without duplicating its sizing rules.
                let mut hits = HitMapBuilder::new();
                let canonical = infallible(terminal.draw(|frame| {
                    assert!(matches!(
                        super::action_menu::render(
                            frame,
                            viewport,
                            menu.view().unwrap(),
                            &crate::client_view::Bindings::default(),
                            locale,
                            &theme,
                            &mut hits,
                        ),
                        ModalPresentation::Presented(_)
                    ));
                }))
                .buffer
                .clone();
                let canonical_hits = hits.finish();
                let points: Vec<_> = (viewport.y..viewport.bottom())
                    .flat_map(|y| (viewport.x..viewport.right()).map(move |x| (x, y)))
                    .filter(|(x, y)| canonical_hits.hit_target_at(*x, *y).is_some())
                    .collect();
                let left = points.iter().map(|(x, _)| *x).min().unwrap();
                let top = points.iter().map(|(_, y)| *y).min().unwrap();
                let right = points.iter().map(|(x, _)| *x).max().unwrap() + 1;
                let bottom = points.iter().map(|(_, y)| *y).max().unwrap() + 1;
                let area = Rect::new(left, top, right - left, bottom - top);
                assert_eq!(area.height, 12);
                let first_row = points.iter().find_map(|(x, y)| {
                    matches!(
                        canonical_hits.hit_target_at(*x, *y),
                        Some(HitTarget::ActionMenuRow { token: drawn, row: 0 })
                            if *drawn == token
                    )
                    .then_some((*x, *y))
                });
                let (first_row_x, first_row_y) =
                    first_row.expect("the first action row is painted");
                assert_eq!(
                    canonical_hits.hit_target_at(first_row_x, first_row_y),
                    Some(&HitTarget::ActionMenuRow { token, row: 0 })
                );

                let surface = theme
                    .style(Role::SurfaceChrome)
                    .patch(theme.style(Role::TextPrimary));
                let glyph_style = theme
                    .style(Role::SurfaceChrome)
                    .patch(theme.style(Role::AccentFocus))
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
                let mut underlay = Buffer::empty(viewport);
                for y in viewport.y..viewport.bottom() {
                    underlay.set_string(viewport.x, y, "p".repeat(80), surface);
                }
                let mut seams = Vec::new();
                for (row, (glyph, crosses, skip)) in [
                    ("中", true, false),
                    ("🙂", true, false),
                    ("中", false, false),
                    ("🙂", false, false),
                    ("X", false, false),
                    ("中", true, true),
                    ("🙂", true, true),
                ]
                .into_iter()
                .enumerate()
                {
                    let y = top + 1 + u16::try_from(row).unwrap();
                    let width = u16::try_from(display_width(glyph)).unwrap();
                    let left_glyph = if crosses && left > viewport.x {
                        left - 1
                    } else if !crosses && left >= viewport.x + width {
                        left - width
                    } else {
                        left
                    };
                    let right_glyph = if crosses && right < viewport.right() {
                        right - 1
                    } else if !crosses && right + width <= viewport.right() {
                        right
                    } else {
                        right - width
                    };
                    for x in [left_glyph, right_glyph] {
                        underlay.set_string(x, y, glyph, glyph_style);
                        if skip {
                            underlay[(x + 1, y)].set_diff_option(CellDiffOption::Skip);
                        }
                        let style = underlay[(x, y)].style();
                        if crosses && x < left {
                            seams.push((x, y, style));
                        }
                        if crosses && x + width > right {
                            seams.push((right, y, style));
                        }
                    }
                }
                infallible(terminal.clear());
                infallible(terminal.draw(|frame| *frame.buffer_mut() = underlay.clone()));
                let mut previous = underlay.clone();
                for pass in 0..2 {
                    let mut hits = HitMapBuilder::new();
                    let painted = infallible(terminal.draw(|frame| {
                        *frame.buffer_mut() = underlay.clone();
                        assert!(matches!(
                            super::action_menu::render(
                                frame,
                                viewport,
                                menu.view().unwrap(),
                                &crate::client_view::Bindings::default(),
                                locale,
                                &theme,
                                &mut hits,
                            ),
                            ModalPresentation::Presented(_)
                        ));
                    }))
                    .buffer
                    .clone();
                    let emitted = previous.diff(&painted);
                    if pass == 1 {
                        assert!(emitted.is_empty(), "repeat {case}");
                    }
                    for y in top..bottom {
                        for x in [left, right - 1] {
                            if pass == 0 {
                                assert!(
                                    emitted.iter().any(|(cx, cy, cell)| {
                                        (*cx, *cy) == (x, y) && **cell == canonical[(x, y)]
                                    }),
                                    "border must be emitted: {case} ({x},{y})"
                                );
                            }
                            assert_eq!(
                                terminal.backend().buffer()[(x, y)],
                                canonical[(x, y)],
                                "drawn border: {case} ({x},{y})"
                            );
                        }
                    }
                    for &(x, y, style) in &seams {
                        let cell = &painted[(x, y)];
                        assert_eq!(cell.symbol(), " ");
                        assert_eq!(cell.style(), style, "seam style: {case} ({x},{y})");
                        assert_eq!(cell.diff_option, CellDiffOption::None);
                        assert_eq!(terminal.backend().buffer()[(x, y)], *cell);
                    }
                    let hits = hits.finish();
                    for y in viewport.y..viewport.bottom() {
                        for x in viewport.x..viewport.right() {
                            assert_eq!(
                                hits.hit_target_at(x, y),
                                canonical_hits.hit_target_at(x, y),
                                "hit geometry: {case} ({x},{y})"
                            );
                            if !area.contains((x, y).into())
                                && !seams.iter().any(|(sx, sy, _)| (*sx, *sy) == (x, y))
                            {
                                assert_eq!(
                                    painted[(x, y)],
                                    underlay[(x, y)],
                                    "intact outside cell: {case} ({x},{y})"
                                );
                            }
                        }
                    }
                    previous = painted;
                }
                menu.cancel();
                infallible(terminal.draw(|frame| *frame.buffer_mut() = underlay.clone()));
                for y in viewport.y..viewport.bottom() {
                    let mut x = viewport.x;
                    while x < viewport.right() {
                        let cell = &underlay[(x, y)];
                        assert_eq!(
                            terminal.backend().buffer()[(x, y)],
                            *cell,
                            "restored visible glyph: {case} ({x},{y})"
                        );
                        x += u16::try_from(display_width(cell.symbol()).max(1)).unwrap();
                    }
                }
            }
        }
    }
}

fn rendered_text(width: u16, height: u16, view: &ClientView, locale: Locale) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = infallible(Terminal::new(backend));
    infallible(terminal.draw(|frame| {
        render_shell(
            frame,
            view,
            locale,
            &crate::tui_theme::Theme::rgb_test_dark(),
        );
    }));
    let buffer = terminal.backend().buffer();
    let mut text = String::new();
    for y in 0..height {
        for x in 0..width {
            text.push_str(buffer[(x, y)].symbol());
        }
        text.push('\n');
    }
    text
}

fn rendered_rows(width: u16, height: u16, view: &ClientView, locale: Locale) -> Vec<String> {
    rendered_text(width, height, view, locale)
        .split('\n')
        .map(str::to_owned)
        .collect()
}

fn infallible<T>(result: Result<T, Infallible>) -> T {
    match result {
        Ok(value) => value,
        Err(never) => match never {},
    }
}

fn without_cell_padding(value: &str) -> String {
    value.replace(' ', "")
}

#[cfg(windows)]
#[test]
fn root_chrome_actions_use_canonical_geometry_roles_and_exact_targets() {
    use crate::{
        client_view::{ClientAction, SidebarSurface},
        terminal_text::display_width,
        tui_layout::{ViewportTier, project_shell_areas},
        tui_pointer::{HitMapBuilder, HitTarget},
        tui_theme::{Role, Theme},
    };
    use ratatui::{layout::Rect, style::Modifier};

    for (width, height) in [(20, 5), (40, 14), (50, 10), (59, 11), (80, 24), (120, 40)] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let mut view = ClientView::new(width);
            view.machines.available = true;
            if view.sidebar_surface() != SidebarSurface::Hidden {
                let transition = view.decide_action(ClientAction::ToggleBusinessSidebar);
                view.commit(transition, SessionActionDisposition::KeepNavigator);
            }
            assert_eq!(view.sidebar_surface(), SidebarSurface::Hidden);

            let projected =
                project_shell_areas(Rect::new(0, 0, width, height), view.sidebar_surface());
            let theme = Theme::one_dark();
            let mut terminal = infallible(Terminal::new(TestBackend::new(width, height)));
            let mut hits = HitMapBuilder::new();
            infallible(terminal.draw(|frame| {
                super::render_terminal_shell(frame, &view, locale, None, &theme, &mut hits);
            }));
            let map = hits.finish();
            let buffer = terminal.backend().buffer();
            let target_rect = |target: &HitTarget| {
                let cells = (0..height)
                    .flat_map(|y| (0..width).map(move |x| (x, y)))
                    .filter(|&(x, y)| map.hit_target_at(x, y) == Some(target))
                    .collect::<Vec<_>>();
                if cells.is_empty() {
                    return None;
                }
                let y = cells[0].1;
                assert!(cells.iter().all(|(_, candidate_y)| *candidate_y == y));
                let left = cells.iter().map(|(x, _)| *x).min().unwrap();
                let right = cells.iter().map(|(x, _)| *x).max().unwrap() + 1;
                assert_eq!(cells.len(), usize::from(right - left));
                Some(Rect::new(left, y, right - left, 1))
            };
            let assert_action = |rect: Rect, label: &str| {
                assert_eq!(
                    usize::from(rect.width),
                    display_width(label) + 5,
                    "one focus cell plus canonical `[ Label ]` grammar"
                );
                let text = (rect.x..rect.right())
                    .map(|x| buffer[(x, rect.y)].symbol())
                    .collect::<String>();
                assert_eq!(without_cell_padding(&text), format!("[{label}]"));
                let expected = theme.content_on_surface(
                    Role::SurfaceChrome,
                    None,
                    Some(Role::TextPrimary),
                    theme.style(Role::TextPrimary),
                );
                let mut x = rect.x;
                while x < rect.right() {
                    let cell = &buffer[(x, rect.y)];
                    assert_eq!(cell.style(), expected, "action cell at column {x}");
                    assert!(!cell.modifier.contains(Modifier::BOLD));
                    // A wide grapheme's lead cell owns the terminal style for its complete
                    // display width; TestBackend represents continuation cells as padding.
                    x = x.saturating_add(
                        u16::try_from(display_width(cell.symbol()).max(1))
                            .expect("one action glyph fits in the terminal width"),
                    );
                }
                assert_eq!(x, rect.right(), "action styles cover the exact target");
            };

            let menu = target_rect(&HitTarget::MenuButton);
            if projected.tier == ViewportTier::Minimal {
                assert!(
                    menu.is_none(),
                    "{width}x{height} {locale:?}: no content overlay"
                );
                assert!(target_rect(&HitTarget::SidebarRestoreButton).is_none());
            } else {
                let menu = menu.expect("global Menu action");
                assert_action(menu, locale.menu_title());
                assert_eq!(menu.right(), projected.tabs.right());
                let restore = target_rect(&HitTarget::SidebarRestoreButton)
                    .expect("hidden available sidebar restore action");
                assert_action(restore, locale.sidebar_restore_label());
                assert_eq!(restore.x, projected.tabs.x);
                assert!(restore.right() <= menu.x);
            }
        }
    }
}

#[cfg(windows)]
#[test]
fn too_small_root_compositor_paints_only_the_notice_and_status() {
    use crate::{
        tui_pointer::{HitMapBuilder, HitTarget},
        tui_theme::Theme,
    };

    #[derive(Clone, Copy, Debug)]
    enum Scenario {
        Ordinary,
        Help,
        Resize,
    }

    for (width, height) in [(100, 1), (100, 3), (100, 4), (19, 24)] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for scenario in [Scenario::Ordinary, Scenario::Help, Scenario::Resize] {
                let mut view = ClientView::new(width);
                view.machines.available = true;
                match scenario {
                    Scenario::Ordinary => {}
                    Scenario::Help => prefix(&mut view, plain('?')),
                    Scenario::Resize => prefix(&mut view, plain('P')),
                }

                let theme = Theme::one_dark();
                let key_glyphs = match theme.glyphs.mode() {
                    crate::tui_theme::GlyphMode::Unicode => {
                        crate::client_view::KeyLabelGlyphs::Unicode
                    }
                    crate::tui_theme::GlyphMode::Simplified => {
                        crate::client_view::KeyLabelGlyphs::Ascii
                    }
                };
                let exit_key = view
                    .bindings()
                    .action_labels_with_glyphs(
                        crate::client_view::PrefixAction::Client(
                            crate::client_view::ClientAction::Exit,
                        ),
                        key_glyphs,
                    )
                    .into_iter()
                    .next()
                    .expect("the effective Exit action remains bound");
                let exit_hint = format!("{exit_key} {}", locale.status().exit);
                let resize_exit = crate::client_view::format_binding_with_glyphs(
                    &crate::client_view::Binding::new(
                        crate::client_view::KeyCode::Esc,
                        crate::client_view::Modifiers::NONE,
                    ),
                    crate::client_view::LabelStyle::for_host(),
                    key_glyphs,
                );
                let mut terminal = infallible(Terminal::new(TestBackend::new(width, height)));
                let mut hits = HitMapBuilder::new();
                infallible(terminal.draw(|frame| {
                    super::render_terminal_shell(frame, &view, locale, None, &theme, &mut hits);
                }));
                assert_eq!(
                    hits.painted_panes().count(),
                    0,
                    "{width}x{height} {scenario:?}"
                );
                let map = hits.finish();
                assert_eq!(
                    map.help_draw_source(),
                    None,
                    "{width}x{height} {scenario:?} published hidden Help metadata"
                );
                for row in 0..height {
                    for column in 0..width {
                        if let Some(target) = map.hit_target_at(column, row) {
                            assert_eq!(
                                target,
                                &HitTarget::OverlaySurface,
                                "{width}x{height} {scenario:?} published {target:?}"
                            );
                        }
                    }
                }

                let rows = buffer_rows(&terminal, width, height);
                let body_rows = &rows[..usize::from(height.saturating_sub(1))];
                let visible_body = body_rows
                    .iter()
                    .map(|row| without_cell_padding(row))
                    .filter(|row| !row.is_empty())
                    .collect::<Vec<_>>();
                let marker = format!("{} ", theme.glyphs.status_bullet);
                let marker_width = crate::terminal_text::display_width(&marker);
                let expected_notice = if marker_width < usize::from(width) {
                    format!(
                        "{marker}{}",
                        crate::terminal_text::truncate_display(
                            locale.shell().too_small_notice,
                            usize::from(width) - marker_width,
                            theme.glyphs.ellipsis,
                        )
                    )
                } else {
                    crate::terminal_text::truncate_display(
                        locale.shell().too_small_notice,
                        usize::from(width),
                        theme.glyphs.ellipsis,
                    )
                };
                let expected_body = (height > 1)
                    .then(|| without_cell_padding(&expected_notice))
                    .into_iter()
                    .collect::<Vec<_>>();
                assert_eq!(
                    visible_body, expected_body,
                    "{width}x{height} {scenario:?} painted hidden chrome"
                );
                if height > 1 {
                    let buffer = terminal.backend().buffer();
                    let notice_row = (0..height - 1)
                        .find(|&row| {
                            (0..width).any(|column| {
                                buffer[(column, row)].symbol() == theme.glyphs.status_bullet
                            })
                        })
                        .expect("TooSmall warning marker");
                    let marker_column = (0..width)
                        .find(|&column| {
                            buffer[(column, notice_row)].symbol() == theme.glyphs.status_bullet
                        })
                        .expect("TooSmall warning marker column");
                    let separator_column = marker_column
                        + u16::try_from(crate::terminal_text::display_width(
                            theme.glyphs.status_bullet,
                        ))
                        .expect("glyph width is bounded by the frame");
                    assert_eq!(
                        buffer[(separator_column, notice_row)].fg,
                        theme.color(Role::TextPrimary),
                        "the warning role must end at the marker glyph"
                    );
                    for column in 0..width {
                        let cell = &buffer[(column, notice_row)];
                        if cell.symbol().trim().is_empty() {
                            continue;
                        }
                        let role = if cell.symbol() == theme.glyphs.status_bullet {
                            Role::StatusWarning
                        } else {
                            Role::TextPrimary
                        };
                        assert_eq!(
                            cell.fg,
                            theme.color(role),
                            "{width}x{height} {scenario:?} misstyled {:?}",
                            cell.symbol()
                        );
                    }
                }
                let status = &rows[usize::from(height - 1)];
                match scenario {
                    Scenario::Resize => assert!(
                        without_cell_padding(status).contains(&without_cell_padding(&resize_exit)),
                        "{width}x{height} resize lost its exit hint: {status:?}"
                    ),
                    Scenario::Ordinary | Scenario::Help => assert!(
                        without_cell_padding(status).contains(&without_cell_padding(&exit_hint)),
                        "{width}x{height} {scenario:?} lost the available exit hint: {status:?}"
                    ),
                }
            }
        }
    }
}

fn plain(c: char) -> PrefixInput {
    PrefixInput::Key(KeyEvent::press(KeyCode::Char(c), Modifiers::NONE))
}

fn ctrl_b() -> PrefixInput {
    PrefixInput::Key(KeyEvent::press(KeyCode::Char('b'), Modifiers::CTRL))
}

fn prefix(view: &mut ClientView, input: PrefixInput) {
    commit(view, ctrl_b());
    commit(view, input);
}

fn commit(view: &mut ClientView, input: PrefixInput) {
    let transition = view.decide(input);
    view.commit(transition, SessionActionDisposition::KeepNavigator);
}

#[test]
fn wide_render_exposes_the_business_sidebar_and_unavailable_workbench_truth() {
    let view = ClientView::new(120);
    let rendered = rendered_text(120, 40, &view, Locale::EnUs);

    assert!(rendered.contains("ORCHBEE"));
    assert!(rendered.contains("Projects"));
    assert!(rendered.contains("WORKBENCH"));
    assert!(rendered.contains("Terminal unavailable"));
    assert!(rendered.contains("No Orchbee session is connected."));
    assert!(!rendered.contains("Ctrl+b · shortcuts"));
}

#[test]
fn unconnected_guidance_remains_readable_beside_the_overlay_sidebar_in_both_locales() {
    for width in [70, 80] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let mut view = ClientView::new(width);
            prefix(&mut view, plain('b'));
            let areas = crate::tui_layout::project_shell_areas(
                ratatui::layout::Rect::new(0, 0, width, 24),
                view.sidebar_surface(),
            );
            let sidebar = areas.sidebar.expect("the explicit sidebar is visible");
            assert_eq!(sidebar.layer, crate::tui_layout::SidebarLayer::Overlay);
            let mut terminal = infallible(Terminal::new(TestBackend::new(width, 24)));
            infallible(terminal.draw(|frame| {
                render_shell(
                    frame,
                    &view,
                    locale,
                    &crate::tui_theme::Theme::rgb_test_dark(),
                );
            }));
            let buffer = terminal.backend().buffer();
            let exposed: String = (1..23)
                .flat_map(|row| {
                    (sidebar.area.right() + 1..width - 1)
                        .map(move |column| buffer[(column, row)].symbol())
                })
                .flat_map(str::chars)
                .filter(|character| !character.is_whitespace())
                .collect();
            let header: String = (sidebar.area.right()..width)
                .map(|column| buffer[(column, 0)].symbol())
                .collect();
            assert!(
                without_cell_padding(&header)
                    .contains(&without_cell_padding(locale.shell().workbench_title))
            );
            for fact in [
                locale.shell().shell_only_detail,
                locale.shell().no_client_session,
            ] {
                let complete: String = fact.chars().filter(|c| !c.is_whitespace()).collect();
                assert!(exposed.contains(&complete), "{width} {locale:?}: {fact}");
            }
            assert_eq!(
                crate::tui_layout::project_shell_areas(
                    ratatui::layout::Rect::new(0, 0, width, 24),
                    view.sidebar_surface(),
                ),
                areas,
                "guidance paint must not reserve or resize terminal geometry",
            );
        }
    }
}

#[test]
fn narrow_render_hides_then_overlays_the_business_sidebar() {
    let mut view = ClientView::new(70);
    let hidden = rendered_text(70, 24, &view, Locale::EnUs);
    assert!(!hidden.contains("Projects"));
    assert!(hidden.contains("WORKBENCH"));

    prefix(&mut view, plain('b'));
    let overlaid = rendered_text(70, 24, &view, Locale::EnUs);
    assert!(overlaid.contains("Projects"));
    assert!(overlaid.contains("Backend unavailable"));
}

#[test]
fn renders_visible_prefix_and_localized_shortcut_overlays() {
    let mut view = ClientView::new(120);
    commit(&mut view, ctrl_b());
    let prefix_mode = rendered_text(120, 32, &view, Locale::ZhCn);
    assert!(without_cell_padding(&prefix_mode).contains("前缀Ctrl+b"));

    commit(&mut view, plain('?'));
    for character in "关闭帮助".chars() {
        commit(&mut view, plain(character));
    }
    let filtered_help = rendered_text(120, 32, &view, Locale::ZhCn);
    let filtered_compact = without_cell_padding(&filtered_help);
    assert!(filtered_help.contains("Ctrl+b ?"));
    assert!(filtered_compact.contains("Ctrl+b?关闭帮助"));
    assert!(filtered_compact.contains("其他"));

    commit(
        &mut view,
        PrefixInput::Key(KeyEvent::press(KeyCode::Char('u'), Modifiers::CTRL)),
    );
    assert!(view.help_filter().is_empty());
    let help = rendered_text(120, 32, &view, Locale::ZhCn);
    let compact_help = without_cell_padding(&help);
    assert!(compact_help.contains("键盘快捷键"));
    assert!(help.contains("Ctrl+b c"));
    assert!(help.contains("Ctrl+b ,"));

    // The composed registry paginates whole groups at this size: the
    // literal-prefix row sits on a later page. Page forward with the
    // documented `Ctrl+B PageDown` binding until it renders.
    let mut paged = String::new();
    for _ in 0..8 {
        if paged.contains("Ctrl+b Ctrl+b") {
            break;
        }
        commit(&mut view, ctrl_b());
        commit(
            &mut view,
            PrefixInput::Key(KeyEvent::press(KeyCode::PageDown, Modifiers::NONE)),
        );
        paged = rendered_text(120, 32, &view, Locale::ZhCn);
    }
    assert!(
        paged.contains("Ctrl+b Ctrl+b"),
        "the literal-prefix row must render on a reachable help page"
    );
}

#[test]
fn renders_backend_action_results_as_no_command_sent() {
    let mut view = ClientView::new(120);
    view.report_backend_unavailable(BackendAction::CreateTab);
    let english = rendered_text(120, 40, &view, Locale::EnUs);
    let chinese = rendered_text(120, 40, &view, Locale::ZhCn);

    assert!(english.contains("Tab creation unavailable · no command was sent"));
    assert!(without_cell_padding(&chinese).contains("无法创建Tab·未发送命令"));
}

#[test]
fn ordinary_state_omits_status_chrome_except_the_too_small_exit_hint() {
    for (width, height) in [(120, 40), (80, 24), (60, 12), (50, 10), (15, 4)] {
        let view = ClientView::new(width);
        let rows = rendered_rows(width, height, &view, Locale::EnUs);
        assert_eq!(rows.len(), height as usize + 1, "{width}x{height}");
        let status = &rows[height as usize - 1];
        let exit_key = view
            .bindings()
            .action_labels_with_glyphs(
                crate::client_view::PrefixAction::Client(crate::client_view::ClientAction::Exit),
                crate::client_view::KeyLabelGlyphs::Unicode,
            )
            .into_iter()
            .next()
            .expect("the effective Exit action remains bound");
        let exit_hint = format!("{exit_key} {}", Locale::EnUs.status().exit);
        assert!(
            without_cell_padding(status).contains(&without_cell_padding(&exit_hint))
                == (width < 20 || height < 5),
            "status row {width}x{height}: {status:?}"
        );
    }
}

#[test]
fn overlays_never_cover_the_status_row() {
    let mut view = ClientView::new(80);
    prefix(&mut view, plain('?'));
    let rows = rendered_rows(80, 24, &view, Locale::EnUs);
    let status = &rows[23];
    assert!(status.contains("unavailable"), "{status:?}");
    for row in &rows[..23] {
        assert!(!row.contains("Backend unavailable ·"), "{row:?}");
    }

    let mut view = ClientView::new(60);
    prefix(&mut view, plain('w'));
    let rows = rendered_rows(60, 12, &view, Locale::ZhCn);
    // The buffer text helper includes wide-glyph continuation cells as spaces.
    let status_text = rows[11].replace(' ', "");
    assert!(
        status_text.contains("快捷键") || status_text.contains("会话"),
        "status row: {:?}",
        rows[11]
    );
}

#[test]
fn degrades_to_minimal_chrome_below_sixty_columns_or_twelve_rows() {
    let view = ClientView::new(50);
    let rendered = rendered_text(50, 10, &view, Locale::EnUs);
    assert!(!rendered.contains("Ctrl+b q · exit"));
    assert!(!rendered.contains("ORCHBEE"));
    assert!(!rendered.contains("WORKBENCH · "));

    // Width alone qualifies as Full; height forces Minimal.
    let view = ClientView::new(120);
    let short = rendered_text(120, 10, &view, Locale::EnUs);
    assert!(!short.contains("Ctrl+b q · exit"));
    assert!(!short.contains("ORCHBEE"));

    let mut toggled = ClientView::new(50);
    prefix(&mut toggled, plain('b'));
    let toggled_render = rendered_text(50, 10, &toggled, Locale::EnUs);
    assert!(!toggled_render.contains("ORCHBEE"));
}

#[test]
fn paints_only_the_localized_too_small_notice_and_the_status_line() {
    let view = ClientView::new(15);
    let exit_key = view
        .bindings()
        .action_labels_with_glyphs(
            crate::client_view::PrefixAction::Client(crate::client_view::ClientAction::Exit),
            crate::client_view::KeyLabelGlyphs::Unicode,
        )
        .into_iter()
        .next()
        .expect("the effective Exit action remains bound");
    let english = rendered_rows(15, 4, &view, Locale::EnUs);
    assert!(english.iter().any(|row| row.contains("Too small")));
    let english_exit = format!("{exit_key} {}", Locale::EnUs.status().exit);
    assert!(
        without_cell_padding(&english[3]).contains(&without_cell_padding(&english_exit)),
        "{:?}",
        english[3]
    );
    assert!(!english.iter().any(|row| row.contains("WORKBENCH")));

    let chinese = rendered_rows(15, 4, &view, Locale::ZhCn);
    assert!(
        chinese
            .iter()
            .any(|row| without_cell_padding(row).contains("窗口太小"))
    );
    let chinese_exit = format!("{exit_key} {}", Locale::ZhCn.status().exit);
    assert!(
        without_cell_padding(&chinese[3]).contains(&without_cell_padding(&chinese_exit)),
        "{:?}",
        chinese[3]
    );

    // Width below 20 alone also triggers the too-small surface.
    let narrow = rendered_rows(19, 30, &view, Locale::EnUs);
    assert!(narrow.iter().any(|row| row.contains("Too small")));
}

fn buffer_rows(terminal: &Terminal<TestBackend>, width: u16, height: u16) -> Vec<String> {
    let buffer = terminal.backend().buffer();
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect()
}

#[test]
fn sidebar_status_truncates_with_ellipsis_on_grapheme_boundaries() {
    let theme = crate::tui_theme::Theme::rgb_test_dark();
    let text = Locale::EnUs.shell();
    let mut terminal = infallible(Terminal::new(TestBackend::new(30, 12)));
    let sidebar = super::SidebarArea {
        area: ratatui::layout::Rect::new(0, 0, 30, 12),
        layer: super::SidebarLayer::Docked,
    };
    infallible(terminal.draw(|frame| {
        super::render_sidebar(
            frame,
            sidebar,
            text,
            super::SiteStatus {
                text: "Tab 1 · Ready · controlling",
                role: crate::tui_theme::Role::StatusSuccess,
            },
            None,
            &theme,
        );
    }));
    let rows = buffer_rows(&terminal, 30, 12);
    let joined = rows.concat();
    // The 29-cell status exceeds the 26-cell inner width: it must end with
    // the ellipsis marker instead of a half-clipped word.
    assert!(joined.contains('…'), "no ellipsis marker: {rows:?}");
    assert!(!joined.contains("controlling"), "status not cut: {rows:?}");
    assert!(joined.contains("contr"), "{rows:?}");
}

#[test]
fn zoom_context_and_restore_hint_are_projected_separately_in_both_locales() {
    for (locale, context, hint) in [
        (Locale::EnUs, "Zoomed", "Ctrl+b z restore"),
        (Locale::ZhCn, "已放大", "Ctrl+b z 恢复"),
    ] {
        let mut terminal = infallible(Terminal::new(TestBackend::new(120, 1)));
        let view = ClientView::new(120);
        infallible(terminal.draw(|frame| {
            super::render_status(
                frame,
                frame.area(),
                &view,
                None,
                locale,
                "terminal · Ready · controlling",
                None,
                true,
                false,
                false,
                ViewportTier::Full,
                &crate::tui_theme::Theme::rgb_test_dark(),
                None,
            );
        }));
        let text = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        // Cell padding splits wide glyphs — compare on collapsed text.
        assert!(
            without_cell_padding(&text).contains(&without_cell_padding(context)),
            "{locale:?} missing context: {text:?}"
        );
        assert!(
            without_cell_padding(&text).contains(&without_cell_padding(hint)),
            "{locale:?} missing effective restore hint: {text:?}"
        );
    }
}

// ===== Retained-history pane surface =====

use std::num::NonZeroU64;

use orchbee_terminal_protocol::{
    AttachmentId, HostEpoch, TerminalFrameRoute, TerminalInteraction, TerminalPixelGeometry,
    TerminalTextCell, TerminalTextCursor, TerminalTextFrame, TerminalTextRow, TerminalTextStyle,
};
use ratatui::layout::Rect;

use crate::terminal_session::NativeViewportSurface;

#[cfg(windows)]
#[test]
fn keyboard_copy_preserves_first_and_last_served_rows_without_history_chrome() {
    for width in [40, 80, 120] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for theme in [
                crate::tui_theme::Theme::terminal_native(),
                crate::tui_theme::Theme::rgb_test_dark(),
                crate::tui_theme::Theme::one_light(),
                crate::tui_theme::Theme::one_dark(),
            ] {
                let first = "F".repeat(usize::from(width));
                let last = "L".repeat(usize::from(width));
                let window = history_window(&[first.as_str(), "middle", last.as_str()], width);
                let mut terminal = infallible(Terminal::new(TestBackend::new(width, 3)));
                infallible(terminal.draw(|frame| {
                    super::viewport::render_selected(
                        frame,
                        Rect::new(0, 0, width, 3),
                        NativeViewportSurface::Window {
                            window: &window,
                            position: Some((42, 200)),
                        },
                        None,
                        false,
                        locale,
                        &theme,
                        None,
                        true,
                    )
                }));
                let rows = buffer_rows(&terminal, width, 3);
                assert_eq!(rows[0], first);
                assert_eq!(rows[2], last);
            }
        }
    }
}

/// A styled row window already fenced by the viewport core.
fn history_window(
    texts: &[&str],
    columns: u16,
) -> crate::terminal_session::terminal_viewport::ViewportWindow {
    use orchbee_terminal_protocol::{RowWindowDetail, TerminalRowWindowRoute};
    crate::terminal_session::terminal_viewport::ViewportWindow {
        graphics_present: false,
        route: TerminalRowWindowRoute {
            attachment_id: AttachmentId::from_bytes([7; 16]).unwrap(),
            host_epoch: HostEpoch::from_bytes([9; 16]).unwrap(),
            sequence: NonZeroU64::MIN,
            revision: 1,
            generation: NonZeroU64::MIN,
            total_rows: 200,
            start_row: 42,
            columns,
            row_count: texts.len() as u16,
            detail: RowWindowDetail::Render,
        },
        foreground: 0xffffff,
        background: 0,
        rows: texts
            .iter()
            .enumerate()
            .map(|(index, text)| TerminalTextRow {
                prompt_class: 0,
                index: index as u16,
                flags: 0,
                cells: text
                    .chars()
                    .map(|ch| TerminalTextCell {
                        text: ch.to_string().into(),
                        width: 0,
                        style: TerminalTextStyle {
                            foreground: 0x1000000,
                            background: 0x1000000,
                            underline_color: 0x1000000,
                            flags: 0,
                            underline: 0,
                        },
                    })
                    .collect(),
            })
            .collect(),
    }
}

fn render_viewport(
    width: u16,
    height: u16,
    surface: NativeViewportSurface<'_>,
    live_frame: Option<&TerminalTextFrame>,
    locale: Locale,
) -> Vec<String> {
    let theme = crate::tui_theme::Theme::rgb_test_dark();
    let mut terminal = infallible(Terminal::new(TestBackend::new(width, height)));
    infallible(terminal.draw(|frame| {
        super::viewport::render(
            frame,
            Rect::new(0, 0, width, height),
            surface,
            live_frame,
            false,
            locale,
            &theme,
        );
    }));
    buffer_rows(&terminal, width, height)
}

// A validated window paints its retained rows under the mode badge, with
// the position readout and the persistent return hint on the bottom line.
#[test]
fn history_window_paints_rows_position_and_hint() {
    let window = history_window(&["row-one", "row-two"], 40);
    let rows = render_viewport(
        40,
        8,
        NativeViewportSurface::Window {
            window: &window,
            position: Some((42, 200)),
        },
        None,
        Locale::EnUs,
    );
    assert!(rows[0].contains("row-one"), "{rows:?}");
    assert!(
        rows[0].contains("HISTORY"),
        "badge on the first row: {rows:?}"
    );
    assert!(rows[1].contains("row-two"), "{rows:?}");
    assert!(rows[7].contains("Row 42 of 200"), "{rows:?}");
    assert!(rows[7].contains("Esc back to live"), "{rows:?}");
}

// The bottom line and indicators localize with the session locale.
#[test]
fn history_window_bottom_line_localizes() {
    let window = history_window(&["row"], 40);
    let rows = render_viewport(
        40,
        8,
        NativeViewportSurface::Window {
            window: &window,
            position: Some((42, 200)),
        },
        None,
        Locale::ZhCn,
    );
    let bottom = without_cell_padding(&rows[7]);
    assert!(bottom.contains("第42行·共200行"), "{rows:?}");
    assert!(bottom.contains("Esc返回实时"), "{rows:?}");
    assert!(without_cell_padding(&rows[0]).contains("历史"), "{rows:?}");
}

// A narrow leaf degrades on grapheme boundaries: the return hint keeps
// priority while the position truncates first.
#[test]
fn history_bottom_line_hint_keeps_priority_when_narrow() {
    let window = history_window(&["row"], 18);
    let rows = render_viewport(
        18,
        6,
        NativeViewportSurface::Window {
            window: &window,
            position: Some((42, 200)),
        },
        None,
        Locale::EnUs,
    );
    assert!(
        rows[5].contains("Esc back to live"),
        "hint survives the position: {rows:?}"
    );
    assert!(
        !rows[5].contains("Row 42"),
        "the position yields first: {rows:?}"
    );
}

// `AwaitingWindow` and `Recovering` clear the leaf and paint their own
// indicator — no position without a validated window, never stale rows.
#[test]
fn history_indicators_clear_and_show_without_position() {
    let theme = crate::tui_theme::Theme::rgb_test_dark();
    for surface in [
        NativeViewportSurface::AwaitingWindow,
        NativeViewportSurface::Recovering,
    ] {
        let mut terminal = infallible(Terminal::new(TestBackend::new(40, 8)));
        // Stale content under the leaf: the surface must clear it.
        infallible(terminal.draw(|frame| {
            frame.buffer_mut().set_stringn(
                0,
                0,
                "stale-live-content",
                17,
                ratatui::style::Style::new(),
            );
        }));
        infallible(terminal.draw(|frame| {
            super::viewport::render(
                frame,
                Rect::new(0, 0, 40, 8),
                surface,
                None,
                false,
                Locale::EnUs,
                &theme,
            );
        }));
        let rows = buffer_rows(&terminal, 40, 8);
        let joined = rows.concat();
        assert!(
            !joined.contains("stale-live-content"),
            "indicator surface left stale content: {rows:?}"
        );
        assert!(rows[7].contains("Esc back to live"), "{rows:?}");
        assert!(!joined.contains("Row"), "no position shown: {rows:?}");
        let badge = match surface {
            NativeViewportSurface::AwaitingWindow => "Loading history…",
            NativeViewportSurface::Recovering => "Returning to live…",
            _ => unreachable!(),
        };
        assert!(joined.contains(badge), "{rows:?}");
    }
}

// `Entering` keeps the last verified live frame beneath the loading badge.
#[test]
fn history_entering_keeps_the_live_frame() {
    let live = TerminalTextFrame {
        route: TerminalFrameRoute {
            attachment_id: AttachmentId::from_bytes([7; 16]).expect("attachment"),
            host_epoch: HostEpoch::from_bytes([9; 16]).expect("epoch"),
            sequence: NonZeroU64::new(1).expect("sequence"),
            generation: NonZeroU64::new(1).expect("generation"),
            revision: 1,
            base_revision: None,
        },
        geometry: TerminalPixelGeometry {
            cell_pixel_height: 16,
            cell_pixel_width: 8,
            columns: 40,
            rows: 8,
        },
        cursor: TerminalTextCursor {
            column: 0,
            row: 0,
            visible: false,
        },
        alternate_screen: false,
        interaction: TerminalInteraction::default(),
        foreground: 0,
        background: 0,
        theme_revision: 1,
        scrollback_rows: 9,
        progress: None,
        graphics_present: None,
        command_completion: None,
        title: None,
        current_directory: None,
        rows: vec![TerminalTextRow {
            prompt_class: 0,
            index: 0,
            flags: 0,
            cells: "live-tail"
                .chars()
                .map(|c| TerminalTextCell {
                    text: c.to_string().into(),
                    style: TerminalTextStyle {
                        foreground: 0,
                        background: 0,
                        underline_color: 0,
                        flags: 0,
                        underline: 0,
                    },
                    width: 0,
                })
                .collect(),
        }],
    };
    let rows = render_viewport(
        40,
        8,
        NativeViewportSurface::Entering,
        Some(&live),
        Locale::EnUs,
    );
    let joined = rows.concat();
    assert!(
        joined.contains("live-tail"),
        "live frame preserved: {rows:?}"
    );
    assert!(joined.contains("Loading history…"), "{rows:?}");
    assert!(rows[7].contains("Esc back to live"), "{rows:?}");
}

#[test]
fn history_chrome_clears_child_styles_text_and_crossing_wide_cells() {
    use ratatui::style::Modifier;
    let line = "X".repeat(40);
    let mut window = history_window(&[&line, &line, &line], 40);
    for row in &mut window.rows {
        for cell in &mut row.cells {
            cell.style.flags = 1 | 2 | 16 | 32 | 64;
            cell.style.underline = 1;
            cell.style.background = 0x123456;
        }
    }
    // The nine-column English badge begins at31, inside this wide glyph.
    window.rows[0].cells[30].text = "界".into();
    window.rows[0].cells[30].width = 1;
    window.rows[0].cells[31].text = "".into();
    window.rows[0].cells[31].width = 2;
    let theme = crate::tui_theme::Theme::rgb_test_dark();
    let mut terminal = infallible(Terminal::new(TestBackend::new(40, 3)));
    infallible(terminal.draw(|frame| {
        super::viewport::render(
            frame,
            Rect::new(0, 0, 40, 3),
            NativeViewportSurface::Window {
                window: &window,
                position: Some((42, 200)),
            },
            None,
            false,
            Locale::EnUs,
            &theme,
        );
    }));
    let buffer = terminal.backend().buffer();
    assert!(buffer[(0, 0)].modifier.contains(Modifier::HIDDEN));
    assert!(buffer[(0, 1)].modifier.contains(Modifier::HIDDEN));
    assert_eq!(buffer[(30, 0)].symbol(), " ");
    let leaked = Modifier::BOLD
        | Modifier::ITALIC
        | Modifier::REVERSED
        | Modifier::HIDDEN
        | Modifier::CROSSED_OUT
        | Modifier::UNDERLINED;
    for x in 31..40 {
        assert!(!buffer[(x, 0)].modifier.intersects(leaked));
    }
    for x in 0..40 {
        assert!(!buffer[(x, 2)].modifier.intersects(leaked));
    }
    for x in 14..23 {
        assert_eq!(buffer[(x, 2)].symbol(), " ");
    }
    let rows = buffer_rows(&terminal, 40, 3);
    assert!(rows[0].contains("HISTORY"));
    assert!(rows[2].contains("Row 42 of 200"));
    assert!(rows[2].contains("Esc back to live"));
}

#[test]
fn history_preserves_styles_defaults_wide_cells_and_sanitization() {
    use ratatui::style::{Color, Modifier};
    let mut window = history_window(&["header", "ABCDE"], 8);
    window.foreground = 0x010203;
    window.background = 0x040506;
    window.rows[1].cells[0].style = TerminalTextStyle {
        foreground: 0x123456,
        background: 0xabcdef,
        underline_color: 0x1000000,
        flags: 3,
        underline: 1,
    };
    window.rows[1].cells[1].text = "界".into();
    window.rows[1].cells[1].width = 1;
    window.rows[1].cells[2].text = "".into();
    window.rows[1].cells[2].width = 2;
    window.rows[1].cells[3].text = "\x1b[31m".into();
    let theme = crate::tui_theme::Theme::rgb_test_dark();
    let mut terminal = infallible(Terminal::new(TestBackend::new(40, 8)));
    infallible(terminal.draw(|frame| {
        super::viewport::render(
            frame,
            Rect::new(0, 0, 40, 8),
            NativeViewportSurface::Window {
                window: &window,
                position: None,
            },
            None,
            true,
            Locale::EnUs,
            &theme,
        )
    }));
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer[(0, 1)].fg, Color::Rgb(0x12, 0x34, 0x56));
    assert_eq!(buffer[(0, 1)].bg, Color::Rgb(0xab, 0xcd, 0xef));
    assert!(
        buffer[(0, 1)]
            .modifier
            .contains(Modifier::BOLD | Modifier::ITALIC | Modifier::UNDERLINED)
    );
    assert_eq!(buffer[(1, 1)].symbol(), "界");
    assert_eq!(buffer[(3, 1)].symbol(), "�");
    assert_eq!(
        buffer[(4, 1)].fg,
        theme.color(crate::tui_theme::Role::TextPrimary)
    );
    assert_eq!(
        buffer[(4, 1)].bg,
        theme.color(crate::tui_theme::Role::SurfaceCanvas)
    );
    let clipped = render_viewport(
        2,
        4,
        NativeViewportSurface::Window {
            window: &window,
            position: None,
        },
        None,
        Locale::EnUs,
    );
    assert_eq!(clipped[1], "A ");
}

#[cfg(windows)]
#[test]
fn popup_draw_uses_an_exact_shared_close_and_keeps_program_mouse_in_the_body() {
    use crate::{
        terminal_input::{
            MouseAction, MouseButton, MouseEvent, TerminalInputPort, TerminalInputUnavailable,
        },
        terminal_session::PopupView,
        tui_pointer::{
            HitMapBuilder, HitTarget, InteractionTarget, MotionObservability, PointerOutcome,
            PointerState, PrimaryContract,
        },
        tui_theme::{GlyphSet, Theme},
    };
    use orchbee_workbench_protocol::{PluginPopupSize, WorkbenchRequestId};
    use ratatui::{layout::Rect, widgets::Paragraph};
    struct Port(Vec<(WorkbenchRequestId, Rect)>);
    impl TerminalInputPort for Port {
        fn forward(&mut self, _: &[u8]) -> Result<(), TerminalInputUnavailable> {
            panic!("popup paint leaked input");
        }
        fn popup_mouse(&mut self, id: &WorkbenchRequestId, rect: Rect, _: MouseEvent) {
            self.0.push((id.clone(), rect));
        }
    }
    let event = |column, row, action, button| MouseEvent {
        column,
        row,
        action,
        button,
        mods: Modifiers::NONE,
    };
    for glyphs in [GlyphSet::UTF8, GlyphSet::ASCII] {
        for closing in [false, true] {
            let id = WorkbenchRequestId::parse(&format!("{:032x}", 1)).unwrap();
            let popup = PopupView {
                id: &id,
                title: "Dashboard",
                projection: None,
                width: Some(PluginPopupSize::Cells(20)),
                height: Some(PluginPopupSize::Cells(8)),
                closing,
            };
            let theme = Theme::rgb_test_dark().with_glyphs(glyphs);
            let paint = |pointer: &PointerState| {
                let mut terminal = infallible(Terminal::new(TestBackend::new(40, 12)));
                let mut hits = HitMapBuilder::with_pointer(pointer.projection());
                infallible(terminal.draw(|frame| {
                    frame.render_widget(
                        Paragraph::new("XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX\n".repeat(12)),
                        frame.area(),
                    );
                    super::render_popup(
                        frame,
                        Rect::new(0, 0, 40, 12),
                        &popup,
                        Locale::EnUs,
                        &theme,
                        &mut hits,
                    );
                }));
                (terminal, hits.finish())
            };
            let mut pointer = PointerState::default();
            pointer.set_capture(true);
            pointer.set_motion_observability(MotionObservability::AnyMotion);
            let (terminal, hits) = paint(&pointer);
            let buffer = terminal.backend().buffer();
            assert_eq!(
                buffer[(11, 3)].symbol(),
                Locale::EnUs
                    .plugin_popup_pending(closing)
                    .chars()
                    .next()
                    .unwrap()
                    .to_string(),
            );
            assert_ne!(buffer[(11, 8)].symbol(), "X");
            assert_eq!(buffer[(9, 3)].symbol(), "X");
            assert_eq!(buffer[(26, 2)].symbol(), "[");
            assert_eq!(buffer[(27, 2)].symbol(), "x");
            assert_eq!(buffer[(28, 2)].symbol(), "]");
            assert_eq!(
                buffer[(27, 2)].fg,
                theme.color(if closing {
                    Role::TextDisabled
                } else {
                    Role::TextSecondary
                })
            );
            assert_eq!(
                (0..40)
                    .flat_map(|x| (0..12).map(move |y| (x, y)))
                    .filter(|&(x, y)| matches!(
                        hits.hit_target_at(x, y),
                        Some(HitTarget::PopupClose(target)) if target.id() == &id
                    ))
                    .count(),
                3
            );
            assert!(!matches!(
                hits.hit_target_at(25, 2),
                Some(HitTarget::PopupClose(_))
            ));
            assert!(!matches!(
                hits.hit_target_at(29, 2),
                Some(HitTarget::PopupClose(_))
            ));

            pointer.publish(hits);
            let mut port = Port(Vec::new());
            for action in [MouseAction::Press, MouseAction::Release] {
                assert_eq!(
                    pointer.popup_event(&mut port, event(11, 3, action, MouseButton::Left),),
                    PointerOutcome::Handled
                );
            }
            assert_eq!(
                port.0,
                [
                    (id.clone(), Rect::new(11, 3, 18, 6)),
                    (id.clone(), Rect::new(11, 3, 18, 6)),
                ]
            );

            if closing {
                assert_eq!(
                    pointer.popup_event(
                        &mut port,
                        event(27, 2, MouseAction::Press, MouseButton::Left),
                    ),
                    PointerOutcome::Handled
                );
                continue;
            }

            assert!(matches!(
                pointer.popup_event(
                    &mut port,
                    event(27, 2, MouseAction::Motion, MouseButton::None),
                ),
                PointerOutcome::IdleMotion { changed: true }
            ));
            let (mut hovered, hits) = paint(&pointer);
            assert_eq!(
                hovered.backend().buffer()[(27, 2)].bg,
                theme.color(Role::SurfaceHover)
            );
            if glyphs == GlyphSet::UTF8 {
                crate::tui_render::test_capture::capture_screen(
                    &mut hovered,
                    "alignment-popup-close-hover-en-us-40x12-direct-renderer-fixture",
                    Locale::EnUs,
                    &theme,
                );
            }
            pointer.publish(hits);
            assert_eq!(
                pointer.popup_event(
                    &mut port,
                    event(27, 2, MouseAction::Press, MouseButton::Left),
                ),
                PointerOutcome::Handled
            );
            let (pressed, hits) = paint(&pointer);
            assert_eq!(
                pressed.backend().buffer()[(27, 2)].bg,
                theme.color(Role::SurfacePressed)
            );
            pointer.publish(hits);
            assert!(matches!(
                pointer.popup_event(
                    &mut port,
                    event(27, 2, MouseAction::Release, MouseButton::Left),
                ),
                PointerOutcome::Interaction {
                    target: InteractionTarget::PopupClose(target),
                    contract: PrimaryContract::ActivateOnRelease,
                } if target.id() == &id
            ));
            assert_eq!(
                port.0.len(),
                2,
                "native close never enters the program lane"
            );
        }
    }
}

#[cfg(windows)]
#[test]
fn popup_chrome_uses_the_overlay_pair_and_keeps_program_colors_independent() {
    use crate::{
        terminal_projection::TerminalProjection,
        terminal_session::PopupView,
        tui_pointer::HitMapBuilder,
        tui_theme::{ColorLevel, Theme},
    };
    use orchbee_workbench_protocol::{
        PluginPopupSize, TerminalDisplayAppearance, UserPreferencesValues, WorkbenchRequestId,
    };
    use ratatui::style::{Color, Modifier};

    let mut source = orchbee_terminal_protocol::decode_terminal_text_frame(include_bytes!(
        "../../../../packages/terminal-protocol/fixtures/terminal-frame-v0.0.3.bin"
    ))
    .unwrap();
    source.foreground = 0x141414;
    source.background = 0xfcfcfc;
    source.cursor.visible = false;
    source.rows.truncate(1);
    source.rows[0].index = 0;
    source.rows[0].cells.truncate(1);
    let cell = &mut source.rows[0].cells[0];
    cell.text = "P".into();
    cell.width = 0;
    cell.style.foreground = 0x123456;
    cell.style.background = 0x654321;
    cell.style.flags = 0;
    let mut projection =
        TerminalProjection::new(source.route.attachment_id, source.route.host_epoch);
    projection.install_frame_for_test(source);
    let id = WorkbenchRequestId::parse(&format!("{:032x}", 1)).unwrap();
    for base in [
        Theme::one_light(),
        Theme::one_dark(),
        Theme::terminal_native(),
    ] {
        for level in [
            ColorLevel::Truecolor,
            ColorLevel::Ansi16,
            ColorLevel::Monochrome,
        ] {
            let theme = base
                .with_terminal_preferences(&UserPreferencesValues {
                    terminal_display_theme: TerminalDisplayAppearance::Host,
                    ..UserPreferencesValues::default()
                })
                .quantize(level);
            for (locale, title) in [(Locale::EnUs, "Popup"), (Locale::ZhCn, "独立终端")] {
                for (width, height) in [(80, 24), (135, 38)] {
                    for ready in [false, true] {
                        let popup = PopupView {
                            id: &id,
                            title,
                            projection: ready.then_some(&projection),
                            width: Some(PluginPopupSize::Cells(30)),
                            height: Some(PluginPopupSize::Cells(8)),
                            closing: false,
                        };
                        let area = Rect::new(0, 0, width, height);
                        let (outer, inner) = crate::terminal_session::popup_geometry(
                            popup.width,
                            popup.height,
                            area,
                        )
                        .unwrap();
                        let mut terminal =
                            infallible(Terminal::new(TestBackend::new(width, height)));
                        let mut hits = HitMapBuilder::new();
                        infallible(terminal.draw(|frame| {
                            super::render_popup(frame, area, &popup, locale, &theme, &mut hits)
                        }));
                        let buffer = terminal.backend().buffer();
                        for y in [outer.y, outer.bottom() - 1] {
                            let mut x = outer.x;
                            while x < outer.right() {
                                let cell = &buffer[(x, y)];
                                // One wide glyph paints its full footprint with the head's style.
                                // Ratatui excludes its reset continuation cells from backend diffs.
                                assert_eq!(
                                    cell.bg,
                                    theme.color(Role::SurfaceOverlay),
                                    "{} {level:?} {locale:?} at ({x}, {y})",
                                    theme.name,
                                );
                                let width = ratatui::buffer::CellWidth::cell_width(cell).max(1);
                                assert!(x + width <= outer.right());
                                x += width;
                            }
                        }
                        for y in outer.y..outer.bottom() {
                            assert_eq!(buffer[(outer.x, y)].bg, theme.color(Role::SurfaceOverlay));
                            assert_eq!(
                                buffer[(outer.right() - 1, y)].bg,
                                theme.color(Role::SurfaceOverlay)
                            );
                        }
                        let title_cell = &buffer[(outer.x + 1, outer.y)];
                        assert_eq!(title_cell.fg, theme.color(Role::TextPrimary));
                        assert!(title_cell.modifier.contains(Modifier::BOLD));
                        assert_eq!(
                            buffer[(outer.x, outer.y + 1)].fg,
                            theme.color(Role::FrameFocused)
                        );
                        let blank = &buffer[(inner.right() - 1, inner.bottom() - 1)];
                        if ready {
                            let body_theme = theme.with_host_terminal_colors(0x141414, 0xfcfcfc);
                            let (foreground, background) = body_theme.terminal_defaults();
                            assert_eq!((blank.fg, blank.bg), (foreground, background));
                            assert_eq!(buffer[(inner.x, inner.y)].symbol(), "P");
                            assert_eq!(buffer[(inner.x, inner.y)].fg, Color::Rgb(0x12, 0x34, 0x56));
                            assert_eq!(buffer[(inner.x, inner.y)].bg, Color::Rgb(0x65, 0x43, 0x21));
                        } else {
                            assert_eq!(blank.bg, theme.color(Role::SurfaceOverlay));
                        }
                    }
                }
            }
        }
    }
}

#[cfg(windows)]
#[test]
fn popup_draw_rejects_incomplete_geometry_before_clearing_or_publishing_targets() {
    use crate::{terminal_session::PopupView, tui_pointer::HitMapBuilder};
    use orchbee_workbench_protocol::WorkbenchRequestId;
    use ratatui::{layout::Rect, widgets::Paragraph};

    let id = WorkbenchRequestId::parse(&format!("{:032x}", 1)).unwrap();
    let popup = PopupView {
        id: &id,
        title: "Dashboard",
        projection: None,
        width: None,
        height: None,
        closing: false,
    };
    let mut terminal = infallible(Terminal::new(TestBackend::new(5, 3)));
    let mut hits = HitMapBuilder::new();
    infallible(terminal.draw(|frame| {
        frame.render_widget(Paragraph::new("XXXXX\nXXXXX\nXXXXX"), frame.area());
        super::render_popup(
            frame,
            Rect::new(0, 0, 5, 3),
            &popup,
            Locale::EnUs,
            &crate::tui_theme::Theme::rgb_test_dark(),
            &mut hits,
        );
    }));
    assert!(
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .all(|cell| cell.symbol() == "X")
    );
    let map = hits.finish();
    assert!((0..5).all(|x| (0..3).all(|y| map.hit_target_at(x, y).is_none())));
}

#[cfg(windows)]
#[test]
fn divider_hover_and_press_style_every_exact_target_cell_without_leaking_to_the_frame() {
    use std::time::Instant;

    use crate::{
        client_view::{ClientView, Modifiers, Notice},
        terminal_input::{
            MouseAction, MouseButton, MouseEvent, TerminalInputPort, TerminalInputUnavailable,
        },
        tui_layout::{DividerSource, DrawnDivider, divide_panes},
        tui_pointer::{HitMapBuilder, MotionObservability, PointerState},
        tui_theme::{Role, Theme},
    };
    use orchbee_workbench_protocol::{
        ClientSplitRatio, PanelOrientation, SiteId, WorkbenchPanelId, WorkbenchPanelSlotId,
        WorkbenchRevision, WorkbenchSplitId, WorkbenchTabId, WorkbenchTerminalContextId,
    };
    use ratatui::{layout::Rect, style::Modifier, widgets::Paragraph};

    struct Port;
    impl TerminalInputPort for Port {
        fn forward(&mut self, _: &[u8]) -> Result<(), TerminalInputUnavailable> {
            Ok(())
        }

        fn pointer_divider_ready(&mut self, _: &DrawnDivider) -> Result<(), Notice> {
            Ok(())
        }
    }

    let panel = Rect::new(0, 0, 11, 5);
    let orientation = PanelOrientation::Horizontal;
    let ratio = ClientSplitRatio::new(0.5).unwrap();
    let divider = divide_panes(panel, orientation, ratio).unwrap().divider;
    let target = DrawnDivider {
        source: DividerSource {
            site: SiteId::parse(&format!("{:032x}", 1)).unwrap(),
            context: WorkbenchTerminalContextId::parse(&format!("{:032x}", 2)).unwrap(),
            tab: WorkbenchTabId::parse(&format!("{:032x}", 3)).unwrap(),
            focus: WorkbenchPanelId::parse(&format!("{:032x}", 4)).unwrap(),
            revision: WorkbenchRevision::new(1),
        },
        viewport: panel,
        split: WorkbenchSplitId::parse(&format!("{:032x}", 5)).unwrap(),
        first: WorkbenchPanelSlotId::parse(&format!("{:032x}", 6)).unwrap(),
        second: WorkbenchPanelSlotId::parse(&format!("{:032x}", 7)).unwrap(),
        orientation,
        ratio,
        area: panel,
        divider,
    };
    let mut published = HitMapBuilder::new();
    published.record_divider(target.clone());
    let mut pointer = PointerState::default();
    pointer.set_capture(true);
    pointer.publish(published.finish());
    pointer.set_motion_observability(MotionObservability::AnyMotion);
    let mut view = ClientView::new(panel.width);
    let mut port = Port;
    let event = |action, button| MouseEvent {
        action,
        button,
        mods: Modifiers::NONE,
        column: divider.x,
        row: divider.y.saturating_add(1),
    };
    pointer.event(
        &mut view,
        &mut port,
        Instant::now(),
        event(MouseAction::Motion, MouseButton::None),
    );

    let theme = Theme::rgb_test_dark();
    let paint = |pointer: &PointerState| {
        let mut terminal = infallible(Terminal::new(TestBackend::new(panel.width, panel.height)));
        let mut hits = HitMapBuilder::with_pointer(pointer.projection());
        infallible(terminal.draw(|frame| {
            frame.render_widget(
                Paragraph::new("...........\n".repeat(5)).style(theme.style(Role::SurfaceCanvas)),
                frame.area(),
            );
            super::render_dividers(
                frame,
                &[divider],
                panel,
                &theme,
                std::slice::from_ref(&target),
                &mut hits,
            );
        }));
        terminal
    };
    let assert_projection = |terminal: &Terminal<TestBackend>, interaction| {
        let expected = theme.content_on_surface(
            Role::SurfaceCanvas,
            Some(interaction),
            Some(Role::FrameDivider),
            theme.style(Role::FrameDivider),
        );
        let buffer = terminal.backend().buffer();
        let boundary = theme.content_on_surface(
            Role::SurfaceCanvas,
            None,
            Some(Role::FrameUnfocused),
            theme.style(Role::FrameUnfocused),
        );
        for row in divider.y..divider.bottom() {
            let cell = &buffer[(divider.x, row)];
            assert_eq!(cell.style(), expected, "target cell at row {row}");
            assert!(!cell.modifier.contains(Modifier::BOLD));
        }
        for (column, row) in [
            (divider.x - 1, panel.y),
            (divider.x + 1, panel.y),
            (divider.x - 1, panel.bottom() - 1),
            (divider.x + 1, panel.bottom() - 1),
        ] {
            assert_eq!(
                buffer[(column, row)].style(),
                boundary,
                "interaction leaked to boundary cell ({column}, {row})"
            );
        }
        assert_eq!(buffer[(panel.x, panel.y + 2)].symbol(), ".");
    };

    let hovered = paint(&pointer);
    assert_projection(&hovered, Role::SurfaceHover);
    pointer.event(
        &mut view,
        &mut port,
        Instant::now(),
        event(MouseAction::Press, MouseButton::Left),
    );
    let pressed = paint(&pointer);
    assert_projection(&pressed, Role::SurfacePressed);
}

#[cfg(windows)]
#[test]
fn resize_mode_status_is_persistent_across_tiers_locales_themes_and_pending_feedback() {
    for (width, height) in [(80, 24), (120, 40), (70, 18), (40, 10), (18, 4)] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for theme in [
                crate::tui_theme::Theme::terminal_native(),
                crate::tui_theme::Theme::rgb_test_dark(),
                crate::tui_theme::Theme::one_light(),
                crate::tui_theme::Theme::one_dark(),
            ] {
                let mut view = ClientView::new(width);
                prefix(&mut view, plain('P'));
                let glyphs = match theme.glyphs.mode() {
                    crate::tui_theme::GlyphMode::Unicode => {
                        crate::client_view::KeyLabelGlyphs::Unicode
                    }
                    crate::tui_theme::GlyphMode::Simplified => {
                        crate::client_view::KeyLabelGlyphs::Ascii
                    }
                };
                let resize_exit = view
                    .resize_help_rows_with_glyphs(glyphs)
                    .into_iter()
                    .find(|(_, desc)| *desc == crate::client_view::PrefixDesc::ResizeExit)
                    .map(|(key, _)| key)
                    .expect("Resize retains its local exit binding");
                for pending in [false, true] {
                    view.resize_pending = pending;
                    view.configuration_notice(crate::client_view::Notice::ResizeBusy);
                    let mut terminal = infallible(Terminal::new(TestBackend::new(width, height)));
                    infallible(terminal.draw(|frame| {
                        render_shell(frame, &view, locale, &theme);
                    }));
                    let rendered: String = terminal
                        .backend()
                        .buffer()
                        .content
                        .iter()
                        .map(|cell| cell.symbol())
                        .collect();
                    let indicator = if pending {
                        locale.status().resize_pending
                    } else {
                        locale.status().resize
                    };
                    let resize_exit_hint = format!("{resize_exit} {}", locale.status().exit);
                    let indicator_fits = crate::terminal_text::display_width(indicator)
                        .saturating_add(
                            crate::terminal_text::display_width(theme.glyphs.middle_dot)
                                .saturating_add(2),
                        )
                        .saturating_add(crate::terminal_text::display_width(&resize_exit_hint))
                        <= usize::from(width);
                    assert_eq!(
                        without_cell_padding(&rendered).contains(&without_cell_padding(indicator)),
                        indicator_fits,
                        "{width}x{height}: {rendered}"
                    );
                    assert!(
                        without_cell_padding(&rendered)
                            .contains(&without_cell_padding(&resize_exit_hint)),
                        "{width}x{height}: {rendered}"
                    );
                }
            }
        }
    }
    let mut view = ClientView::new(120);
    prefix(&mut view, plain('P'));
    commit(&mut view, plain('?'));
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let rows = locale.shell().configured_shortcut_rows(&view);
        assert_eq!(rows.len(), 5);
        assert_eq!(rows.last().unwrap().0.trim(), "Esc");
        assert!(rows[0].1.contains('5'));
        assert!(rows[1].1.contains('1'));
        let rendered = rendered_text(120, 40, &view, locale);
        assert!(rendered.contains("Esc"));
        assert!(!rendered.contains("Send to the terminal input port"));
    }
}
