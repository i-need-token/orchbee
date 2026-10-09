use super::super::workspace_setup_tests::{place, ready_snapshot};
use super::super::*;
use super::*;
use crate::action_menu::MenuDraw;
use crate::local_client::workbench::workspace_setup::WorkbenchSetupCommit;
use crate::terminal_session::TerminalSessionView;
use crate::terminal_session::mailbox::CommandLease;
use orchbee_workbench_protocol::{
    Snapshot, TabGroup, WorkbenchAction, WorkbenchRevision, configure_tabs,
};

fn grouped() -> Snapshot {
    let mut snapshot = super::super::membership_tests::snapshot(3, 1);
    let group = TabGroup {
        group_id: WorkbenchTabGroupId::parse(&format!("{:032x}", 1900)).unwrap(),
        label: "源组".into(),
        color: TabGroupColor::Green,
    };
    for tab in &mut snapshot.terminal_contexts[0].tabs[1..] {
        tab.group_id = Some(group.group_id.clone());
    }
    snapshot.terminal_contexts[0].groups.push(group);
    snapshot
}

fn choose(ui: &mut NativeUi, action: MenuAction) {
    let view = ui.view();
    let menu = view.action_menu().unwrap();
    let row = menu
        .actions
        .iter()
        .position(|candidate| *candidate == action)
        .unwrap();
    let token = menu.token;
    // Unpainted choices cannot produce an intent or mutate the active member.
    ui.click_action_menu(token, row);
    assert_eq!(ui.take_action_menu_action(false), None);
    for _ in 0..row {
        ui.capture_action_menu_key(crate::client_view::KeyEvent::press(
            crate::client_view::KeyCode::Down,
            crate::client_view::Modifiers::NONE,
        ));
    }
    let token = ui.view().action_menu().unwrap().token;
    ui.after_action_menu_draw(Some(ModalPresentation::Presented(MenuDraw {
        token,
        rows: 80,
    })));
    ui.capture_action_menu_key(crate::client_view::KeyEvent::press(
        crate::client_view::KeyCode::Enter,
        crate::client_view::Modifiers::NONE,
    ));
    let picked = ui
        .take_action_menu_action(false)
        .expect("painted menu handoff");
    ui.apply_client_action(picked);
}

fn accept_configuration(
    ui: &mut NativeUi,
    mailbox: &CommandMailbox,
    clock: &LocalOperationClock,
    snapshot: &mut Snapshot,
    created: Option<WorkbenchTabGroupId>,
) {
    let old = ui.state.as_ref().unwrap().desired().cloned();
    let baseline = ui
        .state
        .as_ref()
        .unwrap()
        .cache()
        .canonical_snapshot()
        .clone();
    let lease: CommandLease = mailbox.try_recv().unwrap().expect("configuration command");
    let QueuedCommand::WorkspaceSetup(marker) = lease.record() else {
        panic!("setup lane")
    };
    let WorkbenchAction::TabsConfigure { operation, .. } = &marker.0.command.action else {
        panic!("typed metadata operation")
    };
    assert_eq!(
        mailbox.mark_setup_in_flight(&marker.0).unwrap(),
        MutationResult::Applied
    );
    let candidate =
        configure_tabs(&snapshot.terminal_contexts[0], operation, created.as_ref()).unwrap();
    let unchanged = candidate.is_none();
    if let Some(context) = candidate {
        snapshot.terminal_contexts[0] = context;
        snapshot.revision = WorkbenchRevision::new(snapshot.revision.value() + 1);
    }
    assert_eq!(
        ui.state.as_ref().unwrap().cache().canonical_snapshot(),
        &baseline,
        "queueing is not a canonical write"
    );
    let wait = place(
        mailbox,
        clock,
        &lease,
        WorkbenchSetupCommit::TabsConfigured {
            revision: snapshot.revision,
            group_id: created,
            unchanged,
        },
        snapshot.clone(),
    );
    ui.tick();
    let Some(UiReply::Refresh(settlement)) = mailbox.take_deposited_reply(&wait).unwrap() else {
        panic!("refresh settlement")
    };
    assert_eq!(settlement.decision(), &RefreshDecision::Continue);
    mailbox.finish_lease(lease).unwrap();
    assert_eq!(
        ui.state.as_ref().unwrap().desired(),
        old.as_ref(),
        "same viewer, incarnation and binding"
    );
    assert_eq!(ui.view().phase(), NativeDisplayPhase::Ready);
    assert_eq!(
        ui.state.as_ref().unwrap().cache().canonical_snapshot(),
        snapshot
    );
}

