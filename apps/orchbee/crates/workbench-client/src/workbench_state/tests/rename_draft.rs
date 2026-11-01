use super::super::rename_draft::RenameDraftEdit;
use super::*;
use crate::tui_presentation::GeometrySettlement;

#[test]
fn rename_draft_refuses_a_blocked_first_draw_but_preserves_an_admitted_editor() {
    let mut refused = ready(2);
    refused.open_rename_draft().unwrap();
    assert_eq!(
        refused.settle_rename_draft_not_presented(),
        Some(GeometrySettlement::Refused)
    );
    assert!(refused.rename_draft().is_none());
    assert_eq!(refused.notice, Some(ClientNotice::DialogRefused));

    let mut suspended = ready(2);
    let token = suspended.open_rename_draft().unwrap();
    suspended.acknowledge_rename_draw(Some(token));
    assert!(suspended.rename_draft_input_ready(token));
    assert!(suspended.edit_rename_draft(token, RenameDraftEdit::Append(" preserved")));
    let text = suspended.rename_draft().unwrap().text().to_owned();
    assert_eq!(
        suspended.settle_rename_draft_not_presented(),
        Some(GeometrySettlement::Suspended)
    );
    assert_eq!(suspended.rename_draft().unwrap().token(), token);
    assert_eq!(suspended.rename_draft().unwrap().text(), text);
    assert!(!suspended.rename_draft_input_ready(token));
    assert!(suspended.plan_rename_draft(token).is_none());
    suspended.acknowledge_rename_draw(Some(token));
    assert!(suspended.rename_draft_input_ready(token));
    assert_eq!(suspended.rename_draft().unwrap().text(), text);
}

#[test]
fn rename_draft_requires_exact_draw_and_keeps_unchanged_text_authoritative() {
    let mut state = ready(2);
    let baseline = state.cache().clone();
    let desired = state.desired().unwrap().clone();
    let token = state.open_rename_draft().unwrap();
    assert!(state.plan_rename_draft(token).is_none());
    state.acknowledge_rename_draw(Some(token));
    let intent = state.plan_rename_draft(token).unwrap();
    assert_eq!(intent.token(), token);
    assert_eq!(state.rename_draft().unwrap().token(), token);
    assert_eq!(intent.baseline(), &baseline);
    assert_eq!(intent.target(), &desired);
    assert_eq!(intent.label(), baseline.tabs()[0].tab_label());
    state.acknowledge_rename_draw(None);
    assert!(state.plan_rename_draft(token).is_none());
    state.acknowledge_rename_draw(Some(token));
    assert!(state.plan_rename_draft(token).is_some());
    assert_eq!(state.phase_kind(), ClientPhaseKind::Ready);
    assert_eq!(state.cache(), &baseline);
    assert_eq!(state.desired(), Some(&desired));
}

#[test]
fn rename_draft_fault_only_clears_with_explicit_backspace_or_clear() {
    let mut state = ready(2);
    let token = state.open_rename_draft().unwrap();
    state.acknowledge_rename_draw(Some(token));
    assert!(state.edit_rename_draft(token, RenameDraftEdit::Clear));
    assert!(state.plan_rename_draft(token).is_none());
    assert!(state.edit_rename_draft(token, RenameDraftEdit::Fault));
    assert!(state.edit_rename_draft(token, RenameDraftEdit::Append("雪é")));
    assert!(state.rename_draft().unwrap().input_fault());
    assert!(state.plan_rename_draft(token).is_none());
    assert!(state.edit_rename_draft(token, RenameDraftEdit::Backspace));
    assert_eq!(
        state.plan_rename_draft(token).unwrap().label().as_str(),
        "雪"
    );
    assert!(state.edit_rename_draft(token, RenameDraftEdit::Append(&"x".repeat(512))));
    assert_eq!(state.rename_draft().unwrap().text(), "雪");
    assert!(state.rename_draft().unwrap().input_fault());
    assert!(state.edit_rename_draft(token, RenameDraftEdit::Clear));
    assert!(!state.rename_draft().unwrap().input_fault());
    assert!(state.edit_rename_draft(token, RenameDraftEdit::Fault));
    assert!(state.edit_rename_draft(token, RenameDraftEdit::Backspace));
    assert!(!state.rename_draft().unwrap().input_fault());
    assert!(state.edit_rename_draft(token, RenameDraftEdit::Append("  \t ")));
    assert!(state.plan_rename_draft(token).is_none());
}

