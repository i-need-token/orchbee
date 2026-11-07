//! Reading continuity through fresh projections and the existing Client view handoff.

use super::super::{
    CommandMailbox, ControllerMode, Geometry, ViewportOutcome, ViewportStep, WorkbenchClientUi,
};
use super::*;
use crate::terminal_viewport::ViewportWindow;
use crate::workbench_client::{
    mailbox::QueuedCommand,
    ui::{
        RetainedFrameBank, WorkbenchClientNotice,
        tests::{
            move_membership::ready_ui,
            viewport::{frame, install_projection, settle_viewing},
        },
    },
};
use orchbee_client_server_protocol::ViewportRowOrigin;
use orchbee_terminal_protocol::{
    HistoryAnchorStatus, HistoryChanged, RowWindowDetail, TerminalRowWindowRoute,
};
use std::{num::NonZeroU64, sync::Arc};

fn reading_ui() -> (
    WorkbenchClientUi,
    std::sync::Arc<super::super::CommandMailbox>,
    PaneRef,
) {
    let (ui, mailbox, _, _) = ready_ui();
    read_at_bookmark(ui, mailbox)
}

fn read_at_bookmark(
    mut ui: WorkbenchClientUi,
    mailbox: std::sync::Arc<super::super::CommandMailbox>,
) -> (
    WorkbenchClientUi,
    std::sync::Arc<super::super::CommandMailbox>,
    PaneRef,
) {
    let key = install_projection(&mut ui, Some((376, false, 24)), 0);
    ui.enter_history();
    let lease = mailbox.try_recv().unwrap().unwrap();
    assert!(matches!(lease.record(), QueuedCommand::Viewport(_)));
    mailbox.finish_lease(lease).unwrap();
    settle_viewing(&mut ui, &mailbox, &key, 1);
    ui.remember_history_positions();
    (ui, mailbox, key)
}

fn returning_ui(
    bank: RetainedFrameBank,
) -> (
    WorkbenchClientUi,
    std::sync::Arc<super::super::CommandMailbox>,
    PaneRef,
) {
    let (mut ui, mailbox, _, _) = ready_ui();
    let key = install_projection(&mut ui, Some((400, false, 24)), 0);
    ui.inherit_retained_frame_bank(bank);
    (ui, mailbox, key)
}

fn clipped_projection(ui: &mut WorkbenchClientUi, physical_rows: u16) -> PaneRef {
    let key = install_projection(ui, None, 0);
    let projection = ui.projections.last_mut().unwrap();
    let mut frame = frame(
        &projection.value,
        400 - u64::from(physical_rows),
        false,
        physical_rows,
    );
    frame.geometry.rows = physical_rows;
    projection.value.install_frame_for_test(frame);
    let crop = ui.view().pane_live_view(key.pane().panel()).unwrap();
    assert!(crop.clipped);
    assert_eq!(crop.rows, 20);
    assert_eq!(
        ui.view()
            .painted_content_source(key.pane().panel())
            .unwrap()
            .rows
            .row_count,
        20
    );
    key
}

fn take_viewport_step(mailbox: &CommandMailbox, key: &PaneRef) -> ViewportStep {
    let lease = mailbox.try_recv().unwrap().expect("viewport command");
    let QueuedCommand::Viewport(tagged) = lease.record() else {
        panic!("only viewport commands expected, got {:?}", lease.record());
    };
    assert_eq!(tagged.key(), key);
    let step = *tagged.value();
    mailbox.finish_lease(lease).unwrap();
    assert!(mailbox.try_recv().unwrap().is_none());
    step
}

