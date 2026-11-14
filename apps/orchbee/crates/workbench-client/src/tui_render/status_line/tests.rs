use ratatui::{Terminal, backend::TestBackend, layout::Rect};

use crate::{
    client_view::{
        ClientAction, ClientActionDisposition, ClientView, KeyCode, KeyEvent, KeyLabelGlyphs,
        LabelStyle, Modifiers, Notice, PrefixAction, PrefixInput,
    },
    localization::{Locale, PersistentStatus, TerminalEnd},
    tui_layout::ViewportTier,
    tui_theme::{GlyphSet, Role, Theme},
};

use super::{StatusFacts, StatusPaint, render};

const CANVAS: &str = "¤";

fn facts(primary: &str) -> StatusFacts<'_> {
    StatusFacts {
        primary,
        primary_shorter: &[],
        primary_ordinary: false,
        primary_notice: false,
        persistent: None,
        secondary_notice: None,
        secondary_shorter: &[],
        zoomed: false,
        panels: None,
        read_only: false,
    }
}

#[test]
fn workspace_and_tab_creation_hints_retain_the_same_effective_action_binding_and_localized_scope() {
    let mut view = ClientView::new(120);
    view.configure_keys(&configured_arrow_help());
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let workspace = super::action_hint(
            &view,
            super::StatusAction::CreateWorkspace,
            locale,
            KeyLabelGlyphs::Unicode,
        )
        .unwrap();
        let tab = super::action_hint(
            &view,
            super::StatusAction::CreateTab,
            locale,
            KeyLabelGlyphs::Unicode,
        )
        .unwrap();
        assert_eq!(workspace.key, tab.key);
        assert_eq!(
            workspace.key,
            crate::terminal_events::keys::tests::host_label("Ctrl+a c")
        );
        assert_eq!(workspace.label, locale.status().create_workspace);
        assert_eq!(tab.label, locale.status().create_tab);
        assert_ne!(workspace.label, tab.label);
    }
}

fn paint(
    view: &ClientView,
    width: u16,
    facts: StatusFacts<'_>,
    theme: &Theme,
) -> (Terminal<TestBackend>, StatusPaint) {
    paint_at_tier_with_locale(view, width, facts, ViewportTier::Full, Locale::EnUs, theme)
}

/// Paints into the shell's chip bounds: one cell in from each end of a canvas-filled row, or the
/// whole row at TooSmall.
fn paint_at_tier_with_locale(
    view: &ClientView,
    width: u16,
    facts: StatusFacts<'_>,
    tier: ViewportTier,
    locale: Locale,
    theme: &Theme,
) -> (Terminal<TestBackend>, StatusPaint) {
    let mut terminal = Terminal::new(TestBackend::new(width, 1)).unwrap();
    let mut paint = StatusPaint::default();
    terminal
        .draw(|frame| {
            let row = frame.area();
            for x in row.left()..row.right() {
                frame
                    .buffer_mut()
                    .set_string(x, 0, CANVAS, theme.style(Role::SurfaceCanvas));
            }
            let area = if tier == ViewportTier::TooSmall {
                row
            } else {
                Rect::new(row.x + 1, row.y, row.width.saturating_sub(2), 1)
            };
            paint = render(frame, area, view, None, locale, facts, tier, theme, None);
        })
        .unwrap();
    (terminal, paint)
}

fn arm_prefix(view: &mut ClientView) {
    let prefix = view.bindings().prefix;
    let transition = view.decide(PrefixInput::Key(KeyEvent::press(prefix.code, prefix.mods)));
    view.commit(transition, ClientActionDisposition::KeepNavigator);
    assert!(view.prefix_active());
}

fn semantic_on_chip(theme: &Theme, role: Role) -> ratatui::style::Style {
    theme.content_on_surface(Role::SurfaceOverlay, None, Some(role), theme.style(role))
}