#[test]
fn rename_draft_incomplete_input_cannot_submit_or_correct_fault() {
    let mut state = ready(2);
    let token = state.open_rename_draft().unwrap();
    state.acknowledge_rename_draw(Some(token));
    state.edit_rename_draft(token, RenameDraftEdit::Fault);
    state.edit_rename_draft(token, RenameDraftEdit::Completeness(false));
    assert!(!state.edit_rename_draft(token, RenameDraftEdit::Backspace));
    assert!(!state.edit_rename_draft(token, RenameDraftEdit::Clear));
    assert!(state.plan_rename_draft(token).is_none());
    state.edit_rename_draft(token, RenameDraftEdit::Completeness(true));
    assert!(state.rename_draft().unwrap().input_fault());
    state.edit_rename_draft(token, RenameDraftEdit::Clear);
    state.edit_rename_draft(token, RenameDraftEdit::Append("new"));
    assert!(state.plan_rename_draft(token).is_some());
}

#[test]
fn rename_draft_cancellation_and_stale_authority_never_retarget_text() {
    let mut state = ready(2);
    let old = state.open_rename_draft().unwrap();
    state.acknowledge_rename_draw(Some(old));
    state.cancel_rename_draft();
    let next = state.open_rename_draft().unwrap();
    assert_ne!(old, next);
    state.acknowledge_rename_draw(Some(old));
    assert!(state.plan_rename_draft(next).is_none());
    assert!(!state.edit_rename_draft(old, RenameDraftEdit::Clear));
    state.acknowledge_rename_draw(Some(next));
    state.cache = projected(2, 2);
    assert!(state.rename_draft().is_none());
    assert!(state.plan_rename_draft(next).is_none());
    assert!(!state.edit_rename_draft(next, RenameDraftEdit::Append("stale")));
    assert!(state.rename_draft().is_none());
}

#[test]
fn rename_draft_and_structural_admission_are_mutually_exclusive() {
    let mut state = ready(2);
    let create = create_operation(&state, 2, 90, "new");
    let switch = state
        .target_for_switch(
            1,
            AttachmentKey::new(2, state.tab(1).unwrap().tab_id().clone()),
            OperationDeadlines::from_sample(Instant::now()).unwrap(),
        )
        .unwrap();
    let token = state.open_rename_draft().unwrap();
    assert!(!state.begin_create(create));
    assert!(!state.begin_switch(switch));
    assert!(state.open_rename_draft().is_none());
    assert!(state.open_close_confirmation().is_err());
    assert_eq!(
        state.plan_selection(ClientAction::SelectNextTab),
        SelectionPlan::Busy
    );
    assert!(!state.start_pending_refresh(1));
    assert!(
        state
            .plan_move(
                WorkbenchRequestId::parse(&id(100)).unwrap(),
                orchbee_workbench_protocol::WorkbenchTabMoveDirection::Next,
                Instant::now() + OPERATION_TOTAL
            )
            .is_none()
    );
    state.cancel_rename_draft();
    assert!(state.open_close_confirmation().is_ok());
    assert!(state.open_rename_draft().is_none());
    state.cancel_close_confirmation();
    assert!(state.start_pending_refresh(1));
    assert!(state.open_rename_draft().is_none());
    assert!(state.plan_rename_draft(token).is_none());
}

#[test]
fn rename_draft_accepts_stable_unattached_phases_and_retirement_destroys_it() {
    assert!(state(2).open_rename_draft().is_none());
    for selected_without_attachment in [false, true] {
        let mut state = retryable_state(Instant::now());
        if selected_without_attachment {
            state.phase = ClientPhase::SelectedWithoutAttachment(state.active().unwrap().clone());
        }
        let phase = state.phase_kind();
        let baseline = state.cache().clone();
        let desired = state.desired().unwrap().clone();
        let token = state.open_rename_draft().unwrap();
        state.acknowledge_rename_draw(Some(token));
        assert!(state.plan_rename_draft(token).is_some());
        assert_eq!(state.phase_kind(), phase);
        assert_eq!(state.desired(), Some(&desired));
        assert_eq!(state.cache(), &baseline);
        assert!(state.invariant_holds());
        state.retire(CarrierFailure::Transport);
        assert!(state.rename_draft().is_none());
        assert!(!state.rename_draft_blocks_structure());
        assert!(state.plan_rename_draft(token).is_none());
        assert!(state.open_rename_draft().is_none());
    }
}