/// Serve the physical 36-row read at row 100 and commit only through the matching ACK.
fn complete_clipped_history_entry(
    ui: &mut WorkbenchClientUi,
    mailbox: &Arc<CommandMailbox>,
    key: &PaneRef,
) {
    assert_eq!(take_viewport_step(mailbox, key), ViewportStep::Enter);
    assert!(ui.apply_viewport_outcome(
        mailbox,
        key.clone(),
        ViewportOutcome::Enter { applied: true }
    ));
    assert!(
        mailbox.try_recv().unwrap().is_none(),
        "facts must precede the read"
    );
    let frame = ui.projections[0].value.frame().unwrap().clone();
    let effect = ui.projections[0]
        .viewport
        .as_mut()
        .unwrap()
        .facts(HistoryChanged {
            attachment_id: frame.route.attachment_id,
            sequence: NonZeroU64::MIN,
            revision: frame.route.revision,
            generation: frame.route.generation,
            anchor_status: HistoryAnchorStatus::Valid,
            total_rows: 400,
        });
    ui.settle_viewport_effect(mailbox, key, effect);
    let ViewportStep::Read {
        origin,
        row_count,
        request_sequence,
        generation,
        revision,
        ..
    } = take_viewport_step(mailbox, key)
    else {
        panic!("entry read");
    };
    assert_eq!(row_count, 36);
    assert_eq!(origin, ViewportRowOrigin::Tail { rows_before: 264 });
    let mut rows = frame.rows;
    for row in &mut rows {
        row.cells[0].text = format!("saved {}", 100 + u64::from(row.index)).into();
    }
    assert!(ui.apply_viewport_outcome(
        mailbox,
        key.clone(),
        ViewportOutcome::Window {
            request_sequence,
            window: Box::new(ViewportWindow {
                route: TerminalRowWindowRoute {
                    attachment_id: frame.route.attachment_id,
                    host_epoch: frame.route.host_epoch,
                    sequence: request_sequence,
                    revision,
                    generation,
                    total_rows: 400,
                    start_row: 100,
                    columns: frame.geometry.columns,
                    row_count,
                    detail: RowWindowDetail::Render,
                },
                foreground: frame.foreground,
                background: frame.background,
                graphics_present: false,
                graphics: None,
                rows,
            }),
        }
    ));
    assert!(
        ui.projections[0]
            .viewport
            .as_ref()
            .unwrap()
            .committed_window()
            .is_none()
    );
    assert_eq!(
        take_viewport_step(mailbox, key),
        ViewportStep::Ack {
            request_sequence,
            generation,
            revision
        }
    );
    assert!(ui.apply_viewport_outcome(
        mailbox,
        key.clone(),
        ViewportOutcome::Ack {
            request_sequence,
            accepted: true,
        }
    ));
    assert!(mailbox.try_recv().unwrap().is_none());
    let viewport = ui.projections[0].viewport.as_ref().unwrap();
    assert_eq!(viewport.geometry(), (80, 36));
    assert_eq!(viewport.committed_window().unwrap().route.start_row, 100);
    assert!(!viewport.resuming());
}