/// The row's text with canvas cells blanked, so the chip reads on its own.
fn row(terminal: &Terminal<TestBackend>, width: u16) -> String {
    let buffer = terminal.backend().buffer();
    let mut column = 0;
    let mut text = String::new();
    while column < width {
        let symbol = buffer[(column, 0)].symbol();
        text.push_str(if symbol == CANVAS { " " } else { symbol });
        column += u16::try_from(crate::terminal_text::display_width(symbol).max(1)).unwrap();
    }
    text
}

#[test]
fn the_prefix_banner_floats_end_aligned_only_while_the_prefix_is_armed() {
    let theme = Theme::rgb_test_dark();
    let width = 80;
    let ordinary = StatusFacts {
        primary_ordinary: true,
        ..facts("terminal · Ready · controlling")
    };
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for configured in [false, true] {
            let mut view = ClientView::new(width);
            if configured {
                view.configure_keys(&configured_arrow_help());
            }
            let assert_nothing_floats = |view: &ClientView| {
                let (terminal, paint) = paint_at_tier_with_locale(
                    view,
                    width,
                    ordinary,
                    ViewportTier::Full,
                    locale,
                    &theme,
                );
                assert_eq!(paint, StatusPaint::default(), "{locale:?}");
                assert!(row(&terminal, width).trim().is_empty(), "{locale:?}");
            };
            assert_nothing_floats(&view);

            arm_prefix(&mut view);
            let (terminal, paint) = paint_at_tier_with_locale(
                &view,
                width,
                ordinary,
                ViewportTier::Full,
                locale,
                &theme,
            );
            let prefix = crate::client_view::format_binding_with_glyphs(
                &view.bindings().prefix,
                LabelStyle::for_host(),
                KeyLabelGlyphs::Unicode,
            );
            let text = row(&terminal, width);
            let banner = text.trim();
            assert!(
                banner.starts_with(&format!("{} {prefix} · ", locale.status().prefix)),
                "{banner}"
            );
            let banner_width = u16::try_from(crate::terminal_text::display_width(banner)).unwrap();
            // One padding cell at each end, ending one cell in from the right edge.
            let chip = Rect::new(width - 1 - (banner_width + 2), 0, banner_width + 2, 1);
            assert_eq!(paint.surface, Some(chip), "{locale:?}");
            assert_eq!(paint.primary, None, "the ordinary fact stays unsaid");
            let buffer = terminal.backend().buffer();
            for x in [chip.x, chip.right() - 1] {
                assert_eq!(
                    buffer[(x, 0)].bg,
                    theme.style(Role::SurfaceOverlay).bg.unwrap(),
                    "padding cell {x}"
                );
            }
            for x in 0..width {
                let covered = chip.x <= x && x < chip.right();
                assert_eq!(buffer[(x, 0)].symbol() == CANVAS, !covered, "column {x}");
            }

            let transition = view.decide(PrefixInput::Key(KeyEvent::press(
                KeyCode::Esc,
                Modifiers::NONE,
            )));
            view.commit(transition, ClientActionDisposition::KeepNavigator);
            assert!(!view.prefix_active());
            assert_nothing_floats(&view);
        }
    }
}

#[test]
fn prefix_mode_status_hints_drop_only_the_lowest_priority_whole_items_by_width() {
    let theme = Theme::rgb_test_dark();
    let expected = [
        "Esc cancel",
        "? help",
        "Tab sidebar",
        "Space actions",
        "c create Tab",
        r"\ / - split",
        "q detach",
    ];

    let width_cases: &[(u16, usize)] = &[(160, 7), (115, 7), (100, 6), (85, 5), (60, 3)];

    for &(width, visible_count) in width_cases {
        let mut view = ClientView::new(width);
        arm_prefix(&mut view);
        let (terminal, _) = paint(&view, width, facts("Ready"), &theme);
        let prefix = crate::client_view::format_binding_with_glyphs(
            &view.bindings().prefix,
            LabelStyle::for_host(),
            KeyLabelGlyphs::Unicode,
        );
        let marker = format!("PREFIX {prefix} · ");
        let text = row(&terminal, width);
        let hints = text
            .split_once(&marker)
            .map(|(_, hints)| hints.trim_end())
            .unwrap_or_else(|| {
                panic!("prefix indicator and hints missing at width {width}: {text}")
            });
        let expected_hints = expected[..visible_count].join(" · ");
        assert_eq!(hints, expected_hints, "at width {width}");
    }
}

