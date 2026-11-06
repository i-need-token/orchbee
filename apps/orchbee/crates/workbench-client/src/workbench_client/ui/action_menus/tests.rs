use super::super::{tests::context_selection, tests::move_membership, tests::publication_hint};
use super::*;
use crate::client_view::ClientActionDisposition;
use crate::localization::DisplayPhase;
use crate::workbench_client::DrawReceipt;
use crate::workbench_client::ui::{
    CarrierFailure, QueuedCommand, RefreshDecision, UiReply, WorkerEvent,
};
use crate::{
    client_view::ClientView, localization::Locale, tui_hit_map::HitMapBuilder, tui_theme::Theme,
};
use ratatui::{Terminal, backend::TestBackend};

mod organization;
mod project;
mod quick_create;
mod tab_scope;

#[test]
fn new_tab_button_menu_lists_saved_profiles_default_first_and_cancel_has_no_effect() {
    for cancel in [false, true] {
        let (mut ui, mailbox, _, _) = tests::move_membership::ready_ui();
        let create = ui.view().tab_create_target().unwrap();
        // With only the default profile, the button's own click is the whole choice.
        assert!(!ui.open_action_menu(MenuTarget::TabProfiles(create.clone()), None, false));
        let profile = orchbee_workbench_protocol::UserLaunchProfile {
            id: "test-shell".into(),
            name: Some("测试 PowerShell".into()),
            command: orchbee_workbench_protocol::UserLaunchCommand::CustomExecutable {
                executable: r"C:\test\pwsh.exe".into(),
                arguments: vec!["-NoProfile".into(), "a b".into()],
            },
        };
        ui.preferences
            .launch_profiles
            .profiles
            .insert(0, profile.clone());
        let saved = ui.preferences.clone();
        assert!(ui.open_action_menu(MenuTarget::TabProfiles(create), None, false));
        let view = ui.view();
        let menu = view.action_menu().unwrap();
        assert!(
            menu.actions
                .iter()
                .all(|action| matches!(action, MenuAction::NewTabWithProfile(_)))
        );
        assert!(
            matches!(&menu.actions[0], MenuAction::NewTabWithProfile(first) if first.id == saved.launch_profiles.default_profile_id)
        );
        assert_eq!(
            menu.notes[0],
            Some(crate::action_menu::MenuRowNote::DefaultProfile)
        );
        assert!(menu.notes[1..].iter().all(Option::is_none));
        let row = action_row(
            &ui,
            MenuAction::NewTabWithProfile(Box::new(profile.clone())),
        );
        let token = arm(&mut ui);
        if cancel {
            ui.cancel_action_menu();
            assert!(ui.take_action_menu_action(false).is_none());
        } else {
            activate_row(&mut ui, token, row);
            let action = ui.take_action_menu_action(false).unwrap();
            assert_eq!(
                action.intent(),
                ClientAction::TabMenu {
                    action: MenuAction::NewTabWithProfile(Box::new(profile)),
                    tab_id: None
                }
            );
            assert!(
                mailbox.try_recv().unwrap().is_none(),
                "selection alone submits nothing"
            );
            ui.apply_client_action(action);
            assert!(mailbox.try_recv().unwrap().is_some());
        }
        assert!(mailbox.try_recv().unwrap().is_none());
        assert_eq!(ui.preferences, saved);
    }
}

#[test]
fn global_search_routes_nested_profile_hits_through_the_existing_tab_creation_menu() {
    use orchbee_workbench_protocol::{UserLaunchCommand, UserLaunchProfile};

    let (mut ui, _, _, _) = tests::move_membership::ready_ui();
    let profile = UserLaunchProfile {
        id: "search-profile".into(),
        name: Some("PowerShell preview".into()),
        command: UserLaunchCommand::CustomExecutable {
            executable: r"C:\test\pwsh.exe".into(),
            arguments: Vec::new(),
        },
    };
    ui.preferences
        .launch_profiles
        .profiles
        .push(profile.clone());
    assert!(ui.open_action_menu(MenuTarget::Global, None, false));
    let view = ui.view();
    let menu = view.action_menu().unwrap();
    assert!(menu.search_children.iter().any(|child| {
        child.parent == MenuAction::TabCreation
            && child.actions.iter().any(|action| {
                matches!(action, MenuAction::NewTabWithProfile(candidate) if candidate.id == profile.id)
            })
    }));

    arm(&mut ui);
    for character in "power".chars() {
        ui.capture_action_menu_key(KeyEvent::press(
            crate::client_view::KeyCode::Char(character),
            crate::client_view::Modifiers::NONE,
        ));
    }
    let token = {
        let view = ui.view();
        let menu = view.action_menu().unwrap();
        assert_eq!(menu.query, Some("power"));
        menu.token
    };
    ui.after_action_menu_draw(Some(ModalPresentation::Presented(MenuDraw {
        selected: 0,
        candidate_visible: true,
        token,
        first: 0,
        visible_rows: 1,
        result_actions: vec![0],
    })));
    ui.capture_action_menu_key(KeyEvent::press(
        crate::client_view::KeyCode::Enter,
        crate::client_view::Modifiers::NONE,
    ));
    assert!(ui.take_action_menu_action(false).is_none());

    let view = ui.view();
    let child = view.action_menu().unwrap();
    assert!(matches!(child.source, MenuTarget::TabCreate(_)));
    assert_eq!(child.query, Some("power"));
    assert!(child.actions.iter().any(|action| {
        matches!(action, MenuAction::NewTabWithProfile(candidate) if candidate.id == profile.id)
    }));

    ui.capture_action_menu_key(KeyEvent::press(
        crate::client_view::KeyCode::Esc,
        crate::client_view::Modifiers::NONE,
    ));
    let view = ui.view();
    let parent = view.action_menu().unwrap();
    assert_eq!(parent.query, Some("power"));
    assert_eq!(parent.actions[parent.selected], MenuAction::TabCreation);
}

fn arm(ui: &mut WorkbenchClientUi) -> MenuToken {
    let (token, selected, visible_rows, result_actions, hidden) = {
        let view = ui.view();
        let menu = view.action_menu().unwrap();
        (
            menu.token,
            menu.selected,
            menu.actions.len(),
            menu.result_actions.to_vec(),
            menu.candidate_hidden,
        )
    };
    ui.after_action_menu_draw(Some(ModalPresentation::Presented(MenuDraw {
        selected,
        candidate_visible: !hidden,
        token,
        first: 0,
        visible_rows,
        result_actions,
    })));
    token
}

fn action_row(ui: &WorkbenchClientUi, action: MenuAction) -> usize {
    ui.view()
        .action_menu()
        .unwrap()
        .actions
        .iter()
        .position(|candidate| *candidate == action)
        .unwrap()
}

fn empty_ui_without_focused_panel() -> WorkbenchClientUi {
    let elapsed = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let clock = crate::local_client::test_support::server::adjustable_operation_clock(elapsed);
    let mailbox = std::sync::Arc::new(crate::workbench_client::mailbox::CommandMailbox::new(
        10,
        Geometry::new(120, 40, 8, 16).unwrap(),
    ));
    let mut ui = WorkbenchClientUi::new(std::sync::Arc::clone(&mailbox), clock.clone());
    let cache = crate::workbench_projection::project_preferred_workspace(
        &super::super::tests::membership::snapshot(0, 1),
    )
    .unwrap();
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::Bootstrap {
                selection: None,
                cache,
            },
            crate::workbench_client::mailbox::EventDeadline::Startup(
                clock.now() + crate::workbench_state::OPERATION_TOTAL,
            ),
            &clock,
        )
        .unwrap();
    ui.tick();
    assert!(mailbox.take_deposited_reply(&wait).unwrap().is_some());
    assert!(
        ui.state
            .as_ref()
            .is_some_and(|state| state.focus_pane_ref().is_none()),
        "the empty initial_workspace context keeps its live carrier without a focused panel"
    );
    ui
}

fn activate_row(ui: &mut WorkbenchClientUi, token: MenuToken, row: usize) {
    ui.drive_action_menu();
    let Some(source) = ui.view().action_menu().map(|menu| menu.source.clone()) else {
        return;
    };
    ui.activate_action_menu_row(&source, token, row);
}

