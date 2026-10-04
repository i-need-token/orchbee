use super::{
    BackendAction, ClientAction, ClientTransition, ClientView, KeyCode, KeyEvent, Modifiers,
    Notice, Overlay, PREFIX_GROUPS, PanelDirection, PrefixInput, SessionActionDisposition,
    ShellCommand, SidebarSurface, ViewportClass, WIDE_VIEWPORT_MINIMUM_COLUMNS,
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
    view.commit(transition.clone(), SessionActionDisposition::KeepNavigator);
    transition
}

fn enter_prefix(view: &mut ClientView) {
    let transition = commit(view, ctrl_b());
    assert_eq!(transition.action(), None);
    assert_eq!(transition.shell(), ShellCommand::None);
    assert!(view.prefix_active());
}

#[cfg(windows)]
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
    assert_eq!(wide.sidebar_surface(), SidebarSurface::Docked);
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
    assert_eq!(view.sidebar_surface(), SidebarSurface::Overlay);

    view.resize(WIDE_VIEWPORT_MINIMUM_COLUMNS);
    assert_eq!(view.overlay(), None);
    assert_eq!(view.sidebar_surface(), SidebarSurface::Docked);
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
    assert_eq!(view.sidebar_surface(), SidebarSurface::Overlay);

    view.resize(WIDE_VIEWPORT_MINIMUM_COLUMNS);
    assert_eq!(view.sidebar_surface(), SidebarSurface::Hidden);
}

