use super::{
    BackendAction, ClientAction, ClientActionDisposition, ClientTransition, ClientView, KeyCode,
    KeyEvent, Modifiers, Notice, Overlay, PanelDirection, PrefixInput, ShellCommand,
    SidebarSurface, ViewportClass, WIDE_VIEWPORT_MINIMUM_COLUMNS,
};

fn key(code: KeyCode) -> PrefixInput {
    PrefixInput::Key(KeyEvent::press(code, Modifiers::NONE))
}

fn plain(c: char) -> PrefixInput {
    key(KeyCode::Char(c))
}

fn ctrl_b() -> PrefixInput {
    PrefixInput::Key(KeyEvent::press(KeyCode::Char('b'), Modifiers::CTRL))
}

fn commit(view: &mut ClientView, input: PrefixInput) -> ClientTransition {
    let transition = view.decide(input);
    view.commit(transition.clone(), ClientActionDisposition::KeepNavigator);
    transition
}

fn enter_prefix(view: &mut ClientView) {
    let transition = commit(view, ctrl_b());
    assert_eq!(transition.action(), None);
    assert_eq!(transition.shell(), ShellCommand::None);
    assert!(view.prefix_active());
}

#[test]
fn checkpoint_recovery_precedes_the_persistent_configuration_warning() {
    let mut view = ClientView::new(120);
    view.startup_notice(Notice::CheckpointEmpty);
    view.configuration_warning(true);
    assert_eq!(view.notice(), Some(Notice::CheckpointEmpty));

    enter_prefix(&mut view);
    let transition = commit(&mut view, plain('n'));
    assert_eq!(transition.action(), Some(ClientAction::SelectNextTab));
    assert_eq!(view.notice(), Some(Notice::ConfigurationWarning));

    view.configuration_warning(false);
    assert_eq!(view.notice(), None);
}

#[test]
fn agent_focus_dismissal_clears_shell_capture_without_changing_pointer_or_sidebar() {
    let mut view = ClientView::new(120);
    view.overlay = Some(Overlay::WorkbenchNavigator);
    view.prefix_active = true;
    view.settle_pointer_released(true);
    let sidebar = view.sidebar_surface();
    view.dismiss_focus_modes();
    assert!(view.overlay().is_none());
    assert!(!view.prefix_active());
    assert!(view.pointer_released());
    assert_eq!(view.sidebar_surface(), sidebar);
}

#[test]
fn classifies_the_wide_boundary_and_projects_the_sidebar_per_client() {
    let narrow = ClientView::new(WIDE_VIEWPORT_MINIMUM_COLUMNS - 1);
    assert_eq!(
        ViewportClass::from_width(WIDE_VIEWPORT_MINIMUM_COLUMNS - 1),
        ViewportClass::Narrow
    );
    assert_eq!(narrow.sidebar_surface(), SidebarSurface::Hidden);

    let wide = ClientView::new(WIDE_VIEWPORT_MINIMUM_COLUMNS);
    assert_eq!(
        ViewportClass::from_width(WIDE_VIEWPORT_MINIMUM_COLUMNS),
        ViewportClass::Wide
    );
    assert_eq!(
        wide.sidebar_surface(),
        SidebarSurface::Docked(crate::tui_layout::SidebarWidth::DEFAULT)
    );
}

/// A controlling Client must report the colors it paints even when the outer terminal did not
/// answer. Otherwise a dark named theme leaves OSC 11 describing the Worker's light OS fallback.
#[test]
fn controlling_client_reports_its_named_theme_before_any_outer_terminal_reply() {
    let mut view = ClientView::new(120);
    view.configure_color_level(crate::tui_theme::ColorLevel::Truecolor);
    view.apply_user_preferences(orchbee_workbench_protocol::UserSettingsValues {
        application_theme: orchbee_workbench_protocol::ApplicationAppearance::Dark,
        dark_theme: "gruvbox-dark".into(),
        ..Default::default()
    });
    assert_eq!(
        view.host_default_colors(),
        view.configured_theme().unwrap().terminal_rgb_defaults(),
        "OSC queries must describe the controlling Client's painted theme"
    );
    assert!(view.host_default_colors().is_some());
}