#[test]
fn global_menu_groups_existing_tab_panel_workspace_and_client_actions_and_dispatches_them() {
    use crate::client_view::PanelDirection as Direction;

    let expected = [
        (MenuAction::RenameTab, ClientAction::RequestRenameTab),
        (MenuAction::RenamePanel, ClientAction::RequestRenamePanel),
        (MenuAction::ClearTerminal, ClientAction::ClearTerminal),
        // The Tab section's other rows act on the current Tab, so they name no Tab.
        (
            MenuAction::NewTabToRight,
            ClientAction::TabMenu {
                action: MenuAction::NewTabToRight,
                tab_id: None,
            },
        ),
        (
            MenuAction::DuplicateTab,
            ClientAction::TabMenu {
                action: MenuAction::DuplicateTab,
                tab_id: None,
            },
        ),
        (
            MenuAction::SplitPanel(Direction::Up),
            ClientAction::SplitPanel(Direction::Up),
        ),
        (
            MenuAction::SplitPanel(Direction::Down),
            ClientAction::SplitPanel(Direction::Down),
        ),
        (
            MenuAction::SplitPanel(Direction::Left),
            ClientAction::SplitPanel(Direction::Left),
        ),
        (
            MenuAction::SplitPanel(Direction::Right),
            ClientAction::SplitPanel(Direction::Right),
        ),
        (MenuAction::ClosePane, ClientAction::ClosePanel),
        (MenuAction::Zoom, ClientAction::ToggleZoom),
        (
            MenuAction::MovePanel(Direction::Up),
            ClientAction::MovePanel(Direction::Up),
        ),
        (
            MenuAction::MovePanel(Direction::Down),
            ClientAction::MovePanel(Direction::Down),
        ),
        (
            MenuAction::MovePanel(Direction::Left),
            ClientAction::MovePanel(Direction::Left),
        ),
        (
            MenuAction::MovePanel(Direction::Right),
            ClientAction::MovePanel(Direction::Right),
        ),
        (MenuAction::Copy, ClientAction::EnterCopyMode),
        (MenuAction::Resize, ClientAction::EnterResizeMode),
    ];
    let (mut ui, _, _, _) = tests::move_membership::ready_ui();
    assert!(ui.open_action_menu(MenuTarget::Global, None, false));
    {
        let view = ui.view();
        let menu = view.action_menu().unwrap();
        assert_eq!(menu.actions, crate::action_menu::GLOBAL_ACTIONS);
    }
    ui.cancel_action_menu();

    for (action, expected_client_action) in expected {
        assert!(ui.open_action_menu(MenuTarget::Global, None, false));
        let row = action_row(&ui, action.clone());
        assert_eq!(ui.view().action_menu().unwrap().unavailable[row], None);
        let token = arm(&mut ui);
        activate_row(&mut ui, token, row);
        assert_eq!(
            ui.take_action_menu_action(false)
                .map(|command| command.intent()),
            Some(expected_client_action),
            "{action:?} must use the same ClientAction as its keyboard binding"
        );
    }
}

#[test]
fn global_tab_and_panel_actions_report_when_no_panel_is_focused() {
    use crate::client_view::PanelDirection as Direction;

    let mut ui = empty_ui_without_focused_panel();
    assert!(ui.open_action_menu(MenuTarget::Global, None, false));
    for action in [
        MenuAction::RenameTab,
        MenuAction::SplitPanel(Direction::Up),
        MenuAction::SplitPanel(Direction::Down),
        MenuAction::SplitPanel(Direction::Left),
        MenuAction::SplitPanel(Direction::Right),
        MenuAction::ClosePane,
        MenuAction::Zoom,
        MenuAction::MovePanel(Direction::Up),
        MenuAction::MovePanel(Direction::Down),
        MenuAction::MovePanel(Direction::Left),
        MenuAction::MovePanel(Direction::Right),
        MenuAction::Copy,
        MenuAction::Resize,
    ] {
        let row = action_row(&ui, action);
        assert_eq!(
            ui.view().action_menu().unwrap().unavailable[row],
            Some(MenuUnavailable::FocusedPanelRequired)
        );
    }
    assert_eq!(
        Locale::EnUs.menu_unavailable(MenuUnavailable::FocusedPanelRequired, false),
        "A focused panel is required for this action."
    );
    assert_eq!(
        Locale::ZhCn.menu_unavailable(MenuUnavailable::FocusedPanelRequired, false),
        "此操作需要焦点面板。"
    );
    let row = action_row(&ui, MenuAction::SplitPanel(Direction::Up));
    let token = arm(&mut ui);
    activate_row(&mut ui, token, row);
    assert_eq!(ui.take_action_menu_action(false), None);
    assert!(ui.action_menu_active());
}

#[test]
fn guarded_menu_keys_consume_owned_phases_and_reject_stale_snapshots() {
    use crate::{
        action_menu::MenuInputReadiness,
        client_view::{KeyCode, KeyEvent, Modifiers},
        tui_presentation::PresentationPhase,
    };

    let (mut ui, _, _, _) = tests::move_membership::ready_ui();
    assert!(ui.open_action_menu(MenuTarget::Global, None, false));
    let pending = ui.action_menu_input_context().unwrap();
    assert_eq!(pending.phase(), PresentationPhase::Pending);
    assert_eq!(pending.readiness(), MenuInputReadiness::Recovery);
    assert!(
        ui.capture_action_menu_key_for(pending, KeyEvent::press(KeyCode::Down, Modifiers::NONE))
    );
    assert_eq!(ui.view().action_menu().unwrap().selected, 0);
    assert!(
        ui.capture_action_menu_key_for(pending, KeyEvent::press(KeyCode::Esc, Modifiers::NONE))
    );
    assert!(!ui.action_menu_capture_claim());

    assert!(ui.open_action_menu(MenuTarget::Global, None, false));
    arm(&mut ui);
    ui.after_action_menu_draw(None);
    let covered = ui.action_menu_input_context().unwrap();
    assert_eq!(covered.phase(), PresentationPhase::Admitted);
    assert_eq!(covered.readiness(), MenuInputReadiness::Recovery);
    assert!(
        ui.capture_action_menu_key_for(covered, KeyEvent::press(KeyCode::Down, Modifiers::NONE))
    );
    assert_eq!(ui.view().action_menu().unwrap().selected, 0);
    assert!(
        ui.capture_action_menu_key_for(covered, KeyEvent::press(KeyCode::Esc, Modifiers::NONE))
    );
    assert!(!ui.action_menu_capture_claim());

    assert!(ui.open_action_menu(MenuTarget::Global, None, false));
    arm(&mut ui);
    let first = ui.action_menu_input_context().unwrap();
    assert_eq!(first.readiness(), MenuInputReadiness::ActivationReady);
    assert!(ui.capture_action_menu_key_for(first, KeyEvent::press(KeyCode::Down, Modifiers::NONE)));
    let continuation = ui.action_menu_input_context().unwrap();
    assert_ne!(continuation.token(), first.token());
    assert_eq!(continuation.readiness(), MenuInputReadiness::Continuation);
    assert!(
        !ui.capture_action_menu_key_for(first, KeyEvent::press(KeyCode::Enter, Modifiers::NONE)),
        "a stale central snapshot is fail-closed"
    );
    assert_eq!(ui.menus.menu.confirmed(), None);
    assert!(ui.capture_action_menu_key_for(
        continuation,
        KeyEvent::press(KeyCode::Down, Modifiers::NONE)
    ));
    assert_eq!(
        ui.view().action_menu().unwrap().selected,
        1,
        "a second move waits for the exact selected row to be drawn"
    );
    arm(&mut ui);
    let continuation = ui.action_menu_input_context().unwrap();
    assert!(ui.capture_action_menu_key_for(
        continuation,
        KeyEvent::press(KeyCode::Down, Modifiers::NONE)
    ));
    assert_eq!(ui.view().action_menu().unwrap().selected, 2);
    assert_eq!(ui.menus.menu.confirmed(), None);

    arm(&mut ui);
    let ready = ui.action_menu_input_context().unwrap();
    assert!(ready.activation_ready());
    assert!(
        ui.capture_action_menu_key_for(ready, KeyEvent::press(KeyCode::Enter, Modifiers::NONE))
    );
    assert!(ui.menus.menu.confirmed().is_some());
}

#[test]
fn first_geometry_block_refuses_but_an_admitted_child_resumes_with_parent_state() {
    use crate::{
        client_view::{KeyCode, KeyEvent, Modifiers},
        tui_presentation::PresentationPhase,
    };

    let (mut refused, _, _, _) = tests::move_membership::ready_ui();
    assert!(refused.open_action_menu(MenuTarget::Global, None, false));
    refused.after_action_menu_draw(Some(ModalPresentation::NotPresented));
    assert!(!refused.action_menu_capture_claim());

    let (mut ui, _, _, _) = tests::move_membership::ready_ui();
    assert!(ui.open_action_menu(MenuTarget::Global, Some((7, 4).into()), false));
    let token = arm(&mut ui);
    let child_row = action_row(&ui, MenuAction::TabCreation);
    activate_row(&mut ui, token, child_row);
    assert_eq!(ui.take_action_menu_action(false), None);
    assert_eq!(ui.menus.parent_count(), 1);
    arm(&mut ui);
    ui.capture_action_menu_key(KeyEvent::press(KeyCode::Down, Modifiers::NONE));
    arm(&mut ui);
    let (source, selected, first, anchor) = {
        let view = ui.view();
        let retained = view.action_menu().unwrap();
        (
            retained.source.clone(),
            retained.selected,
            retained.first,
            retained.anchor,
        )
    };

    ui.after_action_menu_draw(Some(ModalPresentation::NotPresented));
    let suspended = ui.action_menu_input_context().unwrap();
    assert_eq!(suspended.phase(), PresentationPhase::Suspended);
    assert_eq!(ui.menus.parent_count(), 1);
    {
        let view = ui.view();
        let retained = view.action_menu().unwrap();
        assert_eq!(retained.source, &source);
        assert_eq!(
            (retained.selected, retained.first, retained.anchor),
            (selected, first, anchor)
        );
    }
    assert!(
        ui.capture_action_menu_key_for(suspended, KeyEvent::press(KeyCode::Down, Modifiers::NONE))
    );
    assert_eq!(ui.view().action_menu().unwrap().selected, selected);

    arm(&mut ui);
    assert_eq!(
        ui.action_menu_input_context().unwrap().phase(),
        PresentationPhase::Admitted
    );
    let child = ui.action_menu_input_context().unwrap();
    assert!(ui.capture_action_menu_key_for(child, KeyEvent::press(KeyCode::Esc, Modifiers::NONE)));
    {
        let view = ui.view();
        let parent = view.action_menu().unwrap();
        assert_eq!(parent.source, &MenuTarget::Global);
        assert_eq!(parent.anchor, Some((7, 4).into()));
        assert_eq!(parent.actions[parent.selected], MenuAction::TabCreation);
    }
    assert_eq!(
        ui.action_menu_input_context().unwrap().readiness(),
        crate::action_menu::MenuInputReadiness::Recovery
    );
}

