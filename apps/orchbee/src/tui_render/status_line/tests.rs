use ratatui::{Terminal, backend::TestBackend, layout::Rect};

use crate::{
    client_view::{
        ClientAction, ClientView, KeyEvent, KeyLabelGlyphs, LabelStyle, Notice, PrefixAction,
        PrefixInput, SessionActionDisposition,
    },
    localization::Locale,
    tui_layout::ViewportTier,
    tui_theme::{GlyphSet, Role, Theme},
};

use super::render;

struct StatusContent<'a> {
    primary: &'a str,
    secondary_notice: Option<&'a str>,
    recovery: bool,
}

#[cfg(windows)]
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
        assert_eq!(workspace.key, "Ctrl+a c");
        assert_eq!(workspace.label, locale.status().create_workspace);
        assert_eq!(tab.label, locale.status().create_tab);
        assert_ne!(workspace.label, tab.label);
    }
}

fn paint(
    view: &ClientView,
    width: u16,
    status: &str,
    secondary_notice: Option<&str>,
    recovery_status: bool,
    theme: &Theme,
) -> (Terminal<TestBackend>, Option<Rect>) {
    paint_at_tier(
        view,
        width,
        status,
        secondary_notice,
        recovery_status,
        ViewportTier::Full,
        theme,
    )
}

fn paint_at_tier(
    view: &ClientView,
    width: u16,
    status: &str,
    secondary_notice: Option<&str>,
    recovery_status: bool,
    tier: ViewportTier,
    theme: &Theme,
) -> (Terminal<TestBackend>, Option<Rect>) {
    paint_at_tier_with_locale(
        view,
        width,
        StatusContent {
            primary: status,
            secondary_notice,
            recovery: recovery_status,
        },
        tier,
        Locale::EnUs,
        theme,
    )
}

fn paint_at_tier_with_locale(
    view: &ClientView,
    width: u16,
    content: StatusContent<'_>,
    tier: ViewportTier,
    locale: Locale,
    theme: &Theme,
) -> (Terminal<TestBackend>, Option<Rect>) {
    let mut terminal = Terminal::new(TestBackend::new(width, 1)).unwrap();
    let mut receipt = None;
    terminal
        .draw(|frame| {
            let area = frame.area();
            frame
                .buffer_mut()
                .set_style(area, theme.style(Role::SurfaceCanvas));
            receipt = render(
                frame,
                frame.area(),
                view,
                None,
                locale,
                content.primary,
                content.secondary_notice,
                false,
                false,
                content.recovery,
                tier,
                theme,
                None,
            );
        })
        .unwrap();
    (terminal, receipt)
}

fn arm_prefix(view: &mut ClientView) {
    let prefix = view.bindings().prefix;
    let transition = view.decide(PrefixInput::Key(KeyEvent::press(prefix.code, prefix.mods)));
    view.commit(transition, SessionActionDisposition::KeepNavigator);
    assert!(view.prefix_active());
}

fn semantic_on_canvas(theme: &Theme, role: Role) -> ratatui::style::Style {
    theme.content_on_surface(Role::SurfaceCanvas, None, Some(role), theme.style(role))
}

fn row(terminal: &Terminal<TestBackend>, width: u16) -> String {
    let buffer = terminal.backend().buffer();
    let mut column = 0;
    let mut text = String::new();
    while column < width {
        let symbol = buffer[(column, 0)].symbol();
        text.push_str(symbol);
        column += u16::try_from(crate::terminal_text::display_width(symbol).max(1)).unwrap();
    }
    text
}

#[test]
fn prefix_mode_status_hints_drop_only_the_lowest_priority_whole_items_by_width() {
    let theme = Theme::rgb_test_dark();
    let mut expected = vec!["Esc cancel", "? help"];
    #[cfg(windows)]
    expected.push("F2 actions");
    expected.extend(["c create Tab", r"\ / - split", "g Agent board", "q detach"]);

    let width_cases: &[(u16, usize)] = if cfg!(windows) {
        &[(160, 7), (100, 6), (80, 5), (60, 3)]
    } else {
        &[(160, 6), (100, 6), (80, 5), (60, 3)]
    };

    for &(width, visible_count) in width_cases {
        let mut view = ClientView::new(width);
        arm_prefix(&mut view);
        let (terminal, _) = paint(&view, width, "Ready", None, false, &theme);
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
        let mut view = ClientView::new(160);
        arm_prefix(&mut view);
        let (terminal, _) = paint_at_tier_with_locale(
            &view,
            160,
            StatusContent {
                primary: "Ready",
                secondary_notice: None,
                recovery: false,
            },
            ViewportTier::Full,
            locale,
            &theme,
        );
        let text = row(&terminal, 160);
        let copy = locale.status();
        for label in [
            copy.cancel,
            copy.help,
            copy.create_tab,
            copy.split,
            copy.agent_board,
            copy.detach,
        ] {
            assert!(text.contains(label), "{label:?} missing in {text:?}");
        }
        #[cfg(windows)]
        assert!(
            text.contains(copy.actions),
            "{} missing in {text:?}",
            copy.actions
        );
    }
}

