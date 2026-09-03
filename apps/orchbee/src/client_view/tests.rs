use super::{
    BackendAction, ClientKey, ClientView, Notice, Overlay, PanelDirection, ShellCommand,
    SidebarSurface, ViewportClass, WIDE_VIEWPORT_MINIMUM_COLUMNS,
};

fn enter_prefix(view: &mut ClientView) {
    assert_eq!(view.transition(ClientKey::ControlB), ShellCommand::None);
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
    assert_eq!(view.transition(ClientKey::Plain('b')), ShellCommand::None);
    assert_eq!(view.sidebar_surface(), SidebarSurface::Overlay);

    view.resize(WIDE_VIEWPORT_MINIMUM_COLUMNS);
    assert_eq!(view.overlay(), None);
    assert_eq!(view.sidebar_surface(), SidebarSurface::Docked);
}

#[test]
fn toggles_the_wide_sidebar_without_changing_the_narrow_overlay_preference() {
    let mut view = ClientView::new(WIDE_VIEWPORT_MINIMUM_COLUMNS);
    enter_prefix(&mut view);
    view.transition(ClientKey::Plain('b'));
    assert_eq!(view.sidebar_surface(), SidebarSurface::Hidden);

    view.resize(WIDE_VIEWPORT_MINIMUM_COLUMNS - 1);
    enter_prefix(&mut view);
    view.transition(ClientKey::Plain('b'));
    assert_eq!(view.sidebar_surface(), SidebarSurface::Overlay);

    view.resize(WIDE_VIEWPORT_MINIMUM_COLUMNS);
    assert_eq!(view.sidebar_surface(), SidebarSurface::Hidden);
}

#[test]
fn maps_every_backend_dependent_prefix_binding_without_mutating_shared_state() {
    let fixed_cases = [
        (ClientKey::Plain('c'), BackendAction::CreateTab),
        (ClientKey::Plain('n'), BackendAction::SelectNextTab),
        (ClientKey::Plain('p'), BackendAction::SelectPreviousTab),
        (
            ClientKey::Plain('h'),
            BackendAction::MovePanelFocus(PanelDirection::Left),
        ),
        (
            ClientKey::Plain('j'),
            BackendAction::MovePanelFocus(PanelDirection::Down),
        ),
        (
            ClientKey::Plain('k'),
            BackendAction::MovePanelFocus(PanelDirection::Up),
        ),
        (
            ClientKey::Plain('l'),
            BackendAction::MovePanelFocus(PanelDirection::Right),
        ),
    ];

    for (key, expected) in fixed_cases {
        let mut view = ClientView::new(WIDE_VIEWPORT_MINIMUM_COLUMNS);
        enter_prefix(&mut view);
        assert_eq!(view.transition(key), ShellCommand::Backend(expected));
        assert!(!view.prefix_active());
        assert_eq!(view.notice(), None);
        assert_eq!(view.overlay(), None);
    }

    for tab in 1..=9 {
        let mut view = ClientView::new(WIDE_VIEWPORT_MINIMUM_COLUMNS);
        enter_prefix(&mut view);
        let key = char::from(b'0' + tab);
        assert_eq!(
            view.transition(ClientKey::Plain(key)),
            ShellCommand::Backend(BackendAction::SelectTab(tab))
        );
        assert!(!view.prefix_active());
        assert_eq!(view.notice(), None);
        assert_eq!(view.overlay(), None);
    }
}

#[test]
fn opens_client_local_overlays_without_making_them_input_modal() {
    for (key, expected) in [
        (ClientKey::Plain('w'), Overlay::WorkbenchNavigator),
        (ClientKey::Plain('?'), Overlay::ShortcutHelp),
    ] {
        let mut view = ClientView::new(WIDE_VIEWPORT_MINIMUM_COLUMNS);
        enter_prefix(&mut view);
        assert_eq!(view.transition(key), ShellCommand::None);
        assert_eq!(view.overlay(), Some(expected));
        assert_eq!(
            view.transition(ClientKey::Plain('x')),
            ShellCommand::ForwardTerminal
        );
        assert_eq!(view.overlay(), Some(expected));
        assert_eq!(
            view.transition(ClientKey::Escape),
            ShellCommand::ForwardTerminal
        );
        assert_eq!(view.overlay(), Some(expected));
    }
}

#[test]
fn forwards_terminal_input_only_outside_prefix_or_for_literal_control_b() {
    let mut view = ClientView::new(WIDE_VIEWPORT_MINIMUM_COLUMNS);
    assert_eq!(
        view.transition(ClientKey::Plain('x')),
        ShellCommand::ForwardTerminal
    );
    assert_eq!(
        view.transition(ClientKey::Other),
        ShellCommand::ForwardTerminal
    );

    enter_prefix(&mut view);
    assert_eq!(
        view.transition(ClientKey::ControlB),
        ShellCommand::ForwardTerminal
    );
    assert!(!view.prefix_active());
}

#[test]
fn cancels_prefix_with_escape_without_changing_an_overlay() {
    let mut view = ClientView::new(WIDE_VIEWPORT_MINIMUM_COLUMNS);
    enter_prefix(&mut view);
    view.transition(ClientKey::Plain('?'));
    enter_prefix(&mut view);
    assert_eq!(view.transition(ClientKey::Escape), ShellCommand::None);
    assert!(!view.prefix_active());
    assert_eq!(view.overlay(), Some(Overlay::ShortcutHelp));
}

#[test]
fn reports_unknown_prefix_and_unavailable_adapter_results_as_local_notices() {
    let mut view = ClientView::new(WIDE_VIEWPORT_MINIMUM_COLUMNS);
    enter_prefix(&mut view);
    assert_eq!(view.transition(ClientKey::Plain('x')), ShellCommand::None);
    assert_eq!(view.notice(), Some(Notice::UnknownPrefix));

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
        assert_eq!(view.transition(ClientKey::Plain(key)), ShellCommand::Exit);
    }

    let mut view = ClientView::new(WIDE_VIEWPORT_MINIMUM_COLUMNS);
    assert_eq!(
        view.transition(ClientKey::Plain('q')),
        ShellCommand::ForwardTerminal
    );
}