#[test]
fn typed_menu_pointer_activation_requires_the_exact_source_and_token() {
    let (mut ui, _, _, _) = tests::move_membership::ready_ui();
    assert!(ui.open_action_menu(MenuTarget::Global, None, false));
    let token = arm(&mut ui);
    let row = action_row(&ui, MenuAction::Help);

    ui.activate_action_menu_row(&MenuTarget::Create(None), token, row);
    assert_eq!(ui.menus.menu.confirmed(), None);
    ui.activate_action_menu_control(&MenuTarget::Create(None), token, MenuControl::Close);
    assert!(ui.action_menu_capture_claim());

    ui.activate_action_menu_row(&MenuTarget::Global, token, row);
    assert_eq!(ui.menus.menu.confirmed(), Some(MenuAction::Help));
    assert_eq!(
        ui.take_action_menu_action(false)
            .map(|command| command.intent()),
        Some(ClientAction::OpenShortcutHelp)
    );
    assert!(!ui.action_menu_capture_claim());
}

#[test]
fn typed_menu_hover_requires_a_drawn_source_and_never_enters_a_submenu_or_action() {
    let (mut ui, _, _, _) = tests::move_membership::ready_ui();
    assert!(ui.open_action_menu(MenuTarget::Global, Some((7, 4).into()), false));
    let token = ui.view().action_menu().unwrap().token;
    let row = action_row(&ui, MenuAction::TabCreation);
    assert!(!ui.hover_action_menu_row(&MenuTarget::Global, token, row));
    arm(&mut ui);
    let other = action_row(&ui, MenuAction::Help);
    assert!(ui.hover_action_menu_row(&MenuTarget::Global, token, other));
    assert!(!ui.hover_action_menu_row(&MenuTarget::Create(None), token, row));
    assert!(ui.hover_action_menu_row(&MenuTarget::Global, token, row));
    assert_eq!(ui.view().action_menu().unwrap().selected, row);
    assert_eq!(ui.view().action_menu().unwrap().source, &MenuTarget::Global);
    assert_eq!(ui.view().action_menu().unwrap().anchor, Some((7, 4).into()));
    assert!(ui.menus.parent_count() == 0, "hover cannot open a submenu");
    assert_eq!(ui.menus.menu.confirmed(), None);
    assert_eq!(ui.take_action_menu_action(false), None);
    assert!(matches!(ui.menus.intent().unwrap().stage(), Stage::Open));
    ui.cancel_action_menu();
    assert!(!ui.hover_action_menu_row(&MenuTarget::Global, token, row));
}

#[test]
fn hovering_menu_families_keeps_the_active_workspace_tab_and_selection() {
    let (mut ui, mailbox, _, _) = tests::context_selection::ready(false);
    let state = ui.state.as_ref().unwrap();
    let tab = state.cache().tabs()[1].tab_id().clone();
    let workspace = state.cache().navigation()[1].location.clone();
    let pane = state.focus_pane_ref().unwrap();
    let project = tests::context_selection::snapshot(false)
        .navigation
        .projects[0]
        .project_id
        .clone();
    let create = ui.view().tab_create_target().unwrap();
    let origin = Origin::capture(ui.state.as_ref());
    for target in [
        MenuTarget::Global,
        MenuTarget::Create(None),
        MenuTarget::Create(Some(project.clone())),
        MenuTarget::Project(project),
        MenuTarget::Workspace(workspace),
        MenuTarget::Tab(tab.clone()),
        MenuTarget::Pane(pane),
        MenuTarget::TabCreate(create),
        MenuTarget::TabGroupChoices(tab),
    ] {
        assert!(ui.open_action_menu(target.clone(), Some((7, 4).into()), false));
        let token = arm(&mut ui);
        let (source, enabled) = {
            let view = ui.view();
            let menu = view.action_menu().unwrap();
            (
                menu.source.clone(),
                menu.unavailable
                    .iter()
                    .enumerate()
                    .filter_map(|(row, reason)| reason.is_none().then_some(row))
                    .collect::<Vec<_>>(),
            )
        };
        assert!(!enabled.is_empty(), "{target:?}");
        for row in enabled {
            let (previous, hidden) = {
                let view = ui.view();
                let menu = view.action_menu().unwrap();
                (menu.selected, menu.candidate_hidden)
            };
            assert_eq!(
                ui.hover_action_menu_row(&source, token, row),
                row != previous || hidden,
                "{target:?} row {row}"
            );
            assert_eq!(ui.view().action_menu().unwrap().selected, row);
            assert!(!ui.view().action_menu().unwrap().candidate_hidden);
            assert_eq!(Origin::capture(ui.state.as_ref()), origin);
            assert_eq!(ui.menus.menu.confirmed(), None);
            assert_eq!(ui.take_action_menu_action(false), None);
            assert!(mailbox.try_recv().unwrap().is_none());
        }
        ui.cancel_action_menu();
    }
}

#[test]
fn notifications_follow_the_real_pending_admitted_and_released_menu_capture() {
    use crate::tui_presentation::PresentationPhase;

    let (mut ui, _, _, _) = tests::move_membership::ready_ui();
    let shell = ClientView::new(80);
    let available = |ui: &WorkbenchClientUi| {
        crate::workbench_client::notification_available(&shell, Some(ui.view()))
    };

    assert!(available(&ui));
    assert!(ui.open_action_menu(MenuTarget::Global, None, false));
    assert_eq!(
        ui.action_menu_input_context().unwrap().phase(),
        PresentationPhase::Pending
    );
    assert!(ui.action_menu_capture_claim());
    assert!(!available(&ui));

    arm(&mut ui);
    assert_eq!(
        ui.action_menu_input_context().unwrap().phase(),
        PresentationPhase::Admitted
    );
    assert!(ui.action_menu_capture_claim());
    assert!(!available(&ui));

    ui.cancel_action_menu();
    assert!(!ui.action_menu_capture_claim());
    assert!(available(&ui));
}

#[test]
fn global_menu_renders_the_resolved_configured_binding_with_ascii_fallback() {
    use crate::{
        client_view::{ClientView, KeyLabelGlyphs, PrefixAction},
        tui_hit_map::HitTarget,
        tui_theme::GlyphSet,
    };
    use orchbee_local_runtime::{KeyChord, KeyOverride, KeySettings, KeyTrigger};

    let chord = |key: &str, ctrl| KeyChord {
        key: key.into(),
        ctrl,
        alt: false,
        shift: false,
        super_key: false,
    };
    let (mut ui, _, _, _) = tests::move_membership::ready_ui();
    ui.observe_geometry(Geometry::new(120, 30, 8, 16).unwrap());
    assert!(ui.open_action_menu(MenuTarget::Global, None, false));
    ui.menus.menu.select_action(MenuAction::Help);
    let help_row = action_row(&ui, MenuAction::Help);
    let mut view = ClientView::new(120);
    view.configure_keys(&KeySettings {
        prefix: chord("a", true),
        overrides: vec![KeyOverride {
            action: "help",
            triggers: vec![KeyTrigger {
                prefix: true,
                chord: chord("left", false),
            }],
        }],
        commands: Vec::new(),
    });
    let expected = view
        .bindings()
        .action_labels_with_glyphs(PrefixAction::ShowHelp, KeyLabelGlyphs::Ascii)
        .into_iter()
        .next()
        .unwrap();
    assert_eq!(expected, "Ctrl+a Left");

    let theme = Theme::terminal_native().with_glyphs(GlyphSet::ASCII);
    let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
    let mut hits = HitMapBuilder::new();
    let mut receipt = DrawReceipt::default();
    terminal
        .draw(|frame| {
            receipt = crate::tui_render::render_terminal_shell(
                frame,
                &view,
                Locale::EnUs,
                Some(ui.view()),
                &theme,
                &mut hits,
            );
        })
        .unwrap();
    ui.after_close_draw(receipt);
    let hits = hits.finish();
    let y = (0..30)
        .find(|y| {
            (0..120).any(|x| {
                matches!(
                    hits.hit_target_at(x, *y),
                    Some(HitTarget::ActionMenuRow { row, .. }) if *row == help_row
                )
            })
        })
        .unwrap();
    let row = (0..120)
        .map(|x| terminal.backend().buffer()[(x, y)].symbol())
        .collect::<String>();
    assert!(row.contains(&expected), "{row:?}");
}