#[test]
fn detected_host_polarity_updates_only_system_chrome_and_retains_terminal_selection() {
    use crate::host_appearance::{HostAppearance, HostColors};
    use orchbee_workbench_protocol::{
        ApplicationAppearance as Application, TerminalDisplayAppearance as Terminal,
    };

    let values = orchbee_workbench_protocol::UserSettingsValues {
        application_theme: Application::System,
        terminal_display_theme: Terminal::Custom,
        custom_foreground: "#123456".into(),
        custom_background: "#654321".into(),
        ..Default::default()
    };
    let mut system = ClientView::new(120);
    system.configure_color_level(crate::tui_theme::ColorLevel::Truecolor);
    system.apply_user_preferences(values);
    assert!(system.configure_host_terminal(
        HostAppearance::Dark,
        HostColors {
            foreground: Some(0xcccccc),
            background: Some(0x0c0c0c),
            ..Default::default()
        }
    ));
    let dark = system.configured_theme().unwrap();
    assert_eq!(dark.name, "orchbee-dark");
    assert_eq!(
        dark.terminal_defaults(),
        (
            ratatui::style::Color::Rgb(0x12, 0x34, 0x56),
            ratatui::style::Color::Rgb(0x65, 0x43, 0x21)
        )
    );
    assert!(system.configure_host_terminal(
        HostAppearance::Light,
        HostColors {
            foreground: Some(0x141414),
            background: Some(0xfcfcfc),
            ..Default::default()
        }
    ));
    assert_eq!(system.configured_theme().unwrap().name, "orchbee-light");

    let host_values = orchbee_workbench_protocol::UserSettingsValues {
        application_theme: Application::System,
        terminal_display_theme: Terminal::Host,
        ..Default::default()
    };
    let mut host = ClientView::new(120);
    host.configure_color_level(crate::tui_theme::ColorLevel::Truecolor);
    host.apply_user_preferences(host_values);
    assert!(host.configure_host_appearance(HostAppearance::Dark));
    assert_eq!(
        host.configured_theme()
            .unwrap()
            .with_host_terminal_colors(0x141414, 0xfcfcfc)
            .terminal_defaults(),
        (ratatui::style::Color::Reset, ratatui::style::Color::Reset)
    );
    assert!(host.configure_host_terminal(
        HostAppearance::Dark,
        crate::host_appearance::HostColors {
            foreground: Some(0x123456),
            background: Some(0x203040),
            ..Default::default()
        },
    ));
    let host_theme = host
        .configured_theme()
        .unwrap()
        .with_host_terminal_colors(0x141414, 0xfcfcfc);
    assert_eq!(host_theme.name, "orchbee-dark");
    assert_eq!(
        host_theme.terminal_defaults(),
        (ratatui::style::Color::Reset, ratatui::style::Color::Reset)
    );
    assert_eq!(
        host_theme.terminal_rgb_defaults(),
        Some((0x123456, 0x203040))
    );

    for (application, expected) in [
        (Application::Light, "orchbee-light"),
        (Application::Dark, "orchbee-dark"),
    ] {
        let values = orchbee_workbench_protocol::UserSettingsValues {
            application_theme: application,
            ..Default::default()
        };
        let mut explicit = ClientView::new(120);
        explicit.configure_color_level(crate::tui_theme::ColorLevel::Truecolor);
        explicit.apply_user_preferences(values);
        assert!(!explicit.configure_host_appearance(HostAppearance::Dark));
        let theme = explicit
            .configured_theme()
            .unwrap()
            .with_host_terminal_colors(0x123456, 0x203040);
        assert_eq!(theme.name, expected);
        assert_eq!(
            theme.terminal_defaults(),
            if application == Application::Light {
                (
                    ratatui::style::Color::Rgb(0x14, 0x14, 0x14),
                    ratatui::style::Color::Rgb(0xfc, 0xfc, 0xfc),
                )
            } else {
                (
                    ratatui::style::Color::Rgb(0xd4, 0xd4, 0xd4),
                    ratatui::style::Color::Rgb(0x1e, 0x1e, 0x1e),
                )
            }
        );
    }
}