#[test]
fn decides_each_completed_prefix_without_mutating_the_view() {
    let fixed_cases = [
        (plain('r'), ClientAction::RequestWorkspaceRename, None),
        (plain('R'), ClientAction::RequestProjectRename, None),
        (plain('a'), ClientAction::RequestWorkspaceCreate, None),
        #[cfg(windows)]
        (plain('N'), ClientAction::RequestQuickWorkspace, None),
        (plain('D'), ClientAction::RequestWorkspaceClose, None),
        (plain('e'), ClientAction::RequestWorkspaceDirectory, None),
        (plain('c'), ClientAction::CreateTab, None),
        (plain('<'), ClientAction::MovePreviousTab, None),
        (plain('>'), ClientAction::MoveNextTab, None),
        (plain('n'), ClientAction::SelectNextTab, None),
        (plain('p'), ClientAction::SelectPreviousTab, None),
        (plain('['), ClientAction::SelectPreviousContext, None),
        (plain(']'), ClientAction::SelectNextContext, None),
        (plain('('), ClientAction::PreviousPresentation, None),
        (plain(')'), ClientAction::NextPresentation, None),
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
        view.commit(transition, SessionActionDisposition::KeepNavigator);
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
fn question_mark_toggles_the_shortcut_help_without_capturing_input() {
    let mut view = ClientView::new(WIDE_VIEWPORT_MINIMUM_COLUMNS);
    enter_prefix(&mut view);
    let opened = commit(&mut view, plain('?'));
    assert_eq!(opened.action(), Some(ClientAction::OpenShortcutHelp));
    assert_eq!(view.overlay(), Some(Overlay::ShortcutHelp));

    // The help overlay never captures: ordinary keys and Esc keep
    // forwarding so the program's own Esc stays untouched.
    assert_eq!(
        commit(&mut view, plain('x')).shell(),
        ShellCommand::ForwardTerminal
    );
    assert_eq!(
        commit(&mut view, key(KeyCode::Esc)).shell(),
        ShellCommand::ForwardTerminal
    );
    assert_eq!(view.overlay(), Some(Overlay::ShortcutHelp));

    // The same binding closes the overlay it opened.
    enter_prefix(&mut view);
    let closed = commit(&mut view, plain('?'));
    assert_eq!(closed.action(), Some(ClientAction::CloseShortcutHelp));
    assert_eq!(view.overlay(), None);
    assert!(!view.prefix_active());
}

#[test]
fn overlays_never_make_ordinary_or_escape_input_modal() {
    for input in [plain('w'), plain('?')] {
        let mut view = ClientView::new(WIDE_VIEWPORT_MINIMUM_COLUMNS);
        enter_prefix(&mut view);
        commit(&mut view, input);
        let expected = view.overlay();

        assert_eq!(
            commit(&mut view, plain('x')).shell(),
            ShellCommand::ForwardTerminal
        );
        assert_eq!(view.overlay(), expected);
        assert_eq!(
            commit(&mut view, key(KeyCode::Esc)).shell(),
            ShellCommand::ForwardTerminal
        );
        assert_eq!(view.overlay(), expected);
    }
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
    view.commit(selection, SessionActionDisposition::KeepNavigator);
    assert_eq!(view.overlay(), Some(Overlay::WorkbenchNavigator));

    enter_prefix(&mut view);
    let selection = view.decide(plain('n'));
    view.commit(selection, SessionActionDisposition::DismissNavigator);
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
                    |_, _| Ok::<_, std::io::Error>(()),
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
                    |_, _| Ok::<_, std::io::Error>(()),
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
fn page_keys_page_an_open_help_and_reset_on_close_and_reopen() {
    let mut view = ClientView::new(WIDE_VIEWPORT_MINIMUM_COLUMNS);
    assert_eq!(view.help_page(), 0);

    enter_prefix(&mut view);
    commit(&mut view, plain('?'));
    assert_eq!(view.overlay(), Some(Overlay::ShortcutHelp));

    enter_prefix(&mut view);
    let paged = commit(&mut view, key(KeyCode::PageDown));
    assert_eq!(paged.action(), Some(ClientAction::PageShortcutHelp));
    assert_eq!(paged.shell(), ShellCommand::None);
    assert_eq!(view.help_page(), 1);

    enter_prefix(&mut view);
    commit(&mut view, key(KeyCode::PageDown));
    assert_eq!(view.help_page(), 2);
    enter_prefix(&mut view);
    commit(&mut view, key(KeyCode::PageUp));
    assert_eq!(view.help_page(), 1);

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
fn unprefixed_page_keys_still_reach_the_terminal() {
    let mut view = ClientView::new(WIDE_VIEWPORT_MINIMUM_COLUMNS);
    for page_key in [KeyCode::PageUp, KeyCode::PageDown] {
        let transition = commit(&mut view, key(page_key));
        assert_eq!(transition.action(), None);
        assert_eq!(transition.shell(), ShellCommand::ForwardTerminal);
    }
    // And the same holds while the help overlay is open: it never
    // captures ordinary input.
    enter_prefix(&mut view);
    commit(&mut view, plain('?'));
    let transition = commit(&mut view, key(KeyCode::PageDown));
    assert_eq!(transition.shell(), ShellCommand::ForwardTerminal);
    assert_eq!(view.help_page(), 0);
}

#[test]
fn the_help_page_counter_is_bounded_and_settles_the_renderers_effective_page() {
    let mut view = ClientView::new(WIDE_VIEWPORT_MINIMUM_COLUMNS);
    enter_prefix(&mut view);
    commit(&mut view, plain('?'));

    // PageDown can never run away: the counter is capped by the registry
    // row bound, and PageUp saturates at 0.
    for _ in 0..(PREFIX_GROUPS.len() + 8) {
        enter_prefix(&mut view);
        commit(&mut view, key(KeyCode::PageDown));
    }
    assert_eq!(view.help_page(), PREFIX_GROUPS.len());
    enter_prefix(&mut view);
    commit(&mut view, key(KeyCode::PageUp));
    assert_eq!(view.help_page(), PREFIX_GROUPS.len() - 1);

    // The UI loop's write-back of the renderer's effective page is the
    // only way an out-of-range index (e.g. after a resize shrank the page
    // count) reconciles — render never mutates view state itself.
    view.settle_help_page(3);
    assert_eq!(view.help_page(), 3);
    view.settle_help_page(usize::MAX);
    assert_eq!(view.help_page(), PREFIX_GROUPS.len());
    view.settle_help_page(0);
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