#[test]
fn pointer_menu_uses_the_full_viewport_while_keyboard_keeps_content_centering() {
    use crate::{client_view::ClientView, tui_hit_map::HitTarget};
    use ratatui::layout::Rect;
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for theme in [
            Theme::terminal_native(),
            Theme::orchbee_light(),
            Theme::terminal_native().quantize(crate::tui_theme::ColorLevel::Monochrome),
        ] {
            for (width, height) in [(120, 40), (80, 24), (70, 16), (50, 10), (20, 7)] {
                for anchor in [
                    Some((0, 0)),
                    Some((3, 4)),
                    Some((width - 1, height - 1)),
                    None,
                ] {
                    let (mut ui, _, _, _) = tests::move_membership::ready_ui();
                    ui.observe_geometry(Geometry::new(width, height, 8, 16).unwrap());
                    assert!(ui.open_action_menu(MenuTarget::Global, anchor.map(Into::into), false));
                    let mut view = ClientView::new(width);
                    view.machines.available = true;
                    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                    let mut hits = HitMapBuilder::new();
                    let mut receipt = crate::workbench_client::DrawReceipt::default();
                    terminal
                        .draw(|frame| {
                            receipt = crate::tui_render::render_terminal_shell(
                                frame,
                                &view,
                                locale,
                                Some(ui.view()),
                                &theme,
                                &mut hits,
                            );
                        })
                        .unwrap();
                    ui.after_close_draw(receipt);
                    let hits = hits.finish();
                    let cells: Vec<_> = (0..height)
                        .flat_map(|y| (0..width).map(move |x| (x, y)))
                        .filter(|(x, y)| {
                            *y < height - 1
                                && matches!(
                                    hits.hit_target_at(*x, *y),
                                    Some(
                                        HitTarget::ActionMenuRow { .. } | HitTarget::OverlaySurface
                                    )
                                )
                        })
                        .collect();
                    if cells.is_empty() {
                        let (_, y) = anchor.expect("keyboard menu has enough centered space");
                        let above = y.min(height - 1);
                        let below = (height - 1).saturating_sub(y.saturating_add(1));
                        assert!(above.max(below) < 6, "{width}x{height} {anchor:?}");
                        assert!(!ui.action_menu_active(), "a refused draw retires the menu");
                        assert_eq!(ui.menus.menu.confirmed(), None);
                        continue;
                    }
                    let left = cells.iter().map(|(x, _)| *x).min().unwrap();
                    let top = cells.iter().map(|(_, y)| *y).min().unwrap();
                    let right = cells.iter().map(|(x, _)| *x).max().unwrap() + 1;
                    let bottom = cells.iter().map(|(_, y)| *y).max().unwrap() + 1;
                    let selected = ui.view().action_menu().unwrap().selected;
                    let selected_row_y = (0..height)
                        .find(|y| {
                            (0..width).any(|x| {
                                matches!(
                                    hits.hit_target_at(x, *y),
                                    Some(HitTarget::ActionMenuRow { row, .. }) if *row == selected
                                )
                            })
                        })
                        .expect("the selected action row is painted");
                    let buffer = terminal.backend().buffer();
                    assert_ne!(
                        buffer[(left, top)].symbol(),
                        "[",
                        "sidebar restore chrome must stay below the menu frame"
                    );
                    let selection = theme.style(crate::tui_theme::Role::SurfaceSelection);
                    if anchor.is_some() {
                        assert!(ui.view().action_menu().unwrap().candidate_hidden);
                        assert!(
                            (top..bottom).all(|y| {
                                buffer[(left + 1, y)].symbol() != theme.glyphs.navigation_marker
                            }),
                            "a pointer-opened menu starts without a candidate"
                        );
                    } else {
                        assert_eq!(
                            buffer[(left + 1, selected_row_y)].symbol(),
                            theme.glyphs.navigation_marker
                        );
                        let mut x = left + 1;
                        while x < right - 1 {
                            let cell = &buffer[(x, selected_row_y)];
                            assert_eq!(cell.bg, selection.bg.unwrap());
                            // Wide-character continuation cells inherit the lead's visible style.
                            x += u16::try_from(
                                crate::terminal_text::display_width(cell.symbol()).max(1),
                            )
                            .unwrap();
                        }
                    }
                    assert!(
                        right <= width && bottom < height,
                        "menu must leave status visible"
                    );
                    if let Some((x, y)) = anchor {
                        assert_eq!(
                            left,
                            x.min(width - (right - left)),
                            "{width}x{height} {anchor:?}"
                        );
                        assert!(top > y || bottom <= y, "menu must not cover its anchor");
                    } else {
                        // The unanchored menu centers in the whole window.
                        let content = Rect::new(0, 0, width, height);
                        assert_eq!(left, content.x + (content.width - (right - left)) / 2);
                        assert_eq!(top, content.y + (content.height - (bottom - top)) / 2);
                    }
                    assert!(ui.action_menu_active());
                    assert_eq!(ui.menus.menu.confirmed(), None);
                }
            }
        }
    }
}

#[test]
fn ux_menu_released_pointer_rejects_disabled_pointer_activation_without_handoff() {
    let (mut ui, mailbox, _, _) = tests::move_membership::ready_ui();
    assert!(ui.open_action_menu(MenuTarget::Global, None, true));
    let row = ui
        .view()
        .action_menu()
        .unwrap()
        .actions
        .iter()
        .position(|a| *a == MenuAction::ReleasePointer)
        .expect("released pointer remains discoverable");
    let token = arm(&mut ui);
    activate_row(&mut ui, token, row);
    assert!(ui.action_menu_active());
    assert_ne!(ui.view().action_menu().unwrap().selected, row);
    assert_eq!(ui.menus.menu.confirmed(), None);
    assert_eq!(ui.take_action_menu_action(true), None);
    assert!(mailbox.try_recv().unwrap().is_none());
}

#[test]
fn ux_menu_disabled_reason_and_exit_survive_short_geometry_in_both_locales() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for (width, height) in [(120, 40), (80, 24), (80, 20), (50, 10), (20, 7)] {
            for theme in [
                Theme::terminal_native(),
                Theme::orchbee_light(),
                Theme::orchbee_dark(),
                Theme::orchbee_light().quantize(crate::tui_theme::ColorLevel::Ansi256),
                Theme::orchbee_dark().quantize(crate::tui_theme::ColorLevel::Ansi256),
                Theme::orchbee_light().quantize(crate::tui_theme::ColorLevel::Ansi16),
                Theme::orchbee_dark().quantize(crate::tui_theme::ColorLevel::Ansi16),
            ] {
                let (mut ui, _, _, _) = tests::move_membership::ready_ui();
                ui.observe_geometry(Geometry::new(width, height, 8, 16).unwrap());
                assert!(ui.open_action_menu(MenuTarget::Global, None, true));
                let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                let mut hits = HitMapBuilder::new();
                for select in [false, true] {
                    if select {
                        for key in [
                            crate::client_view::KeyCode::End,
                            crate::client_view::KeyCode::Up,
                        ] {
                            ui.capture_action_menu_key(crate::client_view::KeyEvent::press(
                                key,
                                crate::client_view::Modifiers::NONE,
                            ));
                        }
                    }
                    let mut receipt = crate::workbench_client::DrawReceipt::default();
                    terminal
                        .draw(|frame| {
                            receipt = crate::tui_render::render_terminal_shell(
                                frame,
                                &crate::client_view::ClientView::new(width),
                                locale,
                                Some(ui.view()),
                                &theme,
                                &mut hits,
                            );
                        })
                        .unwrap();
                    ui.after_close_draw(receipt);
                }
                let text: String = terminal
                    .backend()
                    .buffer()
                    .content()
                    .iter()
                    .map(|c| c.symbol())
                    .collect();
                let reason = locale.menu_unavailable(MenuUnavailable::PointerReleased, true);
                let compact_text = text.replace(' ', "");
                assert!(
                    compact_text.contains(&reason.replace(' ', ""))
                        || compact_text.contains(
                            &locale
                                .menu_unavailable(MenuUnavailable::PointerReleased, false)
                                .replace(' ', "")
                        ),
                    "{locale:?} {width}x{height}: {text:?}"
                );
                assert!(text.contains("Esc"));
                let view = ui.view();
                let menu = view.action_menu().unwrap();
                assert_eq!(menu.actions[menu.selected], MenuAction::ReleasePointer);
                assert!(text.contains(&format!("{}/{}", menu.selected + 1, menu.actions.len())));
                assert!(!text.contains("Enter"));
                let hits = hits.finish();
                let (x, y) = (0..height)
                    .find_map(|y| {
                        (0..width)
                            .find(|x| {
                                matches!(
                                    hits.hit_target_at(*x, y),
                                    Some(crate::tui_hit_map::HitTarget::ActionMenuRow {
                                        row,
                                        ..
                                    }) if *row == menu.selected
                                )
                            })
                            .map(|x| (x, y))
                    })
                    .expect("selected disabled row");
                let cell = &terminal.backend().buffer()[(x + 1, y)];
                let disabled_selection = theme.content_on_surface(
                    crate::tui_theme::Role::SurfaceOverlay,
                    Some(crate::tui_theme::Role::SurfaceSelection),
                    Some(crate::tui_theme::Role::TextDisabled),
                    theme.style(crate::tui_theme::Role::TextDisabled),
                );
                assert_eq!(
                    cell.style(),
                    disabled_selection,
                    "disabled text must retain its semantic role on the selected surface"
                );
                assert!(
                    terminal
                        .backend()
                        .buffer()
                        .content()
                        .iter()
                        .any(|c| c.fg == theme.color(crate::tui_theme::Role::TextDisabled))
                );
                if locale == Locale::EnUs
                    && (width, height) == (120, 40)
                    && theme.name == "terminal-native"
                {
                    crate::tui_render::test_capture::capture_screen(
                        &mut terminal,
                        "alignment-action-menu-disabled-en-us-120x40-terminal-native",
                        locale,
                        &theme,
                    );
                }
                ui.capture_action_menu_key(crate::client_view::KeyEvent::press(
                    crate::client_view::KeyCode::Enter,
                    crate::client_view::Modifiers::NONE,
                ));
                assert!(ui.action_menu_active());
                assert_eq!(ui.take_action_menu_action(true), None);
            }
        }
    }
}