#[test]
fn clipped_read_only_tab_roundtrip_restores_only_with_unchanged_physical_height() {
    for returned_physical_rows in [36, 35] {
        let (mut ui, mailbox, _, _) = ready_ui();
        ui.observe_geometry(Geometry::new(80, 22, 8, 16).unwrap());
        assert!(
            mailbox.try_recv().unwrap().is_none(),
            "read-only geometry never resizes the host"
        );
        assert_eq!(
            ui.state.as_ref().unwrap().controller_mode(),
            Some(&ControllerMode::ReadOnly)
        );
        let key = clipped_projection(&mut ui, 36);
        ui.enter_pane_history_rows(&key, Some(264));
        complete_clipped_history_entry(&mut ui, &mailbox, &key);
        ui.remember_history_positions();
        let saved = ui.projections[0]
            .viewport
            .as_ref()
            .unwrap()
            .reading_position()
            .unwrap();
        assert_eq!(saved.viewport_rows, 36);
        assert_eq!(saved.witness.len(), 8);

        // A tab roundtrip disposes the projection but retains the canonical pane and local size.
        let focus = ui.state.as_ref().unwrap().focus_pane_ref();
        ui.clear_projections();
        assert_eq!(clipped_projection(&mut ui, returned_physical_rows), key);
        assert!(ui.drive_history_positions());
        if returned_physical_rows != 36 {
            // Both frames crop to 20 local rows, so this must check the physical height.
            assert_eq!(
                ui.view().notice(),
                Some(WorkbenchClientNotice::HistoryPositionLost)
            );
            assert!(ui.projections[0].viewport.is_none());
            assert!(
                mailbox.try_recv().unwrap().is_none(),
                "no Enter, resize, or input on refusal"
            );
        } else {
            assert!(ui.projections[0].viewport.as_ref().unwrap().resuming());
            complete_clipped_history_entry(&mut ui, &mailbox, &key);
            let restored = ui.projections[0]
                .viewport
                .as_ref()
                .unwrap()
                .reading_position()
                .unwrap();
            assert_eq!(restored.start_row, saved.start_row);
            assert_eq!(restored.viewport_rows, saved.viewport_rows);
            assert_eq!(restored.generation, saved.generation);
            assert_eq!(restored.columns, saved.columns);
            assert_eq!(restored.witness, saved.witness);
            assert_ne!(
                ui.view().notice(),
                Some(WorkbenchClientNotice::HistoryPositionLost)
            );
        }
        assert_eq!(ui.state.as_ref().unwrap().focus_pane_ref(), focus);
        assert_eq!(
            ui.state.as_ref().unwrap().controller_mode(),
            Some(&ControllerMode::ReadOnly)
        );
        assert!(!ui.drive_history_positions());
    }
}

#[test]
fn tab_projection_replacement_restores_only_after_enter_facts_and_content_validation() {
    let (mut ui, mailbox, old_key) = reading_ui();
    let saved = ui.projections[0]
        .viewport
        .as_ref()
        .unwrap()
        .reading_position()
        .unwrap();
    assert_eq!(saved.start_row, 100);
    ui.clear_projections();
    let key = install_projection(&mut ui, Some((400, false, 24)), 0);
    let focus = ui.state.as_ref().unwrap().focus_pane_ref();
    assert_eq!(key, old_key);
    assert!(ui.drive_history_positions());
    let lease = mailbox.try_recv().unwrap().unwrap();
    assert!(
        matches!(lease.record(), QueuedCommand::Viewport(tagged) if tagged.key() == &key && tagged.value() == &super::super::ViewportStep::Enter)
    );
    mailbox.finish_lease(lease).unwrap();
    assert!(
        mailbox.try_recv().unwrap().is_none(),
        "no read before current history facts"
    );
    assert!(
        ui.projections[0]
            .viewport
            .as_ref()
            .unwrap()
            .committed_window()
            .is_none()
    );
    assert_eq!(ui.state.as_ref().unwrap().focus_pane_ref(), focus);
    assert!(
        !ui.drive_history_positions(),
        "a pending restoration never enters twice"
    );
}

#[test]
fn targeted_context_handoff_carries_the_reading_choice_but_explicit_live_wins() {
    let (mut old, _, key) = reading_ui();
    let bank = old.retained_frame_bank();
    let (mut fresh, mailbox, _) = returning_ui(bank);
    assert!(fresh.drive_history_positions());
    let lease = mailbox.try_recv().unwrap().unwrap();
    assert!(matches!(lease.record(), QueuedCommand::Viewport(_)));
    mailbox.finish_lease(lease).unwrap();

    old.history_positions.bank.live(&key);
    let newer = old.retained_frame_bank();
    let (mut fresh, mailbox, _) = returning_ui(newer);
    assert!(!fresh.drive_history_positions());
    assert!(mailbox.try_recv().unwrap().is_none());
    assert!(fresh.projections[0].viewport.is_none());
}