#[test]
fn tab_group_menus_submit_canonical_operations_and_preserve_selected_terminal() {
    let (mut ui, mailbox, clock, mut snapshot) = ready_snapshot(grouped());
    let original = snapshot.clone();
    let tab = snapshot.terminal_contexts[0].tabs[0].tab_id.clone();
    assert!(ui.open_action_menu(MenuTarget::Tab(tab.clone()), None, false));
    choose(&mut ui, MenuAction::ManageGroups);
    choose(&mut ui, MenuAction::CreateGroup);
    let token = ui.view().setup_draft().unwrap().token();
    ui.edit_setup_draft(token, NativeSetupEdit::Clear);
    ui.edit_setup_draft(token, NativeSetupEdit::Paste("构建 🦀"));
    ui.submit_setup_draft(token);
    assert!(
        mailbox.try_recv().unwrap().is_none(),
        "draft also requires a complete paint"
    );
    ui.after_setup_draw(Some(ModalPresentation::Presented(token)));
    ui.submit_setup_draft(token);
    accept_configuration(
        &mut ui,
        &mailbox,
        &clock,
        &mut snapshot,
        Some(WorkbenchTabGroupId::parse(&format!("{:032x}", 1901)).unwrap()),
    );
    for action in [
        MenuAction::GroupColor(TabGroupColor::Orange),
        MenuAction::MoveGroupNext,
        MenuAction::MoveGroupPrevious,
        MenuAction::JoinGroup(0),
        MenuAction::RemoveFromGroup,
        MenuAction::PinTab,
        MenuAction::UnpinTab,
        MenuAction::JoinGroup(0),
        MenuAction::Ungroup,
    ] {
        ui.apply_client_action(ClientAction::TabMenu {
            action,
            tab_id: None,
        });
        accept_configuration(&mut ui, &mailbox, &clock, &mut snapshot, None);
        for tab in &snapshot.terminal_contexts[0].tabs {
            let old = original.terminal_contexts[0]
                .tabs
                .iter()
                .find(|old| old.tab_id == tab.tab_id)
                .unwrap();
            assert_eq!(tab.root, old.root);
        }
    }
    assert!(snapshot.terminal_contexts[0].groups.is_empty());
    assert_eq!(
        ui.state
            .as_ref()
            .unwrap()
            .desired()
            .unwrap()
            .anchor()
            .tab_id(),
        &tab
    );
}

#[test]
fn collapse_is_client_local_does_not_select_or_enqueue_and_survives_shared_changes() {
    let (mut ui, mailbox, clock, mut snapshot) = ready_snapshot(grouped());
    let group = snapshot.terminal_contexts[0].groups[0].group_id.clone();
    let selected = ui.state.as_ref().unwrap().desired().cloned();
    ui.apply_client_action(ClientAction::ToggleTabGroup(group.clone()));
    assert!(ui.view().group_collapsed(&group));
    assert_eq!(ui.state.as_ref().unwrap().desired(), selected.as_ref());
    assert!(mailbox.try_recv().unwrap().is_none());
    ui.apply_client_action(ClientAction::TabMenu {
        action: MenuAction::PinTab,
        tab_id: None,
    });
    accept_configuration(&mut ui, &mailbox, &clock, &mut snapshot, None);
    assert!(ui.view().group_collapsed(&group));
    assert_eq!(
        ui.state.as_ref().unwrap().cache().groups(),
        snapshot.terminal_contexts[0].groups
    );
    let (other, _, _, _) = ready_snapshot(snapshot);
    assert!(
        !other.view().group_collapsed(&group),
        "another Client owns independent disclosure"
    );
}

#[test]
fn background_tab_group_menu_does_not_select_its_target() {
    let (mut ui, mailbox, clock, mut snapshot) = ready_snapshot(grouped());
    let original = ui.state.as_ref().unwrap().desired().cloned();
    let target = snapshot.terminal_contexts[0].tabs[1].tab_id.clone();
    assert!(ui.open_action_menu(MenuTarget::Tab(target.clone()), None, false));
    choose(&mut ui, MenuAction::PinTab);
    assert_eq!(ui.state.as_ref().unwrap().desired(), original.as_ref());
    accept_configuration(&mut ui, &mailbox, &clock, &mut snapshot, None);
    assert_eq!(snapshot.terminal_contexts[0].tabs[0].tab_id, target);
    assert!(snapshot.terminal_contexts[0].tabs[0].pinned);
    assert_eq!(ui.state.as_ref().unwrap().desired(), original.as_ref());
}