#[test]
fn ux_menu_projection_keeps_selection_and_hit_indices_in_the_first_resized_paint() {
    fn paint(
        ui: &mut WorkbenchClientUi,
        terminal: &mut Terminal<TestBackend>,
        width: u16,
        locale: Locale,
    ) -> crate::tui_hit_map::FrameHitMap {
        let mut hits = HitMapBuilder::new();
        let mut receipt = crate::workbench_client::DrawReceipt::default();
        terminal
            .draw(|frame| {
                receipt = crate::tui_render::render_terminal_shell(
                    frame,
                    &crate::client_view::ClientView::new(width),
                    locale,
                    Some(ui.view()),
                    &Theme::orchbee_light(),
                    &mut hits,
                );
            })
            .unwrap();
        ui.after_close_draw(receipt);
        hits.finish()
    }
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let (mut ui, _, _, _) = tests::move_membership::ready_ui();
        ui.observe_geometry(Geometry::new(120, 60, 8, 16).unwrap());
        assert!(ui.open_action_menu(MenuTarget::Global, None, true));
        let mut full = Terminal::new(TestBackend::new(120, 60)).unwrap();
        paint(&mut ui, &mut full, 120, locale);
        for key in [
            crate::client_view::KeyCode::End,
            crate::client_view::KeyCode::Up,
        ] {
            ui.capture_action_menu_key(crate::client_view::KeyEvent::press(
                key,
                crate::client_view::Modifiers::NONE,
            ));
        }
        paint(&mut ui, &mut full, 120, locale);
        let selected = ui.view().action_menu().unwrap().selected;
        assert_eq!(
            ui.view().action_menu().unwrap().actions[selected],
            MenuAction::ReleasePointer
        );
        // Vary the pure projection's frame, retaining the admitted menu source.
        // Native geometry observations separately revoke source-bound menus.
        let mut small = Terminal::new(TestBackend::new(50, 10)).unwrap();
        let hits = paint(&mut ui, &mut small, 50, locale);
        let (x, y) = (0..10)
            .find_map(|y| {
                (0..50)
                    .find(|x| {
                        matches!(
                            hits.hit_target_at(*x, y),
                            Some(crate::tui_hit_map::HitTarget::ActionMenuRow { row, .. }) if *row == selected
                        )
                    })
                    .map(|x| (x, y))
            })
            .expect("selected row visible in the first resized paint");
        let Some(crate::tui_hit_map::HitTarget::ActionMenuRow { token, row }) =
            hits.hit_target_at(x + 2, y)
        else {
            panic!("selected label retains a painted menu hit");
        };
        assert_eq!(*row, selected);
        assert_eq!(*token, ui.view().action_menu().unwrap().token);
        assert!(ui.view().action_menu().unwrap().first > 0);
        activate_row(&mut ui, *token, *row);
        assert!(ui.action_menu_active());
        assert_eq!(ui.take_action_menu_action(true), None);
        paint(&mut ui, &mut full, 120, locale);
        let view = ui.view();
        let menu = view.action_menu().unwrap();
        assert_eq!(menu.selected, selected);
        assert_eq!(menu.first, 0, "all actions fit after growing the frame");
    }
}

#[test]
fn ux_menu_configured_workspace_directory_refuses_before_navigation() {
    use orchbee_workbench_protocol::{
        Directory, WorkbenchDirectoryId, WorkbenchDirectoryPath, WorkerId,
    };
    let (mut ui, mailbox, _, _) = tests::context_selection::ready(true);
    let mut snapshot = tests::context_selection::snapshot(true);
    snapshot.terminal_contexts[0].tabs.clear();
    let directory_id = WorkbenchDirectoryId::parse("00000000000000000000000000000901").unwrap();
    snapshot.navigation.directories.push(Directory {
        directory_id: directory_id.clone(),
        worker_id: WorkerId::parse("00000000000000000000000000000902").unwrap(),
        path: WorkbenchDirectoryPath::parse("C:/project").unwrap(),
    });
    snapshot.navigation.workspaces[1].primary_directory_id = Some(directory_id);
    let cache = crate::workbench_projection::project_preferred_workspace(&snapshot).unwrap();
    ui.state = Some(
        WorkbenchClientState::bootstrap(cache, 1, Geometry::new(80, 24, 8, 16).unwrap())
            .unwrap()
            .0,
    );
    let origin = Origin::capture(ui.state.as_ref());
    let location = ui.state.as_ref().unwrap().cache().navigation()[1]
        .location
        .clone();
    assert!(ui.open_action_menu(MenuTarget::Workspace(location), None, false));
    assert!(
        !ui.view()
            .action_menu()
            .unwrap()
            .actions
            .contains(&MenuAction::WorkspaceDirectory)
    );
    assert_eq!(
        action_availability(
            ui.capabilities,
            ui.state.as_ref(),
            ui.menus.intent().unwrap().target(),
            MenuAction::WorkspaceDirectory,
            false,
            None,
            CopyReadiness::Hidden,
        ),
        ActionAvailability::Hidden,
        "Set Directory… joins only a Workspace without one"
    );
    assert!(ui.action_menu_active());
    assert_eq!(ui.menus.menu.confirmed(), None);
    assert_eq!(ui.take_action_menu_action(false), None);
    assert_eq!(Origin::capture(ui.state.as_ref()), origin);
    assert!(mailbox.try_recv().unwrap().is_none());
}

#[test]
fn hidden_action_injected_after_admission_is_refused_by_the_coordinator() {
    let (mut ui, mailbox, _, _) = tests::move_membership::ready_ui();
    let tab_id = ui.state.as_ref().unwrap().cache().tabs()[0]
        .tab_id()
        .clone();
    ui.capabilities = crate::tui_capabilities::Capabilities::none();
    assert!(ui.open_action_menu(MenuTarget::Tab(tab_id), None, false));

    // Model a row this host cannot run reaching the menu after target admission but before the
    // final query. The coordinator must not turn it into a ClientAction.
    ui.menus.menu.cancel();
    assert!(ui.menus.menu.open(
        "stale host capability".into(),
        vec![MenuAction::Machines],
        None,
    ));
    assert_eq!(ui.take_action_menu_action(false), None);
    assert!(!ui.action_menu_active());
    assert!(mailbox.try_recv().unwrap().is_none());
}

#[test]
fn ux_status_prefix_remains_visible_with_the_machine_picker_available() {
    use crate::{
        client_view::{
            Binding, ClientActionDisposition, KeyCode, KeyEvent, KeyLabelGlyphs, LabelStyle,
            Modifiers, PrefixInput, format_binding_with_glyphs,
        },
        tui_theme::GlyphMode,
    };
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for (width, height) in [(120, 40), (80, 24), (80, 20), (50, 10), (20, 5), (19, 4)] {
            for theme in [
                Theme::terminal_native(),
                Theme::rgb_test_dark(),
                Theme::orchbee_light(),
                Theme::orchbee_dark(),
            ] {
                let (mut ui, _, _, _) = tests::move_membership::ready_ui();
                ui.observe_geometry(Geometry::new(width, height, 8, 16).unwrap());
                let mut view = crate::client_view::ClientView::new(width);
                view.machines.available = true;
                let prefix = view.decide(PrefixInput::Key(KeyEvent::press(
                    KeyCode::Char('b'),
                    Modifiers::CTRL,
                )));
                view.commit(prefix, ClientActionDisposition::KeepNavigator);
                let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                terminal
                    .draw(|frame| {
                        let mut hits = HitMapBuilder::new();
                        crate::tui_render::render_terminal_shell(
                            frame,
                            &view,
                            locale,
                            Some(ui.view()),
                            &theme,
                            &mut hits,
                        );
                        view.machines.render(frame, locale, &theme, &mut hits);
                    })
                    .unwrap();
                let row: String = (crate::tui_layout::project_shell_areas(
                    ratatui::layout::Rect::new(0, 0, width, height),
                    view.sidebar_surface(),
                )
                .status
                .x..width)
                    .map(|x| terminal.backend().buffer()[(x, height - 1)].symbol())
                    .collect();
                let glyphs = match theme.glyphs.mode() {
                    GlyphMode::Unicode => KeyLabelGlyphs::Unicode,
                    GlyphMode::Simplified => KeyLabelGlyphs::Ascii,
                };
                let prefix = format!(
                    "{} {}",
                    locale.status().prefix,
                    format_binding_with_glyphs(
                        &view.bindings().prefix,
                        LabelStyle::for_host(),
                        glyphs,
                    )
                );
                // The narrowest floating chip keeps the cancellation key ahead of the prefix key.
                let expected = if (width, height) == (20, 5) {
                    locale.status().prefix.to_owned()
                } else {
                    prefix
                };
                assert!(
                    row.replace(' ', "").contains(&expected.replace(' ', "")),
                    "{locale:?} {width}x{height}: {row:?}"
                );
                let cancel = format_binding_with_glyphs(
                    &Binding::new(KeyCode::Esc, Modifiers::NONE),
                    LabelStyle::for_host(),
                    glyphs,
                );
                assert!(
                    row.contains(&cancel),
                    "{locale:?} {width}x{height}: {row:?}"
                );
            }
        }
    }
}

