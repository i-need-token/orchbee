use super::{
    BackendAction, ClientAction, ClientKey, ClientTransition, ClientView, Notice, Overlay,
    PanelDirection, SessionActionDisposition, ShellCommand, SidebarSurface, ViewportClass,
    WIDE_VIEWPORT_MINIMUM_COLUMNS,
};

fn commit(view: &mut ClientView, key: ClientKey) -> ClientTransition {
    let transition = view.decide(key);
    view.commit(transition, SessionActionDisposition::KeepNavigator);
    transition
}

fn enter_prefix(view: &mut ClientView) {
    let transition = commit(view, ClientKey::ControlB);
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
    let transition = commit(&mut view, ClientKey::Plain('b'));
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
    let transition = commit(&mut view, ClientKey::Plain('b'));
    assert!(transition.layout_changed());
    assert_eq!(view.sidebar_surface(), SidebarSurface::Hidden);

    view.resize(WIDE_VIEWPORT_MINIMUM_COLUMNS - 1);
    enter_prefix(&mut view);
    let transition = commit(&mut view, ClientKey::Plain('b'));
    assert!(!transition.layout_changed());
    assert_eq!(view.sidebar_surface(), SidebarSurface::Overlay);

    view.resize(WIDE_VIEWPORT_MINIMUM_COLUMNS);
    assert_eq!(view.sidebar_surface(), SidebarSurface::Hidden);
}