/// System paints the slot of the settled host appearance: the outer terminal's reported
/// background, or the operating system's appearance once the probe expires unanswered. It paints
/// only at truecolor; otherwise, and before the appearance settles, the TUI keeps Terminal colors.
#[test]
fn system_paints_the_slot_of_the_settled_host_appearance_only_on_a_truecolor_host() {
    use crate::host_appearance::{HostAppearance, HostColors};
    use crate::tui_theme::ColorLevel;
    let values = orchbee_workbench_protocol::UserSettingsValues {
        dark_theme: "gruvbox-dark".into(),
        ..Default::default()
    };
    let mut view = ClientView::new(120);
    view.configure_color_level(ColorLevel::Truecolor);
    view.apply_user_preferences(values.clone());
    assert!(view.named_themes_available());
    assert_eq!(view.configured_theme().unwrap().name, "terminal-native");
    view.configure_host_appearance(HostAppearance::Dark);
    assert_eq!(view.configured_theme().unwrap().name, "gruvbox-dark");
    view.configure_host_terminal(
        HostAppearance::Light,
        HostColors {
            foreground: Some(0x141414),
            background: Some(0xfcfcfc),
            ..Default::default()
        },
    );
    assert_eq!(view.configured_theme().unwrap().name, "orchbee-light");
    let mut preview_values = values.clone();
    preview_values.light_theme = "dayfox".into();
    preview_values.application_theme = orchbee_workbench_protocol::ApplicationAppearance::Light;
    assert_eq!(
        view.preview_theme(crate::native_settings::AppearancePreview::from_values(
            &preview_values
        ))
        .name,
        "dayfox"
    );
    view.configure_color_level(ColorLevel::Ansi256);
    assert!(!view.named_themes_available());
    assert_eq!(view.configured_theme().unwrap().name, "terminal-native");
}

#[test]
fn resizes_without_turning_a_docked_sidebar_into_an_automatic_overlay() {
    let mut view = ClientView::new(WIDE_VIEWPORT_MINIMUM_COLUMNS);
    view.resize(WIDE_VIEWPORT_MINIMUM_COLUMNS - 1);
    assert_eq!(view.sidebar_surface(), SidebarSurface::Hidden);

    enter_prefix(&mut view);
    let transition = commit(&mut view, plain('b'));
    assert_eq!(transition.shell(), ShellCommand::None);
    assert!(!transition.layout_changed());
    assert_eq!(
        view.sidebar_surface(),
        SidebarSurface::Overlay(crate::tui_layout::SidebarWidth::DEFAULT)
    );

    view.resize(WIDE_VIEWPORT_MINIMUM_COLUMNS);
    assert_eq!(view.overlay(), None);
    assert_eq!(
        view.sidebar_surface(),
        SidebarSurface::Docked(crate::tui_layout::SidebarWidth::DEFAULT)
    );
}

/// The mobile list is on demand like the narrow overlay: it opens with the keyboard, crossing
/// 60 columns keeps it open as the overlay, and crossing 90 columns closes it.
#[test]
fn mobile_list_takes_the_keyboard_and_only_the_wide_threshold_closes_it() {
    let mobile = crate::sidebar_width::COMPACT_MINIMUM_COLUMNS - 1;
    let mut view = ClientView::new(mobile);
    view.machines.available = true;
    assert_eq!(ViewportClass::from_width(mobile), ViewportClass::Mobile);
    assert_eq!(view.sidebar_surface(), SidebarSurface::Hidden);
    assert!(!view.sidebar_screen());

    enter_prefix(&mut view);
    commit(&mut view, plain('b'));
    assert!(view.sidebar_screen());
    assert_eq!(view.overlay(), Some(Overlay::BusinessSidebar));
    assert!(view.machines.sidebar_navigation_active());

    view.resize(mobile + 1);
    assert!(matches!(view.sidebar_surface(), SidebarSurface::Overlay(_)));
    assert!(!view.sidebar_screen());
    view.resize(mobile);
    assert!(view.sidebar_screen());

    view.resize(WIDE_VIEWPORT_MINIMUM_COLUMNS);
    assert_eq!(view.overlay(), None);
    assert!(matches!(view.sidebar_surface(), SidebarSurface::Docked(_)));
    view.resize(mobile);
    assert_eq!(view.sidebar_surface(), SidebarSurface::Hidden);
    assert!(!view.machines.sidebar_navigation_active());
}