#[test]
fn ux_presented_menu_clears_the_underlying_status_without_duplicate_keys() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for (width, height) in [(120, 40), (80, 24), (80, 20), (50, 10), (20, 6)] {
            let (mut ui, _, _, _) = tests::move_membership::ready_ui();
            ui.observe_geometry(Geometry::new(width, height, 8, 16).unwrap());
            assert!(ui.open_action_menu(MenuTarget::Global, None, false));
            let mut view = crate::client_view::ClientView::new(width);
            view.machines.available = true;
            view.startup_notice(crate::client_view::Notice::UnknownPrefix);
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal
                .draw(|frame| {
                    let mut hits = HitMapBuilder::new();
                    let theme = Theme::orchbee_light();
                    crate::tui_render::render_terminal_shell(
                        frame,
                        &view,
                        locale,
                        Some(ui.view()),
                        &theme,
                        &mut hits,
                    );
                    view.machines.render(frame, locale, &theme, &mut hits);
                })
                .unwrap();
            let row: String = (crate::tui_layout::project_shell_areas(
                ratatui::layout::Rect::new(0, 0, width, height),
                view.sidebar_surface(),
            )
            .status
            .x..width)
                .map(|x| terminal.backend().buffer()[(x, height - 1)].symbol())
                .collect();
            // The menu may reach the last row; no status paints beside it.
            let notice = locale.shell_notice(Some(crate::client_view::Notice::UnknownPrefix));
            assert!(
                !row.replace(' ', "").contains(&notice.replace(' ', "")),
                "{locale:?} {width}x{height}: {row:?}"
            );
            assert!(
                !row.contains("Ctrl+"),
                "{locale:?} {width}x{height}: {row:?}"
            );
        }
    }
}

#[test]
fn ux_status_short_menu_exposes_its_selection_position() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for width in [70, 20] {
            let (mut ui, _, _, _) = tests::move_membership::ready_ui();
            ui.observe_geometry(Geometry::new(width, 8, 8, 16).unwrap());
            assert!(ui.open_action_menu(MenuTarget::Global, None, false));
            let count = ui.view().action_menu().unwrap().actions.len();
            let mut terminal = Terminal::new(TestBackend::new(width, 8)).unwrap();
            for selected in [1, count] {
                let mut receipt = crate::workbench_client::DrawReceipt::default();
                terminal
                    .draw(|frame| {
                        receipt = crate::tui_render::render_terminal_shell(
                            frame,
                            &crate::client_view::ClientView::new(width),
                            locale,
                            Some(ui.view()),
                            &Theme::terminal_native(),
                            &mut HitMapBuilder::new(),
                        );
                    })
                    .unwrap();
                let text: String = terminal
                    .backend()
                    .buffer()
                    .content()
                    .iter()
                    .map(|c| c.symbol())
                    .collect();
                assert!(
                    text.contains(&format!("{selected}/{count}")),
                    "{locale:?}: {text:?}"
                );
                assert!(text.contains("Esc"));
                if width > 20 {
                    assert!(text.contains("Enter"), "{locale:?} {width}: {text:?}");
                } else {
                    assert!(!text.contains("Enter"));
                }
                ui.after_close_draw(receipt);
                assert_eq!(ui.view().action_menu().unwrap().selected + 1, selected);
                if selected == count {
                    assert!(ui.view().action_menu().unwrap().first > 0);
                }
                ui.capture_action_menu_key(crate::client_view::KeyEvent::press(
                    crate::client_view::KeyCode::End,
                    crate::client_view::Modifiers::NONE,
                ));
            }
        }
    }
}

#[test]
fn ux_status_menu_uses_a_search_cursor_and_cancel_restores_the_pane_cursor() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let (mut ui, _, _, _) = tests::move_membership::ready_ui();
        super::super::tests::viewport::install_projection(&mut ui, Some((9, false, 24)), 0);
        ui.observe_geometry(Geometry::new(120, 40, 8, 16).unwrap());
        let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
        for capturing in [false, true, false] {
            if capturing {
                assert!(ui.open_action_menu(MenuTarget::Global, None, false));
            } else {
                ui.cancel_action_menu();
            }
            let mut hits = HitMapBuilder::new();
            terminal
                .draw(|frame| {
                    crate::tui_render::render_terminal_shell(
                        frame,
                        &crate::client_view::ClientView::new(120),
                        locale,
                        Some(ui.view()),
                        &Theme::orchbee_light(),
                        &mut hits,
                    );
                })
                .unwrap();
            assert!(
                terminal.backend().cursor_visible(),
                "the search field or restored pane owns a visible cursor"
            );
            let cursor = terminal.get_cursor_position().unwrap();
            let hits = hits.finish();
            let target = hits.hit_target_at(cursor.x, cursor.y);
            if capturing {
                assert_eq!(target, Some(&crate::tui_hit_map::HitTarget::OverlaySurface));
            } else {
                assert_ne!(target, Some(&crate::tui_hit_map::HitTarget::OverlaySurface));
            }
        }
    }
}

#[test]
fn pane_open_and_cancel_preserve_focus_then_one_confirmed_action_focuses_exact_peer() {
    let (mut ui, mailbox, clock, _) = tests::publication_hint::ready_split_ui();
    tests::publication_hint::attach_split_peer(&mut ui, &mailbox, &clock);
    let state = ui.state.as_ref().unwrap();
    let old = state.focus_pane_ref().unwrap();
    let peer = state.cache().tabs()[0].panel(1).unwrap().panel_id();
    let peer = state.pane_by_panel(peer).unwrap().pane_ref();
    assert!(ui.open_action_menu(MenuTarget::Pane(peer.clone()), Some((9, 9).into()), false));
    assert_eq!(
        ui.state.as_ref().unwrap().focus_pane_ref(),
        Some(old.clone())
    );
    arm(&mut ui);
    for _action in [MenuAction::Zoom, MenuAction::ClosePane] {
        assert_eq!(
            ui.view().action_menu().unwrap().selected,
            0,
            "hover is transient presentation and never keyboard selection"
        );
        assert_eq!(
            ui.state.as_ref().unwrap().focus_pane_ref(),
            Some(old.clone())
        );
        assert!(ui.menus.intent().unwrap().stage().deadline().is_none());
        assert_eq!(ui.take_action_menu_action(false), None);
        assert!(mailbox.try_recv().unwrap().is_none());
    }
    ui.cancel_action_menu();
    assert_eq!(ui.state.as_ref().unwrap().focus_pane_ref(), Some(old));
    assert_eq!(ui.take_action_menu_action(false), None);
    assert!(ui.open_action_menu(MenuTarget::Pane(peer.clone()), None, false));
    let row = action_row(&ui, MenuAction::ClosePane);
    assert_eq!(ui.view().action_menu().unwrap().selected, 0);
    let token = arm(&mut ui);
    activate_row(&mut ui, token, row);
    assert_eq!(ui.state.as_ref().unwrap().focus_pane_ref(), Some(peer));
    assert_eq!(
        ui.take_action_menu_action(false)
            .map(|command| command.intent()),
        Some(ClientAction::ClosePanel)
    );
    assert_eq!(ui.take_action_menu_action(false), None);
    assert!(!ui.action_menu_active());
    // Handoff only selected the existing close action, never confirmed or submitted it.
    assert!(ui.view().panel_close_confirmation().is_none());
    ui.apply_client_action(ClientAction::ClosePanel);
    assert!(ui.view().panel_close_confirmation().is_some());
}