#[cfg(windows)]
#[test]
fn prefix_mode_hint_uses_the_configured_effective_binding_without_repeating_the_prefix() {
    let theme = Theme::rgb_test_dark();
    let mut view = ClientView::new(120);
    view.configure_keys(&configured_arrow_help());
    arm_prefix(&mut view);

    let (terminal, _) = paint(&view, 120, "Ready", None, false, &theme);
    let text = row(&terminal, 120);
    assert!(
        text.contains("PREFIX Ctrl+a · Esc cancel · ← help"),
        "{text}"
    );
    assert!(!text.contains("Ctrl+a ←"), "{text}");
}

#[cfg(windows)]
fn configured_arrow_help() -> orchbee_local_site_runtime::WindowsKeySettings {
    use orchbee_local_site_runtime::{
        WindowsKeyChord, WindowsKeyOverride, WindowsKeySettings, WindowsKeyTrigger,
    };

    let chord = |key: &str, ctrl| WindowsKeyChord {
        key: key.into(),
        ctrl,
        alt: false,
        shift: false,
        super_key: false,
    };
    WindowsKeySettings {
        commands: Vec::new(),
        prefix: chord("a", true),
        overrides: vec![WindowsKeyOverride {
            action: "help",
            triggers: vec![WindowsKeyTrigger {
                prefix: true,
                chord: chord("left", false),
            }],
        }],
    }
}

#[cfg(windows)]
#[test]
fn configured_effective_hint_uses_ascii_glyphs_and_semantic_key_styles() {
    let width = 80;
    let theme = Theme::rgb_test_dark().with_glyphs(GlyphSet::ASCII);
    let mut view = ClientView::new(width);
    view.configure_keys(&configured_arrow_help());
    view.startup_notice(Notice::UnknownPrefix);

    let (terminal, receipt) = paint(&view, width, "ignored", None, false, &theme);
    let text = row(&terminal, width);
    let key = "Ctrl+a Left";
    let key_start = text.find(key).expect("effective Help chord is visible");
    assert!(text.contains("Unknown prefix key"));
    assert!(text.contains("Ctrl+a Left shortcuts"));
    assert!(!text.contains("Ctrl+b ?"));
    assert!(!text.contains('←'));
    assert_eq!(
        receipt, None,
        "a Client notice is not a native draw receipt"
    );

    for x in key_start..key_start + key.len() {
        assert_eq!(
            terminal.backend().buffer()[(u16::try_from(x).unwrap(), 0)].style(),
            semantic_on_canvas(&theme, Role::HintKey)
        );
    }
    let label_start = key_start + key.len() + 1;
    for x in label_start..label_start + "shortcuts".len() {
        assert_eq!(
            terminal.backend().buffer()[(u16::try_from(x).unwrap(), 0)].style(),
            semantic_on_canvas(&theme, Role::HintLabel)
        );
    }
}

#[cfg(windows)]
#[test]
fn a_hint_that_does_not_fit_is_dropped_as_a_whole() {
    let width = 24;
    let theme = Theme::rgb_test_dark().with_glyphs(GlyphSet::ASCII);
    let mut view = ClientView::new(width);
    view.configure_keys(&configured_arrow_help());
    view.startup_notice(Notice::UnknownPrefix);

    let (terminal, _) = paint(&view, width, "ignored", None, false, &theme);
    let text = row(&terminal, width);
    assert_eq!(text.trim_end(), "Unknown prefix key");
    assert!(!text.contains("Ctrl+a"));
    assert!(!text.contains("shortcuts"));
}

#[test]
fn too_small_keeps_the_effective_exit_hint_ahead_of_status_actions() {
    let width = 80;
    let theme = Theme::rgb_test_dark();
    let mut view = ClientView::new(width);
    view.startup_notice(Notice::UnknownPrefix);

    let (terminal, _) = paint_at_tier(
        &view,
        width,
        "ignored",
        None,
        false,
        ViewportTier::TooSmall,
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
    let (terminal, _) = paint_at_tier(
        &view,
        width,
        "A deliberately long primary status fact",
        None,
        false,
        ViewportTier::TooSmall,
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
        "Ready",
        Some("Needs attention"),
        false,
        &theme,
    );
    let buffer = terminal.backend().buffer();
    let bullet_x = (0..width)
        .find(|&x| buffer[(x, 0)].symbol() == theme.glyphs.status_bullet)
        .expect("notice marker is visible");
    assert_eq!(
        buffer[(bullet_x, 0)].style(),
        semantic_on_canvas(&theme, Role::StatusWarning)
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
            semantic_on_canvas(&theme, Role::TextPrimary)
        );
    }
}

#[test]
fn recoverable_status_receipt_requires_the_complete_raw_fact() {
    let theme = Theme::rgb_test_dark();
    let status = "Close committed; cleanup pending";
    let width = u16::try_from(crate::terminal_text::display_width(status)).unwrap();
    let view = ClientView::new(width);

    let (terminal, receipt) = paint(&view, width, status, None, true, &theme);
    assert_eq!(receipt, Some(Rect::new(0, 0, width, 1)));
    assert_eq!(row(&terminal, width), status);

    let narrow = width - 1;
    let view = ClientView::new(narrow);
    let (terminal, receipt) = paint(&view, narrow, status, None, true, &theme);
    assert_eq!(receipt, None);
    assert!(row(&terminal, narrow).trim().is_empty());
}