#[test]
fn marks_only_wide_sidebar_reflow_as_a_layout_change() {
    let mut view = ClientView::new(WIDE_VIEWPORT_MINIMUM_COLUMNS);
    enter_prefix(&mut view);
    let transition = commit(&mut view, plain('b'));
    assert!(transition.layout_changed());
    assert_eq!(view.sidebar_surface(), SidebarSurface::Hidden);

    view.resize(WIDE_VIEWPORT_MINIMUM_COLUMNS - 1);
    enter_prefix(&mut view);
    let transition = commit(&mut view, plain('b'));
    assert!(!transition.layout_changed());
    assert_eq!(
        view.sidebar_surface(),
        SidebarSurface::Overlay(crate::tui_layout::SidebarWidth::DEFAULT)
    );

    view.resize(WIDE_VIEWPORT_MINIMUM_COLUMNS);
    assert_eq!(view.sidebar_surface(), SidebarSurface::Hidden);
}

#[test]
fn decides_each_completed_prefix_without_mutating_the_view() {
    let fixed_cases = [
        (plain('r'), ClientAction::RequestWorkspaceRename, None),
        (plain('a'), ClientAction::RequestWorkspaceCreate, None),
        (plain('N'), ClientAction::RequestQuickWorkspace, None),
        (plain('D'), ClientAction::RequestWorkspaceClose, None),
        (plain('c'), ClientAction::CreateTab, None),
        (plain('<'), ClientAction::MovePreviousTab, None),
        (plain('>'), ClientAction::MoveNextTab, None),
        (plain('n'), ClientAction::SelectNextTab, None),
        (plain('p'), ClientAction::SelectPreviousTab, None),
        (plain('['), ClientAction::SelectPreviousContext, None),
        (plain(']'), ClientAction::SelectNextContext, None),
        (
            plain('h'),
            ClientAction::MovePanelFocus(PanelDirection::Left),
            None,
        ),
        (
            plain('j'),
            ClientAction::MovePanelFocus(PanelDirection::Down),
            None,
        ),
        (
            plain('k'),
            ClientAction::MovePanelFocus(PanelDirection::Up),
            None,
        ),
        (
            plain('l'),
            ClientAction::MovePanelFocus(PanelDirection::Right),
            None,
        ),
        (
            plain('-'),
            ClientAction::SplitPanel(PanelDirection::Down),
            None,
        ),
        (
            plain('_'),
            ClientAction::SplitPanel(PanelDirection::Up),
            None,
        ),
        (
            plain('\\'),
            ClientAction::SplitPanel(PanelDirection::Left),
            None,
        ),
        (
            plain('|'),
            ClientAction::SplitPanel(PanelDirection::Right),
            None,
        ),
        (plain('X'), ClientAction::ClosePanel, None),
        (
            plain('H'),
            ClientAction::MovePanel(PanelDirection::Left),
            None,
        ),
        (
            plain('J'),
            ClientAction::MovePanel(PanelDirection::Down),
            None,
        ),
        (
            plain('K'),
            ClientAction::MovePanel(PanelDirection::Up),
            None,
        ),
        (
            plain('L'),
            ClientAction::MovePanel(PanelDirection::Right),
            None,
        ),
    ];

    for (key, expected_action, expected_notice) in fixed_cases {
        let mut view = ClientView::new(WIDE_VIEWPORT_MINIMUM_COLUMNS);
        enter_prefix(&mut view);
        let before = view.clone();
        let transition = view.decide(key);
        assert_eq!(view, before);
        assert_eq!(transition.action(), Some(expected_action));
        assert_eq!(transition.shell(), ShellCommand::None);
        view.commit(transition, ClientActionDisposition::KeepNavigator);
        assert!(!view.prefix_active());
        assert_eq!(view.notice(), expected_notice);
    }

    for position in 1..=9 {
        let mut view = ClientView::new(WIDE_VIEWPORT_MINIMUM_COLUMNS);
        enter_prefix(&mut view);
        let key = char::from(b'0' + position);
        let transition = commit(&mut view, plain(key));
        assert_eq!(transition.action(), Some(ClientAction::SelectTab(position)));
        assert_eq!(transition.shell(), ShellCommand::None);
        assert_eq!(view.notice(), None);
    }
}