#[test]
fn tab_groups_composed_screen_keeps_headers_members_and_localized_menu_hits() {
    use crate::{
        localization::Locale,
        tui_pointer::{HitMapBuilder, HitTarget},
        tui_theme::{ColorLevel, GlyphSet, Theme},
    };
    use ratatui::{Terminal, backend::TestBackend};
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for (width, height) in [(120, 40), (80, 24), (80, 20), (40, 10), (20, 5)] {
            for (theme_index, theme) in [
                Theme::terminal_native(),
                Theme::one_light(),
                Theme::one_dark(),
                Theme::one_dark().quantize(ColorLevel::Ansi256),
                Theme::one_light().quantize(ColorLevel::Ansi16),
                Theme::terminal_native()
                    .quantize(ColorLevel::Monochrome)
                    .with_glyphs(GlyphSet::ASCII),
            ]
            .into_iter()
            .enumerate()
            {
                let mut snapshot = grouped();
                let group = snapshot.terminal_contexts[0].groups[0].group_id.clone();
                snapshot.terminal_contexts[0].tabs[0].pinned = true;
                let (mut ui, _, _, _) = ready_snapshot(snapshot);
                ui.observe_geometry(Geometry::new(width, height, 8, 16).unwrap());
                let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                for collapsed in [false, true] {
                    if collapsed {
                        ui.apply_client_action(ClientAction::ToggleTabGroup(group.clone()));
                    }
                    let mut hits = HitMapBuilder::new();
                    terminal
                        .draw(|frame| {
                            crate::tui_render::render_terminal_shell(
                                frame,
                                &crate::client_view::ClientView::new(width),
                                locale,
                                Some(TerminalSessionView::Native(ui.view())),
                                &theme,
                                &mut hits,
                            );
                        })
                        .unwrap();
                    let hits = hits.finish();
                    let header = (0..height).flat_map(|y| (0..width).map(move |x| (x,y))).find(|(x,y)| matches!(hits.hit_target_at(*x,*y), Some(HitTarget::TabGroup { group: id, .. }) if id == &group));
                    if width >= 80 {
                        assert!(
                            header.is_some(),
                            "whole screen must expose the group header"
                        );
                        let labels = rendered_text(terminal.backend().buffer());
                        assert!(labels.contains("源组"));
                        assert!(labels.contains("(2)"));
                        assert!(labels.contains(locale.shell().pinned_tab));
                    }
                    if collapsed {
                        let member_hit = (0..height).any(|y| {
                            (0..width).any(|x| {
                                matches!(hits.hit_target_at(x, y), Some(HitTarget::Tab(id)) if id != ui.state.as_ref().unwrap().cache().tabs()[0].tab_id())
                            })
                        });
                        assert!(!member_hit);
                    }
                    if theme_index == 0 && [(120, 40), (80, 20)].contains(&(width, height)) {
                        capture_screen(
                            terminal.backend().buffer(),
                            &format!(
                                "{locale:?}-{width}x{height}-{}",
                                if collapsed { "collapsed" } else { "expanded" }
                            ),
                        );
                    }
                }
                if theme_index == 0 && [(120, 40), (80, 20)].contains(&(width, height)) {
                    let tab_id = ui.state.as_ref().unwrap().cache().tabs()[1]
                        .tab_id()
                        .clone();
                    assert!(ui.open_action_menu(
                        MenuTarget::TabGroups {
                            tab_id,
                            colors: false
                        },
                        None,
                        false
                    ));
                    let mut hits = HitMapBuilder::new();
                    terminal
                        .draw(|frame| {
                            crate::tui_render::render_terminal_shell(
                                frame,
                                &crate::client_view::ClientView::new(width),
                                locale,
                                Some(TerminalSessionView::Native(ui.view())),
                                &theme,
                                &mut hits,
                            );
                        })
                        .unwrap();
                    capture_screen(
                        terminal.backend().buffer(),
                        &format!("{locale:?}-{width}x{height}-menu"),
                    );
                    let hits = hits.finish();
                    assert!((0..height).any(|y| (0..width).any(|x| matches!(
                        hits.hit_target_at(x, y),
                        Some(HitTarget::ActionMenuRow { .. })
                    ))));
                }
            }
        }
    }
}

fn rendered_text(buffer: &ratatui::buffer::Buffer) -> String {
    let mut text = String::new();
    for y in buffer.area.y..buffer.area.bottom() {
        let mut x = buffer.area.x;
        while x < buffer.area.right() {
            let symbol = buffer[(x, y)].symbol();
            text.push_str(symbol);
            x += u16::try_from(crate::terminal_text::display_width(symbol).max(1)).unwrap();
        }
        text.push('\n');
    }
    text
}

// Optional whole-screen evidence uses the actual composed TestBackend cells, not a separate mock.
// The invoking task owns the supplied capture directory and its release after review.
fn capture_screen(buffer: &ratatui::buffer::Buffer, name: &str) {
    let Some(directory) = std::env::var_os("ORCHBEE_TUI_CAPTURE_DIR") else {
        return;
    };
    let path = std::path::Path::new(&directory).join(format!("{name}.json"));
    let cells: Vec<_> = buffer.content().iter().map(|cell| serde_json::json!({"symbol":cell.symbol(),"fg":format!("{:?}",cell.fg),"bg":format!("{:?}",cell.bg),"modifier":format!("{:?}",cell.modifier)})).collect();
    std::fs::write(path, serde_json::to_vec(&serde_json::json!({"width":buffer.area.width,"height":buffer.area.height,"cells":cells})).unwrap()).unwrap();
}