#[test]
fn exact_incarnation_modal_priority_resize_and_timeout_revoke_unhanded_authority() {
    let (mut ui, _, _, elapsed) = tests::move_membership::ready_ui();
    let focus = ui.state.as_ref().unwrap().focus_pane_ref().unwrap();
    let stale = PaneRef::new(focus.pane().clone(), focus.incarnation() + 1);
    assert!(!ui.open_action_menu(MenuTarget::Pane(stale), None, false));
    ui.state.as_mut().unwrap().open_rename_draft().unwrap();
    assert!(!ui.open_action_menu(MenuTarget::Global, None, false));
    ui.apply_client_action(ClientAction::CancelClose);
    // Use an independently ready UI: the original rename owner retains cancellation semantics.
    let (mut ui, _, _, _) = tests::move_membership::ready_ui();
    assert!(ui.open_action_menu(MenuTarget::Global, None, false));
    arm(&mut ui);
    ui.observe_geometry(Geometry::new(120, 40, 8, 16).unwrap());
    assert!(ui.action_menu_active(), "resize retains the admitted owner");
    assert!(!ui.action_menu_input_context().unwrap().activation_ready());
    assert_eq!(ui.take_action_menu_action(false), None);
    ui.cancel_action_menu();
    assert!(ui.open_action_menu(MenuTarget::Global, None, false));
    arm(&mut ui);
    ui.cancel_action_menu();
    assert!(ui.open_action_menu(MenuTarget::Global, None, false));
    // Deadline tests use the fixture's injected clock, never wall-clock sleep.
    let (mut timed, _, _, time) = tests::move_membership::ready_ui();
    assert!(timed.open_action_menu(MenuTarget::Global, None, false));
    arm(&mut timed);
    time.store(31_000, std::sync::atomic::Ordering::SeqCst);
    assert!(!timed.drive_action_menu());
    assert!(timed.action_menu_active());
    assert!(timed.menus.intent().unwrap().stage().deadline().is_none());
    let row = timed
        .view()
        .action_menu()
        .unwrap()
        .actions
        .iter()
        .position(|action| *action == MenuAction::Help)
        .unwrap();
    let token = arm(&mut timed);
    activate_row(&mut timed, token, row);
    assert!(matches!(
        timed.menus.intent().unwrap().stage(),
        Stage::Offered(_, _)
    ));
    time.store(60_999, std::sync::atomic::Ordering::SeqCst);
    assert!(!timed.drive_action_menu());
    assert!(timed.action_menu_active());
    time.store(61_000, std::sync::atomic::Ordering::SeqCst);
    assert!(timed.drive_action_menu());
    assert!(!timed.action_menu_active());
    assert_eq!(timed.take_action_menu_action(false), None);
    assert_eq!(elapsed.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[test]
fn ux_menu_disabled_activation_does_not_start_an_execution_deadline() {
    let (mut ui, _, _, time) = tests::move_membership::ready_ui();
    assert!(ui.open_action_menu(MenuTarget::Global, None, true));
    let row = ui
        .view()
        .action_menu()
        .unwrap()
        .actions
        .iter()
        .position(|action| *action == MenuAction::ReleasePointer)
        .unwrap();
    let token = arm(&mut ui);
    time.store(31_000, std::sync::atomic::Ordering::SeqCst);
    activate_row(&mut ui, token, row);
    time.store(300_000, std::sync::atomic::Ordering::SeqCst);
    ui.drive_action_menu();
    assert!(ui.action_menu_active());
    assert!(matches!(ui.menus.intent().unwrap().stage(), Stage::Open));
    assert!(ui.menus.intent().unwrap().stage().deadline().is_none());
    assert_eq!(ui.take_action_menu_action(true), None);
}

#[test]
fn empty_workspace_navigation_uses_owned_refresh_and_exact_cleanup_then_original_rename() {
    let (mut ui, mailbox, clock, _) = tests::context_selection::ready(true);
    let location = ui.state.as_ref().unwrap().cache().navigation()[1]
        .location
        .clone();
    let old = Origin::capture(ui.state.as_ref());
    assert!(ui.open_action_menu(MenuTarget::Workspace(location.clone()), None, false));
    assert_eq!(Origin::capture(ui.state.as_ref()), old);
    let token = arm(&mut ui);
    let row = action_row(&ui, MenuAction::RenameWorkspace);
    activate_row(&mut ui, token, row);
    let &Stage::Context { generation, .. } = ui.menus.intent().unwrap().stage() else {
        panic!("owned refresh");
    };
    ui.menu_navigation_prepared(NavigationKey::Context(generation + 1), true);
    ui.menu_navigation_installed(NavigationKey::Context(generation + 1));
    assert_eq!(ui.take_action_menu_action(false), None);
    let lease = mailbox.try_recv().unwrap().unwrap();
    let QueuedCommand::Refresh(marker) = lease.record() else {
        panic!("one refresh");
    };
    assert_eq!(marker.generation(), generation);
    mailbox.mark_refresh_in_flight(generation).unwrap();
    let wait = mailbox
        .publish_refresh_result_before(
            generation,
            orchbee_workbench_protocol::WorkbenchSnapshotOutcome::Available {
                snapshot: tests::context_selection::snapshot(true),
            },
            marker.deadline(),
            &clock,
        )
        .unwrap();
    ui.tick();
    let Some(UiReply::Refresh(reply)) = mailbox.take_deposited_reply(&wait).unwrap() else {
        panic!("refresh reply");
    };
    let RefreshDecision::Cleanup(cleanup) = reply.decision() else {
        panic!("old viewer cleanup");
    };
    assert_eq!(ui.take_action_menu_action(false), None);
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::MembershipDetached {
                cleanup: cleanup.clone(),
                detached: cleanup.old().clone(),
            },
            super::super::super::mailbox::EventDeadline::TotalCleanup(cleanup.deadlines().total()),
            &clock,
        )
        .unwrap();
    ui.tick();
    mailbox.take_deposited_reply(&wait).unwrap().unwrap();
    mailbox.finish_lease(lease).unwrap();
    assert_eq!(ui.state.as_ref().unwrap().cache().location(), &location);
    assert_eq!(ui.view().phase(), DisplayPhase::Empty);
    let action = ui
        .take_action_menu_action(false)
        .expect("original action does not require Ready");
    assert_eq!(action.intent(), ClientAction::RequestWorkspaceRename);
    ui.apply_client_action(action);
    assert!(ui.view().setup_draft().is_some());
}

#[test]
fn toggles_recheck_frozen_prerequisites_and_foreign_navigation_drops_the_intent() {
    let (mut ui, _, _, _) = tests::move_membership::ready_ui();
    assert!(ui.open_action_menu(MenuTarget::Global, None, false));
    let token = arm(&mut ui);
    let release = ui
        .view()
        .action_menu()
        .unwrap()
        .actions
        .iter()
        .position(|action| *action == MenuAction::ReleasePointer)
        .unwrap();
    activate_row(&mut ui, token, release);
    assert_eq!(ui.take_action_menu_action(true), None);
    assert!(!ui.action_menu_active());
    assert!(ui.open_action_menu(MenuTarget::Global, None, false));
    arm(&mut ui);
    ui.apply_client_action(ClientAction::SelectTab(2));
    assert!(!ui.action_menu_active());
    assert_eq!(ui.take_action_menu_action(false), None);
}

#[test]
fn action_menu_renders_and_hits_only_presented_rows_in_both_locales_themes_and_sizes() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for theme in [
            Theme::terminal_native(),
            Theme::rgb_test_dark(),
            Theme::orchbee_light(),
            Theme::orchbee_dark(),
        ] {
            for (width, height) in [(120, 40), (80, 24), (80, 20), (50, 10), (12, 3)] {
                let (mut ui, _, _, _) = tests::move_membership::ready_ui();
                let geometry = Geometry::new(width, height, 8, 16).unwrap();
                let mut snapshot = tests::context_selection::snapshot(true);
                snapshot.navigation.workspaces[1].label = "猫 🐝 workspace".into();
                let cache =
                    crate::workbench_projection::project_preferred_workspace(&snapshot).unwrap();
                let location = cache.navigation()[1].location.clone();
                ui.state = Some(
                    WorkbenchClientState::bootstrap(cache, 10, geometry)
                        .unwrap()
                        .0,
                );
                ui.observe_geometry(geometry);
                assert!(ui.open_action_menu(
                    MenuTarget::Workspace(location),
                    Some((width - 1, height - 1).into()),
                    false
                ));
                let rename = action_row(&ui, MenuAction::RenameWorkspace);
                assert_eq!(ui.view().action_menu().unwrap().unavailable[rename], None);
                ui.menus.menu.select_action(MenuAction::RenameWorkspace);
                let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                let mut hits = HitMapBuilder::new();
                let mut receipt = DrawReceipt::default();
                let mut view = crate::client_view::ClientView::new(width);
                view.commit(
                    view.decide_action(ClientAction::OpenShortcutHelp),
                    ClientActionDisposition::KeepNavigator,
                );
                terminal
                    .draw(|frame| {
                        receipt = crate::tui_render::render_terminal_shell(
                            frame,
                            &view,
                            locale,
                            Some(ui.view()),
                            &theme,
                            &mut hits,
                        );
                    })
                    .unwrap();
                let text: String = terminal
                    .backend()
                    .buffer()
                    .content()
                    .chunks(usize::from(width))
                    .map(|row| {
                        let mut line = String::new();
                        let mut column = 0;
                        while let Some(cell) = row.get(column) {
                            line.push_str(cell.symbol());
                            column += crate::terminal_text::display_width(cell.symbol()).max(1);
                        }
                        line.push('\n');
                        line
                    })
                    .collect();
                ui.after_close_draw(receipt);
                if width == 12 {
                    assert!(!ui.action_menu_active());
                } else {
                    assert!(!text.contains("Esc"), "plain context menu: {text}");
                    assert!(
                        text.contains(
                            &locale.menu_object_action(&MenuAction::RenameWorkspace, None)
                        ),
                        "{locale:?} {width}x{height}: {text}"
                    );
                    assert!(ui.menus.menu.confirm(), "{locale:?} {width}x{height}");
                    let map = hits.finish();
                    assert!((0..height).all(|y| (0..width).all(|x| !matches!(
                        map.hit_target_at(x, y),
                        Some(crate::tui_hit_map::HitTarget::ActionMenuControl { .. })
                    ))));
                    let rows = (0..height)
                        .flat_map(|y| (0..width).map(move |x| (x, y)))
                        .filter(|(x, y)| {
                            matches!(
                                map.hit_target_at(*x, *y),
                                Some(crate::tui_hit_map::HitTarget::ActionMenuRow { .. })
                            )
                        })
                        .count();
                    assert!(rows > 0);
                }
                ui.cancel_action_menu();
                assert_eq!(
                    view.overlay(),
                    Some(crate::client_view::Overlay::ShortcutHelp)
                );
            }
        }
    }
}