#[test]
fn toggles_the_navigator_and_replaces_other_overlays() {
    let mut view = ClientView::new(WIDE_VIEWPORT_MINIMUM_COLUMNS);
    enter_prefix(&mut view);
    let opened = commit(&mut view, plain('w'));
    assert_eq!(opened.action(), Some(ClientAction::OpenWorkbenchNavigator));
    assert_eq!(view.overlay(), Some(Overlay::WorkbenchNavigator));

    enter_prefix(&mut view);
    let closed = commit(&mut view, plain('w'));
    assert_eq!(closed.action(), Some(ClientAction::CloseWorkbenchNavigator));
    assert_eq!(view.overlay(), None);

    enter_prefix(&mut view);
    commit(&mut view, plain('?'));
    assert_eq!(view.overlay(), Some(Overlay::ShortcutHelp));
    enter_prefix(&mut view);
    let replaced = commit(&mut view, plain('w'));
    assert_eq!(
        replaced.action(),
        Some(ClientAction::OpenWorkbenchNavigator)
    );
    assert_eq!(view.overlay(), Some(Overlay::WorkbenchNavigator));
}

#[test]
fn wide_sidebar_toggle_replaces_an_open_overlay() {
    let mut view = ClientView::new(WIDE_VIEWPORT_MINIMUM_COLUMNS);
    enter_prefix(&mut view);
    commit(&mut view, plain('w'));
    assert_eq!(view.overlay(), Some(Overlay::WorkbenchNavigator));

    enter_prefix(&mut view);
    let toggle = commit(&mut view, plain('b'));
    assert_eq!(toggle.action(), Some(ClientAction::ToggleBusinessSidebar));
    assert!(toggle.layout_changed());
    assert_eq!(view.overlay(), None);
    assert_eq!(view.sidebar_surface(), SidebarSurface::Hidden);
}

#[test]
fn shortcut_help_captures_input_and_escape_closes_it() {
    let mut view = ClientView::new(WIDE_VIEWPORT_MINIMUM_COLUMNS);
    enter_prefix(&mut view);
    let opened = commit(&mut view, plain('?'));
    assert_eq!(opened.action(), Some(ClientAction::OpenShortcutHelp));
    assert_eq!(view.overlay(), Some(Overlay::ShortcutHelp));

    // Help filters ordinary input while visible, but the global prefix can
    // still close it or replace it with another Client overlay.
    assert_eq!(commit(&mut view, plain('x')).shell(), ShellCommand::None);
    assert_eq!(view.help_filter(), "x");
    assert_eq!(view.overlay(), Some(Overlay::ShortcutHelp));
    let escaped = commit(&mut view, key(KeyCode::Esc));
    assert_eq!(escaped.action(), Some(ClientAction::CloseShortcutHelp));
    assert_eq!(escaped.shell(), ShellCommand::None);
    assert_eq!(view.overlay(), None);

    // The same binding closes the overlay it opened.
    enter_prefix(&mut view);
    assert_eq!(
        commit(&mut view, plain('?')).action(),
        Some(ClientAction::OpenShortcutHelp)
    );
    enter_prefix(&mut view);
    let closed = commit(&mut view, plain('?'));
    assert_eq!(closed.action(), Some(ClientAction::CloseShortcutHelp));
    assert_eq!(view.overlay(), None);
    assert!(!view.prefix_active());
}