#[test]
fn restart_or_geometry_change_refuses_the_old_position_without_enter_or_input() {
    for changed in ["epoch", "terminal", "generation", "columns", "rows"] {
        let (old, _, _) = reading_ui();
        let mut bank = old.retained_frame_bank();
        let source = &mut bank.history.entries[0].source;
        match changed {
            "epoch" => {
                source.rows.host_epoch =
                    orchbee_terminal_protocol::HostEpoch::from_bytes([55; 16]).unwrap()
            }
            "terminal" => {
                source.host_terminal =
                    orchbee_client_server_protocol::HostTerminalId::parse(&format!("{:032x}", 55))
                        .unwrap()
            }
            "generation" | "columns" | "rows" => {
                let Choice::History(position) = &mut bank.history.entries[0].choice else {
                    panic!()
                };
                match changed {
                    "generation" => position.generation = std::num::NonZeroU64::new(55).unwrap(),
                    "columns" => position.columns = 79,
                    "rows" => position.viewport_rows = 23,
                    _ => unreachable!(),
                }
            }
            _ => unreachable!(),
        }
        let (mut ui, mailbox, _) = returning_ui(bank);
        assert!(ui.drive_history_positions(), "{changed}");
        assert_eq!(
            ui.view().notice(),
            Some(WorkbenchClientNotice::HistoryPositionLost)
        );
        assert!(mailbox.try_recv().unwrap().is_none());
        assert!(ui.projections[0].viewport.is_none());
        assert!(!ui.drive_history_positions());
    }
}

#[test]
fn explicit_history_exit_is_retained_as_live_before_the_core_finishes_recovery() {
    let (mut ui, _, key) = reading_ui();
    assert!(ui.history_key(&crate::client_view::KeyEvent::press(
        crate::client_view::KeyCode::Esc,
        crate::client_view::Modifiers::NONE
    )));
    ui.remember_history_positions();
    assert!(matches!(
        ui.history_positions.bank.entries[0].choice,
        Choice::Live
    ));
    let (mut returned, mailbox, _) = returning_ui(ui.retained_frame_bank());
    assert!(!returned.drive_history_positions());
    assert!(mailbox.try_recv().unwrap().is_none());
    assert_eq!(ui.state.as_ref().unwrap().focus_pane_ref(), Some(key));
}

#[test]
fn canonical_removal_or_rebinding_prunes_the_old_reading_choice() {
    let (mut ui, _, _) = reading_ui();
    ui.history_positions.bank.entries[0].source.member.terminal =
        orchbee_workbench_protocol::TerminalId::parse(&format!("{:032x}", 55)).unwrap();
    ui.history_positions.bank.prune(ui.state.as_ref().unwrap());
    assert!(ui.history_positions.bank.entries.is_empty());
}

#[test]
fn each_terminal_retains_its_own_choice_and_newer_live_overrides_an_older_bookmark() {
    use crate::workbench_state::{AttachmentKey, PaneKey};
    let (mut ui, _, _) = reading_ui();
    let first = ui.history_positions.bank.entries[0].clone();
    let tab = &ui.state.as_ref().unwrap().cache().tabs()[1];
    let panel = tab.panel(0).unwrap();
    let presentation = panel.primary();
    let binding = presentation.binding().unwrap();
    let mut second = first.source.clone();
    second.member.tab = tab.tab_id().clone();
    second.member.panel = panel.panel_id().clone();
    second.member.presentation = presentation.presentation_id().clone();
    second.member.worker = binding.worker_id().clone();
    second.member.terminal = binding.terminal_id().clone();
    second.pane = PaneRef::new(
        PaneKey::new(
            AttachmentKey::new(2, tab.tab_id().clone()),
            panel.panel_id().clone(),
        ),
        0,
    );
    ui.history_positions
        .bank
        .set(second.clone(), first.choice.clone());
    let older = ui.history_positions.bank.clone();
    ui.history_positions
        .bank
        .set(first.source.clone(), Choice::Live);
    ui.history_positions
        .bank
        .inherit_older(older, ui.state.as_ref().unwrap());
    assert_eq!(ui.history_positions.bank.entries.len(), 2);
    assert!(matches!(
        ui.history_positions
            .bank
            .get(&first.source.member)
            .unwrap()
            .choice,
        Choice::Live
    ));
    assert!(matches!(
        ui.history_positions
            .bank
            .get(&second.member)
            .unwrap()
            .choice,
        Choice::History(_)
    ));
}