#[test]
fn prefix_mode_hint_labels_are_localized_in_both_supported_locales() {
    let theme = Theme::rgb_test_dark();
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let mut view = ClientView::new(200);
        arm_prefix(&mut view);
        let (terminal, _) = paint_at_tier_with_locale(
            &view,
            200,
            facts("Ready"),
            ViewportTier::Full,
            locale,
            &theme,
        );
        let text = row(&terminal, 200);
        let copy = locale.status();
        for label in [
            copy.cancel,
            copy.help,
            copy.focus_sidebar,
            copy.actions,
            copy.create_tab,
            copy.split,
            copy.detach,
        ] {
            assert!(text.contains(label), "{label:?} missing in {text:?}");
        }
    }
}

#[test]
fn prefix_mode_hint_uses_the_configured_effective_binding_without_repeating_the_prefix() {
    let theme = Theme::rgb_test_dark();
    let mut view = ClientView::new(120);
    view.configure_keys(&configured_arrow_help());
    arm_prefix(&mut view);

    let (terminal, _) = paint(&view, 120, facts("Ready"), &theme);
    let text = row(&terminal, 120);
    assert!(
        text.contains(&crate::terminal_events::keys::tests::host_label(
            "PREFIX Ctrl+a · Esc cancel · ← help"
        )),
        "{text}"
    );
    assert!(
        !text.contains(&crate::terminal_events::keys::tests::host_label("Ctrl+a ←")),
        "{text}"
    );
}

fn configured_arrow_help() -> orchbee_local_runtime::KeySettings {
    use orchbee_local_runtime::{KeyChord, KeyOverride, KeySettings, KeyTrigger};

    let chord = |key: &str, ctrl| KeyChord {
        key: key.into(),
        ctrl,
        alt: false,
        shift: false,
        super_key: false,
    };
    KeySettings {
        commands: Vec::new(),
        prefix: chord("a", true),
        overrides: vec![KeyOverride {
            action: "help",
            triggers: vec![KeyTrigger {
                prefix: true,
                chord: chord("left", false),
            }],
        }],
    }
}

#[test]
fn configured_effective_hint_uses_ascii_glyphs_and_semantic_key_styles() {
    let width = 80;
    let theme = Theme::rgb_test_dark().with_glyphs(GlyphSet::ASCII);
    let mut view = ClientView::new(width);
    view.configure_keys(&configured_arrow_help());
    view.startup_notice(Notice::UnknownPrefix);

    let (terminal, paint) = paint(&view, width, facts("ignored"), &theme);
    let text = row(&terminal, width);
    let key = "Ctrl+a Left";
    let key_start = text.find(key).expect("effective Help chord is visible");
    assert!(text.contains("Unknown prefix key"));
    assert!(text.contains("Ctrl+a Left shortcuts"));
    assert!(!text.contains("Ctrl+b ?"));
    assert!(!text.contains('←'));
    assert_eq!(
        paint.primary, None,
        "a Client notice is not a native draw receipt"
    );

    for x in key_start..key_start + key.len() {
        assert_eq!(
            terminal.backend().buffer()[(u16::try_from(x).unwrap(), 0)].style(),
            semantic_on_chip(&theme, Role::HintKey)
        );
    }
    let label_start = key_start + key.len() + 1;
    for x in label_start..label_start + "shortcuts".len() {
        assert_eq!(
            terminal.backend().buffer()[(u16::try_from(x).unwrap(), 0)].style(),
            semantic_on_chip(&theme, Role::HintLabel)
        );
    }
}