#[test]
fn help_escape_closes_even_after_prefix_and_other_prefix_actions_switch_surfaces() {
    let mut view = ClientView::new(WIDE_VIEWPORT_MINIMUM_COLUMNS);
    enter_prefix(&mut view);
    commit(&mut view, plain('?'));

    enter_prefix(&mut view);
    let escaped = commit(&mut view, key(KeyCode::Esc));
    assert_eq!(escaped.action(), Some(ClientAction::CloseShortcutHelp));
    assert_eq!(escaped.shell(), ShellCommand::None);
    assert_eq!(view.overlay(), None);
    assert!(!view.prefix_active());

    enter_prefix(&mut view);
    commit(&mut view, plain('?'));
    assert_eq!(view.overlay(), Some(Overlay::ShortcutHelp));

    enter_prefix(&mut view);
    let switched = commit(&mut view, plain('w'));
    assert_eq!(
        switched.action(),
        Some(ClientAction::OpenWorkbenchNavigator)
    );
    assert_eq!(view.overlay(), Some(Overlay::WorkbenchNavigator));
}

#[test]
fn forwards_only_ordinary_input_and_the_literal_prefix_binding() {
    let mut view = ClientView::new(WIDE_VIEWPORT_MINIMUM_COLUMNS);
    assert_eq!(
        commit(&mut view, plain('x')).shell(),
        ShellCommand::ForwardTerminal
    );
    assert_eq!(
        commit(&mut view, PrefixInput::NonKey).shell(),
        ShellCommand::ForwardTerminal
    );

    enter_prefix(&mut view);
    let literal = commit(&mut view, ctrl_b());
    assert_eq!(literal.action(), Some(ClientAction::ForwardLiteralPrefix));
    assert_eq!(literal.shell(), ShellCommand::ForwardTerminal);
    assert!(!view.prefix_active());
}

#[test]
fn completed_prefix_replaces_parser_notice_but_first_prefix_does_not() {
    let mut view = ClientView::new(WIDE_VIEWPORT_MINIMUM_COLUMNS);
    view.report_terminal_input_unavailable();
    enter_prefix(&mut view);
    assert_eq!(view.notice(), Some(Notice::TerminalInputUnavailable));

    let unknown = commit(&mut view, plain('y'));
    assert_eq!(unknown.action(), Some(ClientAction::UnknownPrefix));
    assert_eq!(view.notice(), Some(Notice::UnknownPrefix));

    enter_prefix(&mut view);
    commit(&mut view, key(KeyCode::Esc));
    assert_eq!(view.notice(), None);
}

#[test]
fn only_a_success_disposition_dismisses_an_open_navigator() {
    let mut view = ClientView::new(WIDE_VIEWPORT_MINIMUM_COLUMNS);
    enter_prefix(&mut view);
    commit(&mut view, plain('w'));

    enter_prefix(&mut view);
    let selection = view.decide(plain('n'));
    view.commit(selection, ClientActionDisposition::KeepNavigator);
    assert_eq!(view.overlay(), Some(Overlay::WorkbenchNavigator));

    enter_prefix(&mut view);
    let selection = view.decide(plain('n'));
    view.commit(selection, ClientActionDisposition::DismissNavigator);
    assert_eq!(view.overlay(), None);
}

#[test]
fn reports_unavailable_adapter_results_as_local_notices() {
    let mut view = ClientView::new(WIDE_VIEWPORT_MINIMUM_COLUMNS);
    view.report_backend_unavailable(BackendAction::CreateTab);
    assert_eq!(
        view.notice(),
        Some(Notice::BackendUnavailable(BackendAction::CreateTab))
    );

    view.report_terminal_input_unavailable();
    assert_eq!(view.notice(), Some(Notice::TerminalInputUnavailable));
}