#[test]
fn evicted_witness_retains_a_visible_fallback_instead_of_reusing_coordinates() {
    let (mut old, _, _) = reading_ui();
    let mut saved = old.history_positions.bank.entries[0].clone();
    let Choice::History(position) = &mut saved.choice else {
        panic!()
    };
    position.witness[0].cells[0].text = "x".repeat(MAXIMUM_WITNESS_BYTES + 1).into();
    old.history_positions.bank.set(saved.source, saved.choice);
    assert!(matches!(
        old.history_positions.bank.entries[0].choice,
        Choice::Unavailable
    ));
    let (mut ui, mailbox, _) = returning_ui(old.retained_frame_bank());
    assert!(ui.drive_history_positions());
    assert_eq!(
        ui.view().notice(),
        Some(WorkbenchClientNotice::HistoryPositionLost)
    );
    assert!(mailbox.try_recv().unwrap().is_none());
}

fn painted(ui: &WorkbenchClientUi) -> String {
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 30)).unwrap();
    let view = crate::client_view::ClientView::new(80);
    let mut hits = crate::tui_hit_map::HitMapBuilder::new();
    terminal
        .draw(|frame| {
            crate::tui_render::render_terminal_shell(
                frame,
                &view,
                crate::localization::Locale::EnUs,
                Some(ui.view()),
                &crate::tui_theme::Theme::terminal_native(),
                &mut hits,
            );
        })
        .unwrap();
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect()
}

#[test]
fn retained_history_exposes_its_return_key_on_badge_hover_in_both_locales() {
    use crate::{
        localization::Locale,
        tui_hit_map::{HitMapBuilder, TooltipContent},
        tui_theme::Theme,
    };
    use ratatui::{Terminal, backend::TestBackend};
    let (ui, _, _) = reading_ui();
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let view = crate::client_view::ClientView::new(120);
        let mut hits = HitMapBuilder::new();
        let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
        terminal
            .draw(|frame| {
                crate::tui_render::render_terminal_shell(
                    frame,
                    &view,
                    locale,
                    Some(ui.view()),
                    &Theme::terminal_native(),
                    &mut hits,
                );
            })
            .unwrap();
        let (x, y) = (0..40)
            .find_map(|y| {
                let mut row = String::new();
                let mut x = 0;
                while x < 120 {
                    let cell = &terminal.backend().buffer()[(x, y)];
                    row.push_str(cell.symbol());
                    x += ratatui::buffer::CellWidth::cell_width(cell).max(1);
                }
                let prefix = &row[..row.find(locale.terminal_history_badge())?];
                Some((
                    u16::try_from(crate::terminal_text::display_width(prefix)).unwrap(),
                    y,
                ))
            })
            .expect("history badge is painted");
        assert!(
            matches!(hits.tooltip_content_at(x, y), Some(
            TooltipContent::Action { label, key: Some(key) }) if !label.is_empty() && key == "Esc"),
            "history must expose the return-to-live action without a persistent footer"
        );
        let key = hits.finish().tooltip_at(x, y).unwrap();
        let projection = crate::tui_hit_map::PointerProjection::default().with_tooltip(Some(
            crate::tui_hit_map::tooltip::Shown {
                key,
                pointer: (x, y),
            },
        ));
        let mut hits = HitMapBuilder::with_pointer(projection);
        terminal
            .draw(|frame| {
                crate::tui_render::render_terminal_shell(
                    frame,
                    &view,
                    locale,
                    Some(ui.view()),
                    &Theme::terminal_native(),
                    &mut hits,
                );
            })
            .unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .flat_map(|cell| cell.symbol().chars())
            .filter(|character| !character.is_whitespace())
            .collect();
        assert!(text.contains("Esc"));
        assert!(text.contains(&locale.terminal_back_to_live().replace(' ', "")));
    }
}