#[test]
fn rename_draft_rejects_changed_attachment_generation_independently_of_revision() {
    let mut state = ready(2);
    let token = state.open_rename_draft().unwrap();
    state.acknowledge_rename_draw(Some(token));
    state.desired = Some(PaneKey::new(
        AttachmentKey::new(2, state.desired().unwrap().tab_id().clone()),
        state.desired().unwrap().panel().clone(),
    ));
    assert!(state.plan_rename_draft(token).is_none());
    assert!(state.rename_draft().is_none());
    state.acknowledge_rename_draw(Some(token));
    assert!(!state.rename_draft_blocks_structure());
}

#[test]
fn rename_draft_edits_graphemes_and_projects_its_cursor_through_the_window() {
    let mut state = ready(2);
    let token = state.open_rename_draft().unwrap();
    state.acknowledge_rename_draw(Some(token));
    let prefill = state.rename_draft().unwrap().text().to_owned();
    // Prefill lands the cursor at the exact raw end: an append extends the
    // label, a left step walks back over the whole appended grapheme, delete
    // removes it.
    assert!(state.edit_rename_draft(token, RenameDraftEdit::Append("\u{4e2d}")));
    let mut text = format!("{prefill}\u{4e2d}");
    assert_eq!(state.rename_draft().unwrap().text(), text);
    assert!(state.edit_rename_draft(token, RenameDraftEdit::Left));
    assert!(state.edit_rename_draft(token, RenameDraftEdit::Delete));
    assert_eq!(state.rename_draft().unwrap().text(), prefill);
    // Home + append inserts at the head; End + paste lands atomically at the
    // tail, keeping literal controls in the raw text.
    assert!(state.edit_rename_draft(token, RenameDraftEdit::Home));
    assert!(state.edit_rename_draft(token, RenameDraftEdit::Append("X")));
    text = format!("X{prefill}");
    assert!(state.edit_rename_draft(token, RenameDraftEdit::End));
    assert!(state.edit_rename_draft(token, RenameDraftEdit::Paste(" \u{754c}\x07z")));
    text.push_str(" \u{754c}\x07z");
    assert_eq!(state.rename_draft().unwrap().text(), text);
    // Word-backspace deletes a contiguous suffix back to the last Unicode
    // word boundary — \x07 breaks words, so the first deletion removes `z`
    // alone and a second removes the word run before it.
    assert!(state.edit_rename_draft(token, RenameDraftEdit::WordBackspace));
    let after_first = state.rename_draft().unwrap().text().to_owned();
    assert!(text.starts_with(&after_first) && after_first.len() < text.len());
    assert!(after_first.ends_with('\x07'));
    assert!(state.edit_rename_draft(token, RenameDraftEdit::WordBackspace));
    // The second deletion spans from the last word's start to the cursor —
    // the trailing non-word \x07 goes with it.
    assert_eq!(state.rename_draft().unwrap().text(), format!("X{prefill} "));
    // A raw cursor over a stripped control still lands inside the field: the
    // window's cursor column is a display cell, never a raw byte index.
    let window = state.rename_draft().unwrap().field_window(6).unwrap();
    assert!(window.cursor_column() < 6);
    assert!(crate::terminal_text::display_width(window.display()) <= 6);
    // Over-limit insertion faults atomically: text and cursor unchanged.
    let before = state.rename_draft().unwrap().text().to_owned();
    assert!(state.edit_rename_draft(token, RenameDraftEdit::Append(&"x".repeat(512))));
    assert_eq!(state.rename_draft().unwrap().text(), before);
    assert!(state.rename_draft().unwrap().input_fault());
    assert!(state.edit_rename_draft(token, RenameDraftEdit::Backspace));
    assert!(!state.rename_draft().unwrap().input_fault());
    assert!(state.plan_rename_draft(token).is_some());
}