#[test]
fn unconnected_fallback_reports_the_new_panel_actions_and_keeps_focus_keys_silent() {
    use crate::terminal_input::{DispatchOutcome, InputDispatcher, UnavailableTerminalInputPort};
    use std::time::Instant;

    for (key, expected) in [
        (b'-', BackendAction::SplitPanel),
        (b'_', BackendAction::SplitPanel),
        (b'\\', BackendAction::SplitPanel),
        (b'|', BackendAction::SplitPanel),
        (b'X', BackendAction::ClosePanel),
        (b'H', BackendAction::MovePanel),
        (b'J', BackendAction::MovePanel),
        (b'K', BackendAction::MovePanel),
        (b'L', BackendAction::MovePanel),
    ] {
        let mut view = ClientView::new(WIDE_VIEWPORT_MINIMUM_COLUMNS);
        let mut dispatcher = InputDispatcher::default();
        let mut port = UnavailableTerminalInputPort;
        assert_eq!(
            dispatcher
                .dispatch(
                    &mut view,
                    &[0x02, key],
                    &mut port,
                    Instant::now(),
                    |_, _| Ok::<bool, std::io::Error>(false),
                    &mut |_| Ok(()),
                )
                .unwrap(),
            DispatchOutcome::Continue
        );
        assert_eq!(
            view.notice(),
            Some(Notice::BackendUnavailable(expected)),
            "Ctrl+B {}",
            key as char
        );
        assert!(!view.prefix_active());
    }

    // Baseline asymmetry (B1): h/j/k/l carry no BackendAction, so the same
    // unconnected fallback drops them without any notice.
    for key in *b"hjkl" {
        let mut view = ClientView::new(WIDE_VIEWPORT_MINIMUM_COLUMNS);
        let mut dispatcher = InputDispatcher::default();
        let mut port = UnavailableTerminalInputPort;
        assert_eq!(
            dispatcher
                .dispatch(
                    &mut view,
                    &[0x02, key],
                    &mut port,
                    Instant::now(),
                    |_, _| Ok::<bool, std::io::Error>(false),
                    &mut |_| Ok(()),
                )
                .unwrap(),
            DispatchOutcome::Continue
        );
        assert_eq!(view.notice(), None, "Ctrl+B {}", key as char);
        assert!(!view.prefix_active());
    }
}

#[test]
fn prefix_page_keys_scroll_open_help_and_reset_on_close_and_reopen() {
    let mut view = ClientView::new(WIDE_VIEWPORT_MINIMUM_COLUMNS);
    assert_eq!(view.help_page(), 0);

    enter_prefix(&mut view);
    commit(&mut view, plain('?'));
    assert_eq!(view.overlay(), Some(Overlay::ShortcutHelp));

    enter_prefix(&mut view);
    let paged = commit(&mut view, key(KeyCode::PageDown));
    assert_eq!(paged.action(), Some(ClientAction::PageShortcutHelp));
    assert_eq!(paged.shell(), ShellCommand::None);
    assert_eq!(view.help_page(), 8);

    enter_prefix(&mut view);
    commit(&mut view, key(KeyCode::PageDown));
    assert_eq!(view.help_page(), 16);
    enter_prefix(&mut view);
    commit(&mut view, key(KeyCode::PageUp));
    assert_eq!(view.help_page(), 8);

    // Closing and reopening starts again at page 0.
    enter_prefix(&mut view);
    commit(&mut view, plain('?'));
    assert_eq!(view.overlay(), None);
    assert_eq!(view.help_page(), 0);
    enter_prefix(&mut view);
    commit(&mut view, plain('?'));
    assert_eq!(view.help_page(), 0);
}

#[test]
fn page_keys_without_help_follow_the_unknown_prefix_rules() {
    // `Ctrl+B PageUp` with help closed is the retained-history entry
    // gesture; the session owns admission from there.
    let mut view = ClientView::new(WIDE_VIEWPORT_MINIMUM_COLUMNS);
    enter_prefix(&mut view);
    let transition = commit(&mut view, key(KeyCode::PageUp));
    assert_eq!(transition.action(), Some(ClientAction::EnterHistory));
    assert_eq!(transition.shell(), ShellCommand::None);
    assert_eq!(view.help_page(), 0);

    let mut view = ClientView::new(WIDE_VIEWPORT_MINIMUM_COLUMNS);
    enter_prefix(&mut view);
    let transition = commit(&mut view, key(KeyCode::PageDown));
    assert_eq!(transition.action(), Some(ClientAction::UnknownPrefix));
    assert_eq!(transition.shell(), ShellCommand::None);
    assert_eq!(view.notice(), Some(Notice::UnknownPrefix));
    assert_eq!(view.help_page(), 0);

    // An overlay other than help does not claim the page keys either.
    let mut view = ClientView::new(WIDE_VIEWPORT_MINIMUM_COLUMNS);
    enter_prefix(&mut view);
    commit(&mut view, plain('w'));
    enter_prefix(&mut view);
    let transition = commit(&mut view, key(KeyCode::PageDown));
    assert_eq!(transition.action(), Some(ClientAction::UnknownPrefix));
    assert_eq!(view.help_page(), 0);
}

