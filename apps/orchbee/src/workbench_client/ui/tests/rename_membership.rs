use super::super::*;
use crate::workbench_client::mailbox::EventDeadline;
use crate::workbench_client::state::tab_rename::RenameResult;
use crate::{localization::Locale, tui_render::render_tab_rename_for_test, tui_theme::Theme};
use orchbee_workbench_protocol::{WorkbenchRevision, WorkbenchSnapshotOutcome};
use ratatui::{Terminal, backend::TestBackend};
use std::{sync::atomic::Ordering, time::Duration};

#[test]
fn rename_modal_localizes_automatic_names_as_empty_field_placeholders_and_keeps_fixed_prefill() {
    for (locale, expected) in [(Locale::EnUs, "Tab 1"), (Locale::ZhCn, "标签页 1")] {
        let mut source = super::super::tests::membership::snapshot(1, 1);
        let tab = &mut source.terminal_contexts[0].tabs[0];
        tab.automatic_label = true;
        tab.label = "stored ordinal".into();
        let (mut ui, _, _, _) =
            super::super::tests::move_membership::ready_ui_with_snapshot(source);
        ui.request_rename_draft().unwrap();
        let draft = ui.view().rename_draft().unwrap();
        assert_eq!(draft.text(), "");
        assert!(!draft.label_valid());

        let rendered = render_rename_draft(&draft, locale);
        let compact = rendered.replace(' ', "");
        let expected = expected.replace(' ', "");
        assert_eq!(compact.matches(&expected).count(), 2, "{rendered}");
        assert!(
            !rendered.contains(locale.rename_invalid()),
            "an empty automatic-name placeholder is not an input error\n{rendered}"
        );
    }

    let mut source = super::super::tests::membership::snapshot(1, 1);
    source.terminal_contexts[0].tabs[0].label = "Build Shell".into();
    let (mut ui, _, _, _) = super::super::tests::move_membership::ready_ui_with_snapshot(source);
    ui.request_rename_draft().unwrap();
    let draft = ui.view().rename_draft().unwrap();
    assert_eq!(draft.text(), "Build Shell");
    let rendered = render_rename_draft(&draft, Locale::EnUs);
    assert_eq!(rendered.matches("Build Shell").count(), 2, "{rendered}");
}

fn render_rename_draft(
    draft: &crate::workbench_client::RenameDraftView<'_>,
    locale: Locale,
) -> String {
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal
        .draw(|frame| {
            let area = frame.area();
            render_tab_rename_for_test(frame, locale, draft, area, &Theme::terminal_native());
        })
        .unwrap();
    terminal
        .backend()
        .buffer()
        .content
        .chunks(80)
        .map(|row| {
            let mut line: String = row.iter().map(|cell| cell.symbol()).collect();
            line.push('\n');
            line
        })
        .collect()
}

#[test]
fn rename_removed_candidate_samples_one_posttransport_pair_and_never_renews_at_cleanup() {
    for (millis, wrong_key) in [
        (0, false),
        (44_999, false),
        (45_000, false),
        (49_999, false),
        (50_000, false),
        (0, true),
    ] {
        let (mut ui, mailbox, clock, elapsed) = super::super::tests::move_membership::ready_ui();
        let baseline = ui.state.as_ref().unwrap().cache().clone();
        let old = ui.state.as_ref().unwrap().desired().unwrap().clone();
        let token = ui.request_rename_draft().unwrap();
        ui.after_rename_draw(Some(ModalPresentation::Presented(token)));
        ui.edit_rename_draft(token, RenameEdit::Append("changed"));
        assert_eq!(
            ui.submit_rename_draft(token),
            ClientActionDisposition::DismissNavigator
        );
        let lease = mailbox.try_recv().unwrap().unwrap();
        let QueuedCommand::Rename(marker) = lease.record() else {
            panic!("Rename marker");
        };
        let operation = marker.operation().clone();
        mailbox.mark_rename_in_flight(&operation).unwrap();
        let wait = mailbox
            .publish_rename_result_before(
                operation.clone(),
                RenameResult::Committed {
                    revision: WorkbenchRevision::new(2),
                    outcome: WorkbenchSnapshotOutcome::Available {
                        snapshot: super::super::tests::membership::snapshot(0, 3),
                    },
                },
                operation.deadline(),
                &clock,
            )
            .unwrap();
        elapsed.store(75_000, Ordering::SeqCst);
        ui.tick();
        assert!(!ui.retirement_requested());
        let state = ui.state.as_ref().unwrap();
        assert_eq!(state.cache(), &baseline);
        assert_eq!(state.attached_key(), Some(&old));
        assert_eq!(
            state.rename_receipts_for_test().1.unwrap().operation(),
            &operation
        );
        let Some(UiReply::Rename(reply)) = mailbox.take_deposited_reply(&wait).unwrap() else {
            panic!("Rename reply");
        };
        let RefreshDecision::Cleanup(cleanup) = reply.decision() else {
            panic!("common cleanup");
        };
        assert_eq!(
            cleanup.deadlines().forward_cutoff(),
            clock.now() + Duration::from_secs(45)
        );
        assert_eq!(
            cleanup.deadlines().total(),
            clock.now() + Duration::from_secs(50)
        );
        let detached = if wrong_key {
            AttachmentKey::new(old.anchor().generation() + 1, old.tab_id().clone())
        } else {
            old.anchor().clone()
        };
        let wait = mailbox
            .publish_event_before(
                WorkerEvent::MembershipDetached {
                    cleanup: cleanup.clone(),
                    detached,
                },
                EventDeadline::TotalCleanup(cleanup.deadlines().total()),
                &clock,
            )
            .unwrap();
        elapsed.store(75_000 + millis, Ordering::SeqCst);
        ui.tick();
        let state = ui.state.as_ref().unwrap();
        assert_eq!(
            state.rename_receipts_for_test().1.unwrap().operation(),
            &operation
        );
        if millis < 45_000 && !wrong_key {
            assert!(!ui.retirement_requested());
            assert_eq!(ui.view().phase(), DisplayPhase::Empty);
            assert!(state.pending_rename().is_none());
            assert!(
                matches!(mailbox.take_deposited_reply(&wait).unwrap(), Some(UiReply::Rename(reply)) if reply.request_id() == operation.request_id() && reply.desired().is_none())
            );
            mailbox.finish_lease(lease).unwrap();
        } else {
            assert!(ui.retirement_requested());
            assert_eq!(state.cache(), &baseline);
        }
    }
}