#[test]
fn returning_bookmark_paints_its_last_window_instead_of_the_live_tail() {
    let (mut ui, mailbox, _) = reading_ui();
    ui.clear_projections();
    let key = install_projection(&mut ui, Some((400, false, 24)), 0);
    let panel = key.pane().panel().clone();
    let stand_in = |ui: &WorkbenchClientUi| {
        ui.view()
            .pane_history_stand_in(&panel)
            .map(|window| window.route.start_row)
    };
    let history = |ui: &WorkbenchClientUi| {
        let screen = painted(ui);
        let locale = crate::localization::Locale::EnUs;
        screen.contains(locale.terminal_history_badge())
            && !screen.contains(locale.terminal_history_loading())
    };
    assert!(history(&ui), "the first live frame shows the bookmark");
    assert_eq!(stand_in(&ui), Some(100));
    assert!(ui.view().painted_link_source(&panel).is_none());
    assert!(ui.drive_history_positions());
    let lease = mailbox.try_recv().unwrap().unwrap();
    mailbox.finish_lease(lease).unwrap();
    assert_eq!(stand_in(&ui), Some(100), "entry without a window keeps it");
    assert!(history(&ui));
    settle_viewing(&mut ui, &mailbox, &key, 1);
    assert_eq!(
        stand_in(&ui),
        None,
        "the restored window replaces the image"
    );
    assert!(history(&ui));
    ui.remember_history_positions();
    assert!(ui.history_positions.restoring_image(&key).is_none());
}

/// O-8 step 7: typing right after the return breaks the navigation hold, so a frame is drawn
/// while the returning viewer is still connecting. It shows the reading screen, not the live tail.
#[test]
fn a_return_before_the_first_live_frame_paints_the_bookmark_instead_of_live_pixels() {
    let (mut ui, _mailbox, key) = reading_ui();
    ui.clear_projections();
    let panel = key.pane().panel().clone();
    let stand_in = |ui: &WorkbenchClientUi| {
        ui.view()
            .pane_history_stand_in(&panel)
            .map(|window| window.route.start_row)
    };
    let history = |ui: &WorkbenchClientUi| {
        painted(ui).contains(crate::localization::Locale::EnUs.terminal_history_badge())
    };
    assert_eq!(stand_in(&ui), Some(100), "the viewer is still connecting");
    assert!(
        history(&ui),
        "the reading screen paints instead of the live tail"
    );
    install_projection(&mut ui, None, 0);
    assert_eq!(stand_in(&ui), Some(100), "attached, before its first frame");
    assert!(history(&ui));
}

#[test]
fn typing_before_restoration_chooses_live_and_drops_the_bookmark_image() {
    let (ui, mailbox, _, _) = super::super::test_support::controlling_ui();
    let (mut ui, mailbox, _) = read_at_bookmark(ui, mailbox);
    ui.clear_projections();
    let key = install_projection(&mut ui, Some((400, false, 24)), 0);
    assert!(ui.forward(b"a"));
    let lease = mailbox.try_recv().unwrap().unwrap();
    assert!(matches!(lease.record(), QueuedCommand::Input(_)));
    mailbox.finish_lease(lease).unwrap();
    assert!(
        ui.view()
            .pane_history_stand_in(key.pane().panel())
            .is_none()
    );
    assert!(!ui.drive_history_positions());
    ui.remember_history_positions();
    assert!(matches!(
        ui.history_positions.bank.entries[0].choice,
        Choice::Live
    ));
}