#[test]
fn a_hint_that_does_not_fit_is_dropped_as_a_whole() {
    let width = 24;
    let theme = Theme::rgb_test_dark().with_glyphs(GlyphSet::ASCII);
    let mut view = ClientView::new(width);
    view.configure_keys(&configured_arrow_help());
    view.startup_notice(Notice::UnknownPrefix);

    let (terminal, _) = paint(&view, width, facts("ignored"), &theme);
    let text = row(&terminal, width);
    assert_eq!(text.trim(), "Unknown prefix key");
    assert!(!text.contains("Ctrl+a"));
    assert!(!text.contains("shortcuts"));
}

#[test]
fn too_small_keeps_the_effective_exit_hint_ahead_of_status_actions() {
    let width = 80;
    let theme = Theme::rgb_test_dark();
    let mut view = ClientView::new(width);
    view.startup_notice(Notice::UnknownPrefix);

    let (terminal, _) = paint_at_tier_with_locale(
        &view,
        width,
        facts("ignored"),
        ViewportTier::TooSmall,
        Locale::EnUs,
        &theme,
    );
    let text = row(&terminal, width);
    let key = view
        .bindings()
        .action_labels_with_glyphs(
            PrefixAction::Client(ClientAction::Exit),
            KeyLabelGlyphs::Unicode,
        )
        .into_iter()
        .next()
        .expect("the effective Exit action remains bound");
    let expected = format!("{key} exit");
    assert!(text.contains(&expected), "{text}");
    assert!(!text.contains("shortcuts"), "{text}");
}

#[test]
fn too_small_keeps_the_whole_exit_hint_when_the_primary_fact_cannot_coexist() {
    let width = 19;
    let theme = Theme::rgb_test_dark();
    let view = ClientView::new(width);
    let (terminal, _) = paint_at_tier_with_locale(
        &view,
        width,
        facts("A deliberately long primary status fact"),
        ViewportTier::TooSmall,
        Locale::EnUs,
        &theme,
    );
    let text = row(&terminal, width);
    let key = view
        .bindings()
        .action_labels_with_glyphs(
            PrefixAction::Client(ClientAction::Exit),
            KeyLabelGlyphs::Unicode,
        )
        .into_iter()
        .next()
        .expect("the effective Exit action remains bound");
    let expected = format!("{key} exit");

    assert_eq!(text.trim(), expected);
    assert!(!text.contains("deliberately"), "{text}");
}

#[test]
fn notice_severity_marks_only_the_bullet_and_keeps_reading_text_primary() {
    let width = 60;
    let theme = Theme::rgb_test_dark();
    let view = ClientView::new(width);
    let (terminal, _) = paint(
        &view,
        width,
        StatusFacts {
            secondary_notice: Some("Needs attention"),
            ..facts("Ready")
        },
        &theme,
    );
    let buffer = terminal.backend().buffer();
    let bullet_x = (0..width)
        .find(|&x| buffer[(x, 0)].symbol() == theme.glyphs.status_bullet)
        .expect("notice marker is visible");
    assert_eq!(
        buffer[(bullet_x, 0)].style(),
        semantic_on_chip(&theme, Role::StatusWarning)
    );
    let text_x = bullet_x
        .saturating_add(
            u16::try_from(crate::terminal_text::display_width(
                theme.glyphs.status_bullet,
            ))
            .unwrap(),
        )
        .saturating_add(1);
    for x in text_x..text_x + u16::try_from("Needs attention".len()).unwrap() {
        assert_eq!(
            buffer[(x, 0)].style(),
            semantic_on_chip(&theme, Role::TextPrimary)
        );
    }
}