#[test]
fn drawn_snapshot_source_and_owned_workspace_directory_replacement_are_fenced() {
    use orchbee_workbench_protocol::{
        Directory, WorkbenchDirectoryId, WorkbenchDirectoryPath, WorkbenchRevision, WorkerId,
    };
    let (mut ui, mailbox, clock, _) = tests::context_selection::ready(true);
    let location = ui.state.as_ref().unwrap().cache().navigation()[1]
        .location
        .clone();
    let mut source = ui.view().action_menu_source().unwrap();
    source.revision = WorkbenchRevision::new(source.revision.value() + 1);
    assert!(!ui.open_action_menu(
        MenuTarget::Drawn {
            source,
            target: Box::new(MenuTarget::Workspace(location.clone()))
        },
        None,
        false
    ));
    assert!(ui.open_action_menu(MenuTarget::Workspace(location), None, false));
    let token = arm(&mut ui);
    let row = action_row(&ui, MenuAction::RenameWorkspace);
    activate_row(&mut ui, token, row);
    let &Stage::Context { generation, .. } = ui.menus.intent().unwrap().stage() else {
        panic!("owned generation");
    };
    let mut replacement = tests::context_selection::snapshot(true);
    replacement.revision = WorkbenchRevision::new(2);
    let directory_id = WorkbenchDirectoryId::parse("00000000000000000000000000000901").unwrap();
    replacement.navigation.directories.push(Directory {
        directory_id: directory_id.clone(),
        worker_id: WorkerId::parse("00000000000000000000000000000902").unwrap(),
        path: WorkbenchDirectoryPath::parse("C:/other-project").unwrap(),
    });
    replacement.navigation.workspaces[1].primary_directory_id = Some(directory_id);
    let baseline = Origin::capture(ui.state.as_ref());
    let lease = mailbox.try_recv().unwrap().unwrap();
    let QueuedCommand::Refresh(marker) = lease.record() else {
        panic!("refresh");
    };
    mailbox.mark_refresh_in_flight(generation).unwrap();
    let wait = mailbox
        .publish_refresh_result_before(
            generation,
            orchbee_workbench_protocol::WorkbenchSnapshotOutcome::Available {
                snapshot: replacement,
            },
            marker.deadline(),
            &clock,
        )
        .unwrap();
    ui.tick();
    let Some(UiReply::Refresh(reply)) = mailbox.take_deposited_reply(&wait).unwrap() else {
        panic!("refresh reply");
    };
    assert!(
        !matches!(reply.decision(), RefreshDecision::Cleanup(_)),
        "replacement refused before old viewer cleanup"
    );
    assert_eq!(Origin::capture(ui.state.as_ref()), baseline);
    mailbox.finish_lease(lease).unwrap();
    assert!(!ui.action_menu_active());
    assert_eq!(ui.take_action_menu_action(false), None);
}

#[test]
fn carrier_retirement_revokes_members_and_stateless_global_menu_observes_geometry() {
    for row in [3, 4, 5] {
        let (mut ui, _, _, _) = tests::move_membership::ready_ui();
        let pane = ui.state.as_ref().unwrap().focus_pane_ref().unwrap();
        assert!(ui.open_action_menu(MenuTarget::Pane(pane.clone()), None, false));
        let token = arm(&mut ui);
        activate_row(&mut ui, token, row);
        ui.retire(CarrierFailure::Closed);
        assert!(!ui.action_menu_active());
        assert_eq!(ui.take_action_menu_action(false), None);
        assert!(!ui.open_action_menu(MenuTarget::Pane(pane), None, false));
        assert!(ui.open_action_menu(MenuTarget::Global, None, false));
        assert_eq!(
            ui.view().action_menu().unwrap().actions,
            &[
                MenuAction::Detach,
                MenuAction::Machines,
                MenuAction::Help,
                MenuAction::Sidebar,
                MenuAction::ReleasePointer,
            ]
        );
    }
    let (mut ui, _, _, _) = tests::move_membership::ready_ui();
    ui.state = None;
    ui.menus.forget_geometry();
    assert!(!ui.open_action_menu(MenuTarget::Global, None, false));
    ui.observe_geometry(Geometry::new(80, 24, 8, 16).unwrap());
    assert!(ui.open_action_menu(MenuTarget::Global, None, false));
    arm(&mut ui);
    ui.observe_geometry(Geometry::new(120, 40, 8, 16).unwrap());
    assert!(ui.action_menu_active());
    assert!(!ui.action_menu_input_context().unwrap().activation_ready());
    assert_eq!(ui.take_action_menu_action(false), None);
}

#[test]
fn equal_numeric_navigation_generations_cannot_cross_context_and_tab_domains() {
    for context in [false, true] {
        let (mut ui, _, _, _) = tests::context_selection::ready(true);
        let target = if context {
            MenuTarget::Workspace(
                ui.state.as_ref().unwrap().cache().navigation()[1]
                    .location
                    .clone(),
            )
        } else {
            MenuTarget::Tab(
                ui.state.as_ref().unwrap().cache().tabs()[1]
                    .tab_id()
                    .clone(),
            )
        };
        assert!(ui.open_action_menu(target, None, false));
        let action = if context {
            MenuAction::RenameWorkspace
        } else {
            MenuAction::RenameTab
        };
        let row = ui
            .view()
            .action_menu()
            .unwrap()
            .actions
            .iter()
            .position(|candidate| *candidate == action)
            .unwrap();
        let token = arm(&mut ui);
        activate_row(&mut ui, token, row);
        let generation = match ui.menus.intent().unwrap().stage() {
            Stage::Context { generation, .. } | Stage::Tab { generation, .. } => *generation,
            _ => panic!("navigation pending"),
        };
        let foreign = if context {
            NavigationKey::Tab(generation)
        } else {
            NavigationKey::Context(generation)
        };
        ui.menu_navigation_prepared(foreign, true);
        ui.menu_navigation_installed(foreign);
        assert!(matches!(
            ui.menus.intent().unwrap().stage(),
            Stage::Context {
                prepared: false,
                ..
            } | Stage::Tab {
                prepared: false,
                ..
            }
        ));
        assert_eq!(ui.take_action_menu_action(false), None);
        let own = if context {
            NavigationKey::Context(generation)
        } else {
            NavigationKey::Tab(generation)
        };
        ui.menu_navigation_prepared(own, true);
        assert!(matches!(
            ui.menus.intent().unwrap().stage(),
            Stage::Context { prepared: true, .. } | Stage::Tab { prepared: true, .. }
        ));
        ui.menu_navigation_installed(foreign);
        assert!(matches!(
            ui.menus.intent().unwrap().stage(),
            Stage::Context { prepared: true, .. } | Stage::Tab { prepared: true, .. }
        ));
        assert_eq!(ui.take_action_menu_action(false), None);
    }
}

/// Reads the Panel rows as a user sees them, so the rule is checked through its label.
fn copy_selection_row(ui: &WorkbenchClientUi) -> usize {
    let view = ui.view();
    let menu = view.action_menu().unwrap();
    let labels = menu
        .actions
        .iter()
        .map(|action| Locale::EnUs.menu_action(action))
        .collect::<Vec<_>>();
    labels
        .iter()
        .position(|label| *label == "Copy selection")
        .unwrap_or_else(|| panic!("Panel menu rows {labels:?} do not offer Copy selection"))
}

#[test]
fn panel_menu_copy_selection_is_offered_only_for_a_settled_host_selection_and_exports_it() {
    use orchbee_terminal_protocol::{SelectionChanged, SelectionChangedState, SelectionPoint};

    let (mut ui, mailbox, _, _) = tests::move_membership::ready_ui();
    let pane = super::super::test_support::install_projection(&mut ui, Some((9, false, 24)), 0);
    let attachment = *ui.projections[0].value.attachment();

    // Without a Host selection the row stays visible with its reason.
    assert!(ui.open_action_menu(MenuTarget::Pane(pane.clone()), None, false));
    let row = copy_selection_row(&ui);
    assert!(ui.view().action_menu().unwrap().unavailable[row].is_some());
    ui.cancel_action_menu();

    let driver = ui.projections[0].selection.get_or_insert_with(|| {
        crate::workbench_client::terminal_selection::Driver::new(attachment)
    });
    driver
        .changed(SelectionChanged {
            attachment_id: attachment,
            state: SelectionChangedState::Active {
                start: SelectionPoint { column: 1, row: 9 },
                end: SelectionPoint { column: 3, row: 9 },
                rectangle: false,
                preview: None,
            },
        })
        .unwrap();

    // Opening the menu over the settled selection leaves that selection in place.
    assert!(ui.open_action_menu(MenuTarget::Pane(pane.clone()), None, false));
    let row = copy_selection_row(&ui);
    assert_eq!(ui.view().action_menu().unwrap().unavailable[row], None);
    let token = arm(&mut ui);
    activate_row(&mut ui, token, row);
    let action = ui
        .take_action_menu_action(false)
        .expect("confirmed Copy selection");
    let _ = ui.apply_client_action(action);

    // Copy starts one export of that same selection through the ordinary selection command path.
    let selection = ui.projections[0].selection.as_ref().unwrap();
    assert!(selection.host_selection_active());
    assert!(selection.export_pending());
    ui.admit_selections(&mailbox);
    let lease = mailbox.try_recv().unwrap().unwrap();
    let QueuedCommand::Selection(tagged) = lease.record() else {
        panic!("selection command");
    };
    let command = *tagged.value();
    mailbox.finish_lease(lease).unwrap();
    let viewer = orchbee_client_server_protocol::ViewerId::parse(&attachment.to_string()).unwrap();
    assert!(matches!(
        command.viewer_action(viewer),
        orchbee_client_server_protocol::TerminalViewerAction::SelectionExport { .. }
    ));

    // While that export is in flight, the row is not offered again.
    assert!(ui.open_action_menu(MenuTarget::Pane(pane), None, false));
    let row = copy_selection_row(&ui);
    assert!(ui.view().action_menu().unwrap().unavailable[row].is_some());
}
