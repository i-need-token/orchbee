use std::convert::Infallible;

use ratatui::{Terminal, backend::TestBackend};

use crate::{
    client_view::{
        BackendAction, ClientView, KeyCode, KeyEvent, Modifiers, PrefixInput,
        SessionActionDisposition,
    },
    localization::Locale,
    tui_layout::ViewportTier,
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
            Locale::EnUs,
            Locale::EnUs.shell(),
            "terminal · Close cleanup blocked",
            Some("Close request was not applied"),
            false,
            false,
            false,
            ViewportTier::Full,
            &crate::tui_theme::Theme::rgb_test_dark(),
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
                        locale,
                        locale.shell(),
                        &status,
                        Some(&notice),
                        false,
                        false,
                        false,
                        ViewportTier::Full,
                        &crate::tui_theme::Theme::one_light(),
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
                    assert!(
                        without_cell_padding(&text)
                            .contains(&without_cell_padding(locale.shell().prefix_idle)),
                        "{locale:?} missing idle hint: {text:?}"
                    );
                }
            }
        }
    }
}

#[cfg(windows)]
#[test]
fn sidebar_status_keeps_a_blank_before_hints_without_attesting_truncated_text() {
    use crate::{
        terminal_text::display_width,
        tui_theme::{GlyphSet, Theme},
    };
    use ratatui::{layout::Rect, style::Style, text::Span};

    for width in [70, 120] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for glyphs in [GlyphSet::UTF8, GlyphSet::ASCII] {
                let theme = Theme::rgb_test_dark().with_glyphs(glyphs);
                let mut view = ClientView::new(width);
                view.machines.available = true;
                assert!(view.machines.enter_sidebar_navigation());
                let hint = locale.sidebar_navigation_hint(width == 120);
                let hint_width = u16::try_from(display_width(&hint)).unwrap();
                let left = Rect::new(0, 0, width - hint_width.max(24), 1);
                let separator = left.right() - 1;
                for (status, fits) in [
                    ("Ready".to_owned(), true),
                    ("x".repeat(usize::from(left.width)), false),
                    (
                        format!("{}界", "x".repeat(usize::from(left.width - 2))),
                        false,
                    ),
                ] {
                    let mut terminal = infallible(Terminal::new(TestBackend::new(width, 1)));
                    let mut visible = None;
                    infallible(terminal.draw(|frame| {
                        visible = super::render_status(
                            frame,
                            frame.area(),
                            &view,
                            locale,
                            locale.shell(),
                            &status,
                            None,
                            false,
                            false,
                            false,
                            ViewportTier::from_size(width, 30),
                            &theme,
                        );
                    }));
                    assert_eq!(
                        visible,
                        fits.then_some(left),
                        "{locale:?} {width}: {status}"
                    );
                    let buffer = terminal.backend().buffer();
                    assert_eq!(buffer[(separator, 0)].symbol(), " ");
                    let mut x = 0;
                    let mut retained = String::new();
                    while x < separator {
                        let symbol = buffer[(x, 0)].symbol();
                        retained.push_str(symbol);
                        x += u16::try_from(display_width(symbol).max(1)).unwrap();
                        assert!(x <= separator, "wide glyph crossed the separator");
                    }
                    if fits {
                        assert_eq!(retained.trim_end(), status);
                    } else {
                        assert!(retained.trim_end().ends_with(glyphs.ellipsis));
                    }
                    let mut x = width - hint_width;
                    for grapheme in Span::raw(&hint).styled_graphemes(Style::default()) {
                        assert_eq!(buffer[(x, 0)].symbol(), grapheme.symbol);
                        x += u16::try_from(display_width(grapheme.symbol)).unwrap();
                    }
                    assert_eq!(x, width);
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
                assert_eq!(area.height, 9);
                assert_eq!(
                    canonical_hits.hit_target_at(left + 2, top + 1),
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
    assert!(rendered.contains("No Site or Client Session is connected."));
    assert!(!rendered.contains("Ctrl+B · shortcuts"));
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
    assert!(without_cell_padding(&prefix_mode).contains("前缀Ctrl+B"));

    commit(&mut view, plain('?'));
    let help = rendered_text(120, 32, &view, Locale::ZhCn);
    let compact_help = without_cell_padding(&help);
    assert!(compact_help.contains("键盘快捷键"));
    assert!(help.contains("Ctrl+B c"));
    assert!(help.contains("Ctrl+B ,"));
    assert!(compact_help.contains("Ctrl+B?关闭此帮助"));
    assert!(compact_help.contains("输入仍会传递到终端"));

    // The composed registry paginates whole groups at this size: the
    // literal-prefix row sits on a later page. Page forward with the
    // documented `Ctrl+B PageDown` binding until it renders.
    let mut paged = String::new();
    for _ in 0..8 {
        if paged.contains("Ctrl+B Ctrl+B") {
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
        paged.contains("Ctrl+B Ctrl+B"),
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
        assert!(
            status.contains("Ctrl+B") == (width < 20 || height < 5),
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
    assert!(rows[11].contains("快捷键") || rows[11].contains("Site"));
}

#[test]
fn degrades_to_minimal_chrome_below_sixty_columns_or_twelve_rows() {
    let view = ClientView::new(50);
    let rendered = rendered_text(50, 10, &view, Locale::EnUs);
    assert!(!rendered.contains("Ctrl+B q · exit"));
    assert!(!rendered.contains("ORCHBEE"));
    assert!(!rendered.contains("WORKBENCH · "));

    // Width alone qualifies as Full; height forces Minimal.
    let view = ClientView::new(120);
    let short = rendered_text(120, 10, &view, Locale::EnUs);
    assert!(!short.contains("Ctrl+B q · exit"));
    assert!(!short.contains("ORCHBEE"));

    let mut toggled = ClientView::new(50);
    prefix(&mut toggled, plain('b'));
    let toggled_render = rendered_text(50, 10, &toggled, Locale::EnUs);
    assert!(!toggled_render.contains("ORCHBEE"));
}

#[test]
fn paints_only_the_localized_too_small_notice_and_the_status_line() {
    let view = ClientView::new(15);
    let english = rendered_rows(15, 4, &view, Locale::EnUs);
    assert!(english.iter().any(|row| row.contains("Too small")));
    assert!(english[3].contains("Ctrl+B q"));
    assert!(!english.iter().any(|row| row.contains("WORKBENCH")));

    let chinese = rendered_rows(15, 4, &view, Locale::ZhCn);
    assert!(
        chinese
            .iter()
            .any(|row| without_cell_padding(row).contains("窗口太小"))
    );
    assert!(chinese[3].contains("Ctrl+B q"));

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
fn help_paginates_wrapped_rows_and_clamps_the_requested_page() {
    let theme = crate::tui_theme::Theme::rgb_test_dark();
    let text = Locale::EnUs.shell();
    let mut terminal = infallible(Terminal::new(TestBackend::new(60, 12)));
    let mut placed = None;

    // Wrapped shortcut rows in an 8-line page budget -> several pages; the
    // requested page 99 clamps to the last one and the footer stays visible.
    infallible(terminal.draw(|frame| {
        placed = super::render_shortcut_help(
            frame,
            frame.area(),
            text,
            &ClientView::new(100),
            99,
            &theme,
        );
    }));
    let (_, page) = placed.expect("help must fit in 60x12");
    assert!(page >= 1, "wrapped help must need more than one page");
    let last = page + 1;
    let rows = buffer_rows(&terminal, 60, 12);
    assert!(rows[9].contains(&format!("{last}/{last}")), "{:?}", rows[9]);
    assert!(rows[8].contains("Ctrl+B ? closes"), "{:?}", rows[8]);
    // The last page holds the trailing "other keys" registry row, whose
    // description wrapped onto an aligned continuation line.
    assert!(rows.iter().any(|row| row.contains("terminal")), "{rows:?}");
    let body = rows[2..8]
        .iter()
        .map(|row| row.trim_matches(&[' ', '│'][..]))
        .collect::<Vec<_>>()
        .join(" ");
    assert!(body.contains("input port"), "{rows:?}");
    // Rows from earlier pages are not drawn.
    assert!(!rows.iter().any(|row| row.contains("Ctrl+B a")));

    infallible(terminal.draw(|frame| {
        placed = super::render_shortcut_help(
            frame,
            frame.area(),
            text,
            &ClientView::new(100),
            0,
            &theme,
        );
    }));
    let (_, page) = placed.expect("help must fit in 60x12");
    assert_eq!(page, 0);
    let rows = buffer_rows(&terminal, 60, 12);
    let entries = text.configured_shortcut_rows(&ClientView::new(100));
    let first = entries.first().unwrap().0.trim();
    assert!(rows.iter().any(|row| row.contains(first)), "{rows:?}");
    assert!(rows[9].contains("1/"), "{:?}", rows[9]);

    // Adding a row may move an existing shortcut to the next page. Every
    // offered chord must still appear in the content of at least one page,
    // and each page must preserve its effective number and exit footer.
    let mut content = Vec::new();
    for requested in 0..last {
        infallible(terminal.draw(|frame| {
            placed = super::render_shortcut_help(
                frame,
                frame.area(),
                text,
                &ClientView::new(100),
                requested,
                &theme,
            );
        }));
        assert_eq!(placed.map(|(_, page)| page), Some(requested));
        let rows = buffer_rows(&terminal, 60, 12);
        assert!(rows[9].contains(&format!("{}/{last}", requested + 1)));
        assert!(rows[8].contains("Ctrl+B ? closes"));
        content.extend_from_slice(&rows[2..8]);
    }
    for (chord, _) in entries {
        assert!(
            content.iter().any(|row| row.contains(chord.trim())),
            "missing shortcut {chord:?}: {content:?}"
        );
    }
}

#[test]
fn help_wraps_long_descriptions_instead_of_clipping_them() {
    let theme = crate::tui_theme::Theme::rgb_test_dark();
    let text = Locale::EnUs.shell();
    // A 46-column overlay leaves ~28 cells for labels: long descriptions
    // must wrap onto continuation lines rather than clip at the right edge.
    let mut terminal = infallible(Terminal::new(TestBackend::new(46, 20)));
    let mut placed = None;
    infallible(terminal.draw(|frame| {
        placed = super::render_shortcut_help(
            frame,
            frame.area(),
            text,
            &ClientView::new(100),
            0,
            &theme,
        );
    }));
    placed.expect("46x20 help must fit with wrapping");
    let rows = buffer_rows(&terminal, 46, 20);
    let mut all_rows = rows.clone();
    infallible(terminal.draw(|frame| {
        placed = super::render_shortcut_help(
            frame,
            frame.area(),
            text,
            &ClientView::new(100),
            99,
            &theme,
        );
    }));
    let (_, last_page) = placed.expect("same geometry must retain help placement");
    for page in 1..=last_page {
        infallible(terminal.draw(|frame| {
            placed = super::render_shortcut_help(
                frame,
                frame.area(),
                text,
                &ClientView::new(100),
                page,
                &theme,
            );
        }));
        assert_eq!(placed.map(|(_, page)| page), Some(page));
        all_rows.extend(buffer_rows(&terminal, 46, 20));
    }
    let joined = all_rows.join("\n");
    // The longest English descriptions appear whole across wrapped lines.
    for word in ["Workspace", "directory", "Project"] {
        assert!(joined.contains(word), "clipped {word}: {joined}");
    }
    // Continuation lines align under the description column: after the left
    // border they begin with the key-column padding and still carry text.
    assert!(
        rows.iter().any(|row| {
            let inner = row.trim_start().trim_start_matches(['│', '|', '+']);
            inner.starts_with("              ") && !inner[14..].trim().is_empty()
        }),
        "no aligned continuation line: {rows:?}"
    );
}

#[test]
fn help_wraps_chinese_descriptions_on_grapheme_boundaries() {
    let theme = crate::tui_theme::Theme::rgb_test_dark();
    let text = Locale::ZhCn.shell();
    let mut terminal = infallible(Terminal::new(TestBackend::new(46, 20)));
    let mut placed = None;
    infallible(terminal.draw(|frame| {
        placed = super::render_shortcut_help(
            frame,
            frame.area(),
            text,
            &ClientView::new(100),
            0,
            &theme,
        );
    }));
    placed.expect("zh help must fit with wrapping");
    let rows = buffer_rows(&terminal, 46, 20);
    // A wide-character description hard-wraps without dropping its tail.
    let tail = rows.concat();
    assert!(tail.contains('页'), "wrapped zh tail lost: {rows:?}");
    assert!(
        rows.iter().any(|row| row.contains("PgUp/PgDn")),
        "paging footer missing: {rows:?}"
    );
}

#[test]
fn wrap_display_refuses_widths_that_cannot_hold_a_grapheme() {
    // Zero budget is unrenderable for real text; empty text needs no cell.
    assert_eq!(super::wrap_display("ab", 0), None);
    assert_eq!(super::wrap_display("", 0), Some(vec![String::new()]));
    // A one-cell line can hold ASCII but never a two-cell CJK grapheme —
    // the honest answer is `None`, not a forged wider line.
    assert_eq!(
        super::wrap_display("ab", 1),
        Some(vec!["a".into(), "b".into()])
    );
    assert_eq!(super::wrap_display("中", 1), None);
    assert_eq!(super::wrap_display("a中", 1), None);
    assert_eq!(super::wrap_display("中", 2), Some(vec!["中".into()]));
    // A combining sequence is one grapheme: it fits a 1-cell line whole.
    assert_eq!(
        super::wrap_display("a\u{301}", 1),
        Some(vec!["a\u{301}".into()])
    );
    // An empty word between spaces still returns at least one line.
    assert_eq!(super::wrap_display("", 5), Some(vec![String::new()]));
}

#[test]
fn help_footer_falls_back_to_the_explicit_short_form_then_refuses() {
    let theme = crate::tui_theme::Theme::rgb_test_dark();
    let text = Locale::EnUs.shell();
    // 46x20: the long dismiss line (56 cells) exceeds the 40-cell inner
    // width, so the localized short form renders — close chord, typing
    // purpose, full paging chord, and page number all still visible.
    let mut terminal = infallible(Terminal::new(TestBackend::new(46, 20)));
    let mut placed = None;
    infallible(terminal.draw(|frame| {
        placed = super::render_shortcut_help(
            frame,
            frame.area(),
            text,
            &ClientView::new(100),
            0,
            &theme,
        );
    }));
    placed.expect("46x20 must fit the short footer");
    let rows = buffer_rows(&terminal, 46, 20).concat();
    assert!(
        rows.contains("Ctrl+B ? closes · typing continues"),
        "{rows}"
    );
    assert!(rows.contains("Ctrl+B PgUp/PgDn turns pages"), "{rows}");

    // 38 columns: even the 34-cell short dismiss line cannot fit the
    // 32-cell inner width, so the overlay refuses — no keyless footer.
    let mut terminal = infallible(Terminal::new(TestBackend::new(38, 12)));
    let mut placed = Some((ratatui::layout::Rect::default(), 0usize));
    infallible(terminal.draw(|frame| {
        placed = super::render_shortcut_help(
            frame,
            frame.area(),
            text,
            &ClientView::new(100),
            0,
            &theme,
        );
    }));
    assert_eq!(
        placed, None,
        "38-wide help must refuse, not clip the footer"
    );
}

#[test]
fn help_paging_line_leads_with_the_page_number() {
    let theme = crate::tui_theme::Theme::rgb_test_dark();
    let text = Locale::EnUs.shell();
    // `page/total` always leads the paging line, ahead of the full paging
    // chord — it can never be clipped away by a right-edge cutoff.
    let mut terminal = infallible(Terminal::new(TestBackend::new(60, 12)));
    infallible(terminal.draw(|frame| {
        super::render_shortcut_help(frame, frame.area(), text, &ClientView::new(100), 0, &theme);
    }));
    let rows = buffer_rows(&terminal, 60, 12);
    let paging = rows
        .iter()
        .find(|row| row.contains("PgUp"))
        .expect("paging footer line missing")
        .trim_start_matches([' ', '│', '|', '+'])
        .to_owned();
    assert!(
        paging.starts_with("1/"),
        "page number not leading: {paging:?}"
    );
    assert!(paging.contains("Ctrl+B PgUp/PgDn"), "{paging:?}");
}

#[test]
fn paginate_help_rows_groups_whole_rows_into_page_budgets() {
    // Uniform one-line rows paginate exactly like the old row division.
    assert_eq!(
        super::paginate_help_rows(&[1; 21], 6),
        Some(vec![(0, 6), (6, 12), (12, 18), (18, 21)])
    );
    assert_eq!(super::paginate_help_rows(&[1; 5], 5), Some(vec![(0, 5)]));
    assert_eq!(super::paginate_help_rows(&[], 4), Some(vec![(0, 0)]));
    // A multi-line row counts every rendered line and is never split.
    assert_eq!(
        super::paginate_help_rows(&[3, 3, 1, 4], 5),
        Some(vec![(0, 1), (1, 3), (3, 4)])
    );
    // A row taller than the page refuses placement instead of clipping.
    assert_eq!(super::paginate_help_rows(&[1, 6, 1], 4), None);
    assert_eq!(super::paginate_help_rows(&[2], 0), None);
}

#[test]
fn help_refuses_placement_that_would_clip_rows_or_footer() {
    let theme = crate::tui_theme::Theme::rgb_test_dark();
    let text = Locale::EnUs.shell();
    let mut terminal = infallible(Terminal::new(TestBackend::new(40, 7)));
    let mut placed = Some((ratatui::layout::Rect::default(), 0usize));

    // A 5-row placed overlay leaves 3 inner rows — only 1 row per page, below
    // the 2-row minimum, so the overlay refuses instead of clipping.
    infallible(terminal.draw(|frame| {
        placed = super::render_shortcut_help(
            frame,
            frame.area(),
            text,
            &ClientView::new(100),
            0,
            &theme,
        );
    }));
    assert_eq!(placed, None);
    let text = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(text.trim().is_empty(), "refused overlay drew: {text:?}");
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
fn the_zoom_badge_rides_the_status_label_in_both_locales() {
    for (locale, needle) in [
        (Locale::EnUs, "zoomed · Ctrl+B z restores"),
        (Locale::ZhCn, "已放大 · Ctrl+B z 恢复"),
    ] {
        let mut terminal = infallible(Terminal::new(TestBackend::new(120, 1)));
        let view = ClientView::new(120);
        infallible(terminal.draw(|frame| {
            super::render_status(
                frame,
                frame.area(),
                &view,
                locale,
                locale.shell(),
                "terminal · Ready · controlling",
                None,
                true,
                false,
                false,
                ViewportTier::Full,
                &crate::tui_theme::Theme::rgb_test_dark(),
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
            without_cell_padding(&text).contains(&without_cell_padding(needle)),
            "{locale:?} missing badge: {text:?}"
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
fn popup_draw_clears_the_underlying_cells_and_records_only_its_painted_close_and_body() {
    use crate::{
        terminal_input::{
            MouseAction, MouseButton, MouseEvent, TerminalInputPort, TerminalInputUnavailable,
        },
        terminal_session::PopupView,
        tui_pointer::{HitMapBuilder, PointerState},
    };
    use orchbee_workbench_protocol::{PluginPopupSize, WorkbenchRequestId};
    use ratatui::{layout::Rect, widgets::Paragraph};
    let mut terminal = infallible(Terminal::new(TestBackend::new(40, 12)));
    let id = WorkbenchRequestId::parse(&format!("{:032x}", 1)).unwrap();
    let popup = PopupView {
        id: &id,
        title: "Dashboard",
        projection: None,
        width: Some(PluginPopupSize::Cells(20)),
        height: Some(PluginPopupSize::Cells(8)),
        closing: true,
    };
    let mut hits = HitMapBuilder::new();
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
            &crate::tui_theme::Theme::rgb_test_dark(),
            &mut hits,
        );
    }));
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer[(11, 3)].symbol(), "C");
    assert_ne!(buffer[(11, 8)].symbol(), "X");
    assert_eq!(buffer[(9, 3)].symbol(), "X");
    assert_eq!(buffer[(27, 2)].symbol(), "×");
    struct Port(Vec<(WorkbenchRequestId, Rect, bool)>);
    impl TerminalInputPort for Port {
        fn forward(&mut self, _: &[u8]) -> Result<(), TerminalInputUnavailable> {
            panic!("popup paint leaked input");
        }
        fn popup_mouse(&mut self, id: &WorkbenchRequestId, rect: Rect, _: MouseEvent, close: bool) {
            self.0.push((id.clone(), rect, close));
        }
    }
    let mut pointer = PointerState::default();
    pointer.set_capture(true);
    pointer.publish(hits.finish());
    let mut port = Port(Vec::new());
    for (column, row, action) in [
        (11, 3, MouseAction::Press),
        (11, 3, MouseAction::Release),
        (27, 2, MouseAction::Press),
    ] {
        pointer.popup_event(
            &mut port,
            MouseEvent {
                column,
                row,
                action,
                button: MouseButton::Left,
                mods: Modifiers::NONE,
            },
        );
    }
    assert_eq!(
        port.0,
        [
            (id.clone(), Rect::new(11, 3, 18, 6), false),
            (id.clone(), Rect::new(11, 3, 18, 6), false),
            (id, Rect::default(), true)
        ]
    );
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
                    assert!(
                        without_cell_padding(&rendered).contains(if locale == Locale::EnUs {
                            "RESIZE"
                        } else {
                            "调整"
                        }),
                        "{width}x{height}: {rendered}"
                    );
                    assert!(rendered.contains("Esc"));
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