#[test]
fn rename_retained_key_consumes_timely_result_after_original_cutoff_without_membership_budget() {
    let (mut ui, mailbox, clock, elapsed) = super::super::tests::move_membership::ready_ui();
    let old = ui.state.as_ref().unwrap().desired().unwrap().clone();
    let token = ui.request_rename_draft().unwrap();
    ui.after_rename_draw(Some(ModalPresentation::Presented(token)));
    ui.edit_rename_draft(token, RenameEdit::Clear);
    ui.edit_rename_draft(token, RenameEdit::Append("new"));
    ui.submit_rename_draft(token);
    let lease = mailbox.try_recv().unwrap().unwrap();
    let QueuedCommand::Rename(marker) = lease.record() else {
        panic!("Rename marker");
    };
    let operation = marker.operation().clone();
    mailbox.mark_rename_in_flight(&operation).unwrap();
    let mut snapshot = super::super::tests::membership::snapshot(2, 2);
    snapshot.terminal_contexts[0].tabs[0].label = "new".into();
    let wait = mailbox
        .publish_rename_result_before(
            operation.clone(),
            RenameResult::Committed {
                revision: WorkbenchRevision::new(2),
                outcome: WorkbenchSnapshotOutcome::Available { snapshot },
            },
            operation.deadline(),
            &clock,
        )
        .unwrap();
    elapsed.store(1_000_000, Ordering::SeqCst);
    ui.tick();
    assert!(!ui.retirement_requested());
    assert_eq!(ui.status(), super::super::super::TerminalStatus::ReadOnly);
    assert_eq!(ui.state.as_ref().unwrap().desired(), Some(&old));
    let Some(UiReply::Rename(reply)) = mailbox.take_deposited_reply(&wait).unwrap() else {
        panic!("Rename reply");
    };
    assert_eq!(reply.decision(), &RefreshDecision::Continue);
    assert_eq!(reply.desired(), Some(&old));
    assert_eq!(operation.deadline() + Duration::from_secs(950), clock.now());
    mailbox.finish_lease(lease).unwrap();
}

#[test]
fn automatic_native_titles_follow_observation_while_fixed_titles_and_status_remain_exact() {
    for automatic in [false, true] {
        let mut source = super::super::tests::membership::snapshot(1, 1);
        source.terminal_contexts[0].tabs[0].automatic_label = automatic;
        source.terminal_contexts[0].tabs[0].label = "dev server".into();
        let (ui, _, _, _) = super::super::tests::move_membership::ready_ui_with_snapshot(source);
        let mut tab = ui.view().tab(0).unwrap();
        tab.observed_title = Some("claude");
        for locale in [Locale::EnUs, Locale::ZhCn] {
            assert_eq!(
                tab.display_label(locale),
                if automatic { "claude" } else { "dev server" }
            );
            if !automatic {
                assert!(
                    locale
                        .native_phase_status(&ui.view())
                        .starts_with("dev server")
                );
            }
        }
    }
}