#[test]
fn decides_each_completed_prefix_without_mutating_the_view() {
    let fixed_cases = [
        (
            ClientKey::Plain('r'),
            ClientAction::RequestWorkspaceRename,
            None,
        ),
        (
            ClientKey::Plain('R'),
            ClientAction::RequestProjectRename,
            None,
        ),
        (
            ClientKey::Plain('a'),
            ClientAction::RequestWorkspaceCreate,
            None,
        ),
        (
            ClientKey::Plain('e'),
            ClientAction::RequestWorkspaceDirectory,
            None,
        ),
        (ClientKey::Plain('c'), ClientAction::CreateTab, None),
        (ClientKey::Plain('<'), ClientAction::MovePreviousTab, None),
        (ClientKey::Plain('>'), ClientAction::MoveNextTab, None),
        (ClientKey::Plain('n'), ClientAction::SelectNextTab, None),
        (ClientKey::Plain('p'), ClientAction::SelectPreviousTab, None),
        (
            ClientKey::Plain('['),
            ClientAction::SelectPreviousContext,
            None,
        ),
        (ClientKey::Plain(']'), ClientAction::SelectNextContext, None),
        (
            ClientKey::Plain('h'),
            ClientAction::MovePanelFocus(PanelDirection::Left),
            None,
        ),
        (
            ClientKey::Plain('j'),
            ClientAction::MovePanelFocus(PanelDirection::Down),
            None,
        ),
        (
            ClientKey::Plain('k'),
            ClientAction::MovePanelFocus(PanelDirection::Up),
            None,
        ),
        (
            ClientKey::Plain('l'),
            ClientAction::MovePanelFocus(PanelDirection::Right),
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
        let transition = commit(&mut view, ClientKey::Plain(key));
        assert_eq!(transition.action(), Some(ClientAction::SelectTab(position)));
        assert_eq!(transition.shell(), ShellCommand::None);
        assert_eq!(view.notice(), None);
    }
}

#[test]
fn toggles_the_navigator_and_replaces_other_overlays() {
    let mut view = ClientView::new(WIDE_VIEWPORT_MINIMUM_COLUMNS);
    enter_prefix(&mut view);
    let opened = commit(&mut view, ClientKey::Plain('w'));
    assert_eq!(opened.action(), Some(ClientAction::OpenWorkbenchNavigator));
    assert_eq!(view.overlay(), Some(Overlay::WorkbenchNavigator));

    enter_prefix(&mut view);
    let closed = commit(&mut view, ClientKey::Plain('w'));
    assert_eq!(closed.action(), Some(ClientAction::CloseWorkbenchNavigator));
    assert_eq!(view.overlay(), None);

    enter_prefix(&mut view);
    commit(&mut view, ClientKey::Plain('?'));
    assert_eq!(view.overlay(), Some(Overlay::ShortcutHelp));
    enter_prefix(&mut view);
    let replaced = commit(&mut view, ClientKey::Plain('w'));
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
    commit(&mut view, ClientKey::Plain('w'));
    assert_eq!(view.overlay(), Some(Overlay::WorkbenchNavigator));

    enter_prefix(&mut view);
    let toggle = commit(&mut view, ClientKey::Plain('b'));
    assert_eq!(toggle.action(), Some(ClientAction::ToggleBusinessSidebar));
    assert!(toggle.layout_changed());
    assert_eq!(view.overlay(), None);
    assert_eq!(view.sidebar_surface(), SidebarSurface::Hidden);
}

#[test]
fn overlays_never_make_ordinary_or_escape_input_modal() {
    for key in [ClientKey::Plain('w'), ClientKey::Plain('?')] {
        let mut view = ClientView::new(WIDE_VIEWPORT_MINIMUM_COLUMNS);
        enter_prefix(&mut view);
        commit(&mut view, key);
        let expected = view.overlay();

        assert_eq!(
            commit(&mut view, ClientKey::Plain('x')).shell(),
            ShellCommand::ForwardTerminal
        );
        assert_eq!(view.overlay(), expected);
        assert_eq!(
            commit(&mut view, ClientKey::Escape).shell(),
            ShellCommand::ForwardTerminal
        );
        assert_eq!(view.overlay(), expected);
    }
}

#[test]
fn forwards_only_ordinary_input_and_the_literal_prefix_binding() {
    let mut view = ClientView::new(WIDE_VIEWPORT_MINIMUM_COLUMNS);
    assert_eq!(
        commit(&mut view, ClientKey::Plain('x')).shell(),
        ShellCommand::ForwardTerminal
    );
    assert_eq!(
        commit(&mut view, ClientKey::Other).shell(),
        ShellCommand::ForwardTerminal
    );

    enter_prefix(&mut view);
    let literal = commit(&mut view, ClientKey::ControlB);
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

    let unknown = commit(&mut view, ClientKey::Plain('z'));
    assert_eq!(unknown.action(), Some(ClientAction::UnknownPrefix));
    assert_eq!(view.notice(), Some(Notice::UnknownPrefix));

    enter_prefix(&mut view);
    commit(&mut view, ClientKey::Escape);
    assert_eq!(view.notice(), None);
}

#[test]
fn only_a_success_disposition_dismisses_an_open_navigator() {
    let mut view = ClientView::new(WIDE_VIEWPORT_MINIMUM_COLUMNS);
    enter_prefix(&mut view);
    commit(&mut view, ClientKey::Plain('w'));

    enter_prefix(&mut view);
    let selection = view.decide(ClientKey::Plain('n'));
    view.commit(selection, SessionActionDisposition::KeepNavigator);
    assert_eq!(view.overlay(), Some(Overlay::WorkbenchNavigator));

    enter_prefix(&mut view);
    let selection = view.decide(ClientKey::Plain('n'));
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
fn exits_only_for_the_two_accepted_prefixed_keys() {
    for key in ['q', 'd'] {
        let mut view = ClientView::new(WIDE_VIEWPORT_MINIMUM_COLUMNS);
        enter_prefix(&mut view);
        let transition = commit(&mut view, ClientKey::Plain(key));
        assert_eq!(transition.action(), Some(ClientAction::Exit));
        assert_eq!(transition.shell(), ShellCommand::Exit);
    }

    let mut view = ClientView::new(WIDE_VIEWPORT_MINIMUM_COLUMNS);
    assert_eq!(
        commit(&mut view, ClientKey::Plain('q')).shell(),
        ShellCommand::ForwardTerminal
    );
}