/// A settling resize holds no input, so the refusal of the person's next resize shows over it
/// rather than behind its pending indicator.
#[test]
fn a_resize_refusal_shows_over_the_pending_resize_indicator() {
    let theme = Theme::rgb_test_dark();
    let width = 80;
    let mut view = ClientView::new(width);
    view.resize_pending = true;
    let pending = Locale::EnUs.status().resize_pending;
    let (terminal, _) = paint(&view, width, facts("Ready"), &theme);
    assert!(row(&terminal, width).contains(pending));
    view.configuration_notice(Notice::ResizeBusy);
    let (terminal, _) = paint(&view, width, facts("Ready"), &theme);
    let painted = row(&terminal, width);
    assert!(
        painted.contains(Locale::EnUs.shell_notice(Some(Notice::ResizeBusy))),
        "{painted}"
    );
    assert!(!painted.contains(pending), "{painted}");
}

/// Lasting states name themselves in a word or two; the long fact they replace stays out.
#[test]
fn persistent_states_show_short_copy_with_their_severity_marker() {
    let theme = Theme::rgb_test_dark();
    let width = 80;
    let view = ClientView::new(width);
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let copy = locale.status();
        for (persistent, text, marker) in [
            (
                PersistentStatus::DirectoryMissing,
                copy.directory_missing,
                Role::StatusWarning,
            ),
            (
                PersistentStatus::Disconnected,
                copy.disconnected,
                Role::StatusError,
            ),
            (
                PersistentStatus::TerminalEnded(TerminalEnd::Exited(
                    orchbee_terminal_protocol::ExitResult::ExitCode(1),
                )),
                copy.terminal_exited,
                Role::StatusWarning,
            ),
            (
                PersistentStatus::TerminalEnded(TerminalEnd::Failed),
                &*locale.terminal_end_compact(TerminalEnd::Failed),
                Role::StatusError,
            ),
        ] {
            let (terminal, paint) = paint_at_tier_with_locale(
                &view,
                width,
                StatusFacts {
                    persistent: Some(persistent),
                    ..facts("terminal · a long lasting fact")
                },
                ViewportTier::Full,
                locale,
                &theme,
            );
            let shown = row(&terminal, width);
            assert_eq!(
                shown.trim(),
                format!("{} {text}", theme.glyphs.status_bullet),
                "{locale:?} {persistent:?}"
            );
            assert_eq!(paint.primary, None);
            let chip = paint.surface.expect("the lasting state floats");
            assert_eq!(
                terminal.backend().buffer()[(chip.x + 1, 0)].style(),
                semantic_on_chip(&theme, marker)
            );
        }
    }
}

/// R-001: a stale rename refusal stays visible in the narrowest content status beside a docked
/// Compact sidebar; retrying is a separate hint on the effective rename binding.
#[test]
fn stale_rename_fact_fits_compact_content_and_retry_is_a_separate_rename_hint() {
    use crate::workbench_state::workspace_setup::{
        LocationLabelStatus as Status, LocationLabelTarget as Target, SetupStatus,
    };
    let theme = Theme::rgb_test_dark();
    // Compact docks the sidebar only while content keeps 60 columns beside it.
    let width = 60;
    let view = ClientView::new(width);
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let (target, action) = (Target::Workspace, ClientAction::RequestWorkspaceRename);
        let fact = locale.setup_status(SetupStatus::LocationLabel {
            target,
            status: Status::Stale,
        });
        let (terminal, paint) = paint_at_tier_with_locale(
            &view,
            width,
            facts(fact),
            ViewportTier::Compact,
            locale,
            &theme,
        );
        assert!(row(&terminal, width).trim().starts_with(fact), "{fact}");
        assert!(paint.primary.is_some(), "{fact}");
        let hint = super::action_hint(
            &view,
            super::StatusAction::RetryRename(target),
            locale,
            KeyLabelGlyphs::Unicode,
        )
        .unwrap();
        assert_eq!(hint.label, locale.status().retry_rename);
        assert_eq!(
            hint.key,
            super::effective_hint(
                &view,
                PrefixAction::Client(action),
                "",
                KeyLabelGlyphs::Unicode
            )
            .unwrap()
            .key
        );
    }
}

mod notice_forms;