#[test]
fn unprefixed_page_keys_scroll_help_and_reach_the_terminal_without_it() {
    let mut view = ClientView::new(WIDE_VIEWPORT_MINIMUM_COLUMNS);
    for page_key in [KeyCode::PageUp, KeyCode::PageDown] {
        let transition = commit(&mut view, key(page_key));
        assert_eq!(transition.action(), None);
        assert_eq!(transition.shell(), ShellCommand::ForwardTerminal);
    }
    enter_prefix(&mut view);
    commit(&mut view, plain('?'));
    let transition = commit(&mut view, key(KeyCode::PageDown));
    assert_eq!(transition.action(), Some(ClientAction::PageShortcutHelp));
    assert_eq!(transition.shell(), ShellCommand::None);
    assert_eq!(view.help_page(), 8);
}

#[test]
fn help_scroll_requests_clamp_only_after_an_exact_draw_settles() {
    let mut view = ClientView::new(WIDE_VIEWPORT_MINIMUM_COLUMNS);
    enter_prefix(&mut view);
    commit(&mut view, plain('?'));

    let bound = crate::tui_shortcut_help::entries(
        crate::localization::Locale::EnUs,
        &view,
        true,
        crate::client_view::KeyLabelGlyphs::Unicode,
    )
    .len();
    // Input may request beyond the current viewport; the exact presented
    // source settles it after the renderer knows the scroll boundary.
    for _ in 0..(bound + 8) {
        enter_prefix(&mut view);
        commit(&mut view, key(KeyCode::PageDown));
    }
    assert_eq!(view.help_page(), (bound + 8) * 8);
    enter_prefix(&mut view);
    commit(&mut view, key(KeyCode::PageUp));
    assert_eq!(view.help_page(), (bound + 8) * 8 - 8);

    // Only a successfully published exact source settles the effective page
    // after geometry shrinks the real page count.
    let candidate = view.help_draw_candidate().unwrap();
    view.settle_help_draw(candidate.presented(3, 3));
    assert_eq!(view.help_page(), 3);
    // A scroll request revokes that source and remains pending until the next draw.
    view.request_help_page(usize::MAX);
    assert_eq!(view.help_page(), usize::MAX);
    view.request_help_page(0);
    assert_eq!(view.help_page(), 0);
}

#[test]
fn exits_only_for_the_two_accepted_prefixed_keys() {
    for key in ['q', 'd'] {
        let mut view = ClientView::new(WIDE_VIEWPORT_MINIMUM_COLUMNS);
        enter_prefix(&mut view);
        let transition = commit(&mut view, plain(key));
        assert_eq!(transition.action(), Some(ClientAction::Exit));
        assert_eq!(transition.shell(), ShellCommand::Exit);
    }

    let mut view = ClientView::new(WIDE_VIEWPORT_MINIMUM_COLUMNS);
    assert_eq!(
        commit(&mut view, plain('q')).shell(),
        ShellCommand::ForwardTerminal
    );
}

#[test]
fn prefix_tab_toggles_sidebar_focus_without_activating_a_workspace() {
    let mut view = ClientView::new(120);
    view.machines.available = true;
    for focused in [true, false, true] {
        enter_prefix(&mut view);
        let transition = commit(&mut view, key(KeyCode::Tab));
        assert_eq!(
            transition.action(),
            Some(ClientAction::FocusBusinessSidebar)
        );
        assert_eq!(view.machines.sidebar_navigation_active(), focused);
        assert!(!view.prefix_active());
    }
}

#[test]
fn dropped_input_notice_ends_the_navigation_hold() {
    let mut view = ClientView::new(120);
    assert!(view.permits_navigation_hold());
    view.configuration_warning(true);
    assert!(
        view.permits_navigation_hold(),
        "a persistent warning was already visible before navigation"
    );
    // Typing while the new selection has no live frame is refused; the refusal must paint now.
    view.report_terminal_input_unavailable();
    assert_eq!(view.notice(), Some(Notice::TerminalInputUnavailable));
    assert!(!view.permits_navigation_hold());
}
