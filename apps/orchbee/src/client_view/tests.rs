use super::{
    BackendAction, ClientAction, ClientTransition, ClientView, KeyCode, KeyEvent, Modifiers,
    Notice, Overlay, PanelDirection, PrefixInput, SessionActionDisposition, ShellCommand,
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
    view.commit(transition, SessionActionDisposition::KeepNavigator);
    transition
}

fn enter_prefix(view: &mut ClientView) {
    let transition = commit(view, ctrl_b());
    assert_eq!(transition.action(), None);
    assert_eq!(transition.shell(), ShellCommand::None);
    assert!(view.prefix_active());
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
        (plain('e'), ClientAction::RequestWorkspaceDirectory, None),
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
        let before = view;
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

    let unknown = commit(&mut view, plain('z'));
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
