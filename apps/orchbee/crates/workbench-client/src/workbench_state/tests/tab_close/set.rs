//! Set close traces: the Server closes several Tabs as one operation the Client follows, and one
//! Tab closes as the key closes it.
use super::*;
use crate::workbench_state::{
    CloseLeaseAcknowledgement, CloseOperationKind, CloseSetKind, ControllerMode, FrameIdentity,
    MembershipInstallDecision, OperationDeadlines, close_set_targets,
};
use orchbee_workbench_protocol::{
    Snapshot, TabSetCloseAction, TabSetCloseChild, TabSetCloseFailure, TabSetCloseMember,
    TabSetCloseObservation, TabSetCloseOutcome, TabSetCloseRejection, TabSetCloseStatus,
    WorkbenchLabel, WorkbenchTabCloseReadOutcome, WorkbenchTabGroupId, WorkbenchTabId,
};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Corpus {
    contract_version: u32,
    cases: Vec<Case>,
}
#[derive(Deserialize)]
struct Case {
    name: String,
    tabs: Vec<TabInput>,
    selected: Vec<String>,
    target: String,
    expected: Expected,
}
#[derive(Deserialize)]
struct TabInput {
    id: String,
    pinned: bool,
    group: Option<String>,
}
#[derive(Deserialize)]
struct Expected {
    selected: Vec<String>,
    other: Vec<String>,
    trailing: Vec<String>,
    group: Vec<String>,
}
fn id(value: &str) -> WorkbenchTabId {
    WorkbenchTabId::parse(&format!("{:032x}", value.parse::<u128>().unwrap())).unwrap()
}

#[test]
fn close_set_target_corpus_matches_the_shared_tab_close_contract() {
    let corpus: Corpus = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../schemas/workbench/v1/tab-close-targets-conformance-v1.json"
    )))
    .unwrap();
    assert_eq!(corpus.contract_version, 1);
    for case in corpus.cases {
        let mut source = snapshot(case.tabs.len(), 1, None);
        let tabs = &mut source.terminal_contexts[0].tabs;
        for (tab, input) in tabs.iter_mut().zip(&case.tabs) {
            tab.tab_id = id(&input.id);
            tab.pinned = input.pinned;
            tab.group_id = input.group.as_ref().map(|value| {
                WorkbenchTabGroupId::parse(&format!(
                    "{:032x}",
                    1000 + value.parse::<u128>().unwrap()
                ))
                .unwrap()
            });
        }
        let selected: Vec<_> = case.selected.iter().map(|value| id(value)).collect();
        for (kind, expected) in [
            (CloseSetKind::Selected, case.expected.selected),
            (CloseSetKind::Other, case.expected.other),
            (CloseSetKind::Trailing, case.expected.trailing),
            (CloseSetKind::Group, case.expected.group),
        ] {
            assert_eq!(
                close_set_targets(tabs, &selected, &id(&case.target), kind),
                expected.iter().map(|value| id(value)).collect::<Vec<_>>(),
                "{} {kind:?}",
                case.name
            );
        }
    }
}

fn open(state: &mut WorkbenchClientState, indexes: &[usize], now: Instant) -> CloseOperation {
    let ids: Vec<_> = indexes
        .iter()
        .map(|index| state.cache().tabs()[*index].tab_id().clone())
        .collect();
    state.open_close_set(&ids).unwrap();
    assert_eq!(
        state.plan_close_confirmation(workbench_request_id(8000), now + Duration::from_secs(50)),
        Err(CloseStateError::NotDisplayed)
    );
    let token = state.view().close_confirmation().unwrap().token();
    state.acknowledge_close_draw(Some(token), None);
    let operation = state
        .plan_close_confirmation(workbench_request_id(8000), now + Duration::from_secs(50))
        .unwrap();
    state.begin_close_exchange(&operation, now).unwrap();
    operation
}

fn accept(state: &mut WorkbenchClientState, close: &CloseOperation) {
    // Accepted can name an already admitted primary. The request ID need not be that identity.
    let alias = workbench_request_id(8888);
    assert_ne!(close.key().request_id(), &alias);
    state
        .settle_close_exchange(
            close,
            &CloseResult::Accepted(WorkbenchTabCloseAccepted {
                close_request_id: alias,
                terminal_context_id: close.key().target().terminal_context_id().clone(),
                tab_id: close.key().target().tab_id().clone(),
                accepted_revision: close.selection().base_revision(),
            }),
        )
        .unwrap();
    state.acknowledge_close_lease(CloseLeaseAcknowledgement::Exchange(close.key().clone()));
}

fn read_status(
    state: &mut WorkbenchClientState,
    close: &CloseOperation,
    status: WorkbenchTabCloseStatus,
    now: Instant,
) -> CloseOperation {
    let alias = workbench_request_id(8888);
    let read = state
        .plan_close_read(
            workbench_request_id(8001 + u128::from(state.cache().revision().value())),
            now + Duration::from_secs(50),
            now,
        )
        .unwrap();
    assert!(
        matches!(read.kind(), CloseOperationKind::Read { expected_operation: Some(known), .. } if known == &alias)
    );
    state.begin_close_exchange(&read, now).unwrap();
    state
        .settle_close_exchange(
            &read,
            &CloseResult::Read(WorkbenchTabCloseReadOutcome::Available {
                close_request_id: alias,
                terminal_context_id: close.key().target().terminal_context_id().clone(),
                tab_id: close.key().target().tab_id().clone(),
                attempt_generation: WorkbenchTabCloseAttemptGeneration::new(1).unwrap(),
                status,
            }),
        )
        .unwrap();
    state.acknowledge_close_lease(CloseLeaseAcknowledgement::Exchange(read.key().clone()));
    read
}

fn closed_status(state: &WorkbenchClientState) -> WorkbenchTabCloseStatus {
    WorkbenchTabCloseStatus::Closed {
        revision: WorkbenchRevision::new(state.cache().revision().value() + 1),
    }
}

fn observe_closed(
    state: &mut WorkbenchClientState,
    close: &CloseOperation,
    now: Instant,
) -> CloseOperation {
    accept(state, close);
    let closed = closed_status(state);
    read_status(state, close, closed, now)
}

fn screen(state: &WorkbenchClientState, locale: crate::localization::Locale) -> String {
    use crate::client_view::ClientView;
    use crate::localization::ClientProjectionStatus;
    use crate::tui_hit_map::HitMapBuilder;
    use crate::tui_theme::Theme;
    use crate::workbench_client::WorkbenchClientView;
    use ratatui::{Terminal, backend::TestBackend};
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
    terminal
        .draw(|frame| {
            crate::tui_render::render_terminal_shell(
                frame,
                &ClientView::new(120),
                locale,
                Some(WorkbenchClientView::test_view(
                    state,
                    &[],
                    ClientProjectionStatus::Empty,
                )),
                &Theme::terminal_native(),
                &mut HitMapBuilder::new(),
            );
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    let mut text = String::new();
    for y in 0..40 {
        let mut x = 0;
        while x < 120 {
            let symbol = buffer[(x, y)].symbol();
            text.push_str(symbol);
            x += u16::try_from(crate::terminal_text::display_width(symbol).max(1)).unwrap();
        }
        text.push('\n');
    }
    text
}

fn removed(state: &WorkbenchClientState, tab: &WorkbenchTabId) -> Snapshot {
    let mut source = state.cache().canonical_snapshot().clone();
    source.revision = WorkbenchRevision::new(source.revision.value() + 1);
    let context = &mut source.terminal_contexts[0];
    let group = context
        .tabs
        .iter()
        .find(|member| &member.tab_id == tab)
        .unwrap()
        .group_id
        .clone();
    context.tabs.retain(|member| &member.tab_id != tab);
    context.groups.retain(|candidate| {
        Some(&candidate.group_id) != group.as_ref()
            || context
                .tabs
                .iter()
                .any(|member| member.group_id.as_ref() == Some(&candidate.group_id))
    });
    source
}

fn install(
    state: &mut WorkbenchClientState,
    close: &CloseOperation,
    source: Snapshot,
    now: Instant,
) {
    let marker = state
        .plan_close_reconciliation(
            workbench_request_id(9000 + source.revision.value() as u128),
            close.deadline(),
        )
        .unwrap();
    state.begin_close_reconciliation(&marker).unwrap();
    let prepared = state
        .prepare_close_membership_candidate(
            &marker,
            WorkbenchSnapshotOutcome::Available { snapshot: source },
        )
        .unwrap()
        .unwrap();
    let decision = if prepared.preserves_selection() {
        state
            .prepare_close_membership_install(
                &marker,
                prepared,
                state.desired().map(|key| key.anchor().clone()),
                None,
            )
            .unwrap()
            .commit()
    } else {
        let fresh = prepared
            .selected_tab_id()
            .map(|tab| AttachmentKey::new(100 + state.cache().revision().value(), tab.clone()));
        let cleanup = state
            .stage_close_membership_cleanup(
                &marker,
                prepared,
                OperationDeadlines::from_sample(now).unwrap(),
            )
            .unwrap();
        let candidate = state
            .prepare_cleaned_membership_candidate(&cleanup, cleanup.old())
            .unwrap();
        state
            .prepare_cleaned_membership_install(&cleanup, candidate, fresh)
            .unwrap()
            .commit()
    };
    if let MembershipInstallDecision::Attach(target) = decision {
        let key = target.selected().key().clone();
        assert!(state.install_projection_t(&key));
        assert!(state.record_setup_configuration_t(&key, ControllerMode::ReadOnly, None));
        assert!(state.stage_drawn_frame_t(&key, FrameIdentity::new(1)));
        assert!(state.mark_feedback_accepted_t(&key, FrameIdentity::new(1)));
        assert!(state.publish_ready_t(&key, FrameIdentity::new(1), ControllerMode::ReadOnly, None));
    }
    state.acknowledge_close_lease(CloseLeaseAcknowledgement::Reconciliation(Box::new(marker)));
    assert!(state.invariant_holds());
}

/// The Tabs at `indexes`, in their order.
fn tab_ids(state: &WorkbenchClientState, indexes: &[usize]) -> Vec<WorkbenchTabId> {
    indexes
        .iter()
        .map(|index| state.cache().tabs()[*index].tab_id().clone())
        .collect()
}

/// The Server's report of the set `start` names, with every member still open.
fn report(start: &CloseOperation) -> Vec<TabSetCloseMember> {
    let CloseOperationKind::TabSet(TabSetCloseAction::Start { tab_ids, .. }) = start.kind() else {
        panic!("a set Start");
    };
    tab_ids
        .iter()
        .map(|tab_id| TabSetCloseMember {
            tab_id: tab_id.clone(),
            label: WorkbenchLabel::parse("member").unwrap(),
            close: None,
            absent: false,
        })
        .collect()
}

/// A member's own Close as the Server last observed it.
fn child(request: u128, status: WorkbenchTabCloseStatus) -> Option<TabSetCloseChild> {
    Some(TabSetCloseChild {
        request_id: workbench_request_id(request),
        observation: Some(TabSetCloseObservation {
            attempt_generation: WorkbenchTabCloseAttemptGeneration::new(7).unwrap(),
            status,
        }),
    })
}

fn closing() -> WorkbenchTabCloseStatus {
    WorkbenchTabCloseStatus::Closing {
        phase: orchbee_workbench_protocol::WorkbenchTabClosePhase::WaitingGrace,
    }
}

fn available(
    start: &CloseOperation,
    status: TabSetCloseStatus,
    members: Vec<TabSetCloseMember>,
) -> CloseResult {
    let CloseOperationKind::TabSet(TabSetCloseAction::Start {
        terminal_context_id,
        condition,
        ..
    }) = start.kind()
    else {
        panic!("a set Start");
    };
    CloseResult::TabSet(TabSetCloseOutcome::Available {
        terminal_context_id: terminal_context_id.clone(),
        condition: *condition,
        status,
        members,
    })
}

fn race(revision: u64) -> CloseResult {
    CloseResult::TabSet(TabSetCloseOutcome::Rejected {
        revision: WorkbenchRevision::new(revision),
        reason: TabSetCloseRejection::RevisionMismatch,
    })
}

/// Settles one exchange and releases its lease, as the UI does.
fn settle(state: &mut WorkbenchClientState, exchange: &CloseOperation, result: &CloseResult) {
    state.settle_close_exchange(exchange, result).unwrap();
    state.acknowledge_close_lease(CloseLeaseAcknowledgement::Exchange(exchange.key().clone()));
}

/// Admits the accepted set's next Read at `now`.
fn read_set(state: &mut WorkbenchClientState, request: u128, now: Instant) -> CloseOperation {
    let read = state
        .plan_tab_set_read(
            workbench_request_id(request),
            now + Duration::from_secs(50),
            now,
        )
        .unwrap();
    state.begin_close_exchange(&read, now).unwrap();
    read
}

/// Reads the Close the stopped set left to `tab_id`, which the Server now reports as `status`.
fn read_member(
    state: &mut WorkbenchClientState,
    tab_id: &WorkbenchTabId,
    request: u128,
    status: WorkbenchTabCloseStatus,
    now: Instant,
) {
    let read = state
        .plan_close_read(
            workbench_request_id(request),
            now + Duration::from_secs(50),
            now,
        )
        .unwrap();
    assert_eq!(read.key().target().tab_id(), tab_id);
    let CloseOperationKind::Read {
        expected_operation: Some(known),
        ..
    } = read.kind()
    else {
        panic!("the member's own Close");
    };
    let result = CloseResult::Read(WorkbenchTabCloseReadOutcome::Available {
        close_request_id: known.clone(),
        terminal_context_id: read.key().target().terminal_context_id().clone(),
        tab_id: tab_id.clone(),
        attempt_generation: WorkbenchTabCloseAttemptGeneration::new(7).unwrap(),
        status,
    });
    state.begin_close_exchange(&read, now).unwrap();
    settle(state, &read, &result);
}

/// A peer's edit lands at `revision`: beside the set, or on its Tab at `member`.
fn advance(state: &mut WorkbenchClientState, revision: u64, member: Option<usize>) {
    let mut peer = state.cache().canonical_snapshot().clone();
    peer.revision = WorkbenchRevision::new(revision);
    match member {
        Some(index) => peer.terminal_contexts[0].tabs[index].label = "peer edit".into(),
        None => peer.navigation.workspaces[0].label = format!("peer {revision}"),
    }
    state.cache = state.cache.project_snapshot(&peer).unwrap();
}

/// Several Tabs close as one Server operation: nothing leaves before the Server accepts it, and
/// from then on every member is out of the strip and the status line counts them.
#[test]
fn a_set_counts_from_its_acceptance_and_stops_reporting_once_every_member_is_closed() {
    use crate::localization::Locale;
    for (locale, running, one_closed, stopped) in [
        (
            Locale::EnUs,
            "Closing Tabs: 0 closed, 2 remaining",
            "Closing Tabs: 1 closed, 1 remaining",
            "Closing stopped: 1 closed, 1 unconfirmed",
        ),
        (
            Locale::ZhCn,
            "正在关闭标签页：0 个已关闭，剩余 2 个",
            "正在关闭标签页：1 个已关闭，剩余 1 个",
            "关闭已停止：1 个已关闭，1 个未确认",
        ),
    ] {
        let now = Instant::now();
        let mut state = ready(3);
        let members = tab_ids(&state, &[1, 2]);
        let start = open(&mut state, &[1, 2], now);
        assert!(members.iter().all(|tab| !state.view().tab_leaving(tab)));
        assert!(state.view().close_set_status().is_none());

        settle(
            &mut state,
            &start,
            &available(&start, TabSetCloseStatus::Pending {}, report(&start)),
        );
        assert!(members.iter().all(|tab| state.view().tab_leaving(tab)));
        let shown = screen(&state, locale);
        assert!(shown.contains(running), "{shown}");

        let later = now + Duration::from_millis(250);
        let read = read_set(&mut state, 9100, later);
        let mut members_report = report(&start);
        members_report[0].close = child(61000, closed_status(&state));
        settle(
            &mut state,
            &read,
            &available(
                &start,
                TabSetCloseStatus::Pending {},
                members_report.clone(),
            ),
        );
        let shown = screen(&state, locale);
        assert!(shown.contains(one_closed), "{shown}");

        // The set stops while the second member's own Close may still finish.
        let later = later + Duration::from_millis(250);
        let read = read_set(&mut state, 9101, later);
        members_report[1].close = child(61001, closing());
        settle(
            &mut state,
            &read,
            &available(
                &start,
                TabSetCloseStatus::Failed {
                    reason: TabSetCloseFailure::Deadline,
                },
                members_report,
            ),
        );
        let shown = screen(&state, locale);
        assert!(shown.contains(stopped), "{shown}");
        assert!(!state.close_set_read_due(later + Duration::from_secs(1)));

        let closed = closed_status(&state);
        read_member(&mut state, &members[1], 9102, closed, later);
        let facts = state.close_set_status().unwrap();
        assert_eq!((facts.completed, facts.total, facts.halted), (2, 2, true));
        assert!(state.view().close_set_status().is_none());
    }
}

#[test]
fn close_set_completed_summary_expires_or_yields_to_the_next_notice_without_erasing_facts() {
    use crate::localization::Locale;
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for next in ["timeout", "notice", "action", "halted-notice"] {
            let now = Instant::now();
            let mut state = ready(3);
            let start = open(&mut state, &[1, 2], now);
            settle(
                &mut state,
                &start,
                &available(&start, TabSetCloseStatus::Pending {}, report(&start)),
            );
            let later = now + Duration::from_millis(250);
            let read = read_set(&mut state, 9100, later);
            let mut members = report(&start);
            members[0].close = child(61000, closed_status(&state));
            if next == "halted-notice" {
                members[1].close = child(61001, closing());
                let failed = TabSetCloseStatus::Failed {
                    reason: TabSetCloseFailure::Deadline,
                };
                settle(&mut state, &read, &available(&start, failed, members));
            } else {
                members[1].close = child(61001, closed_status(&state));
                settle(
                    &mut state,
                    &read,
                    &available(&start, TabSetCloseStatus::Closed {}, members),
                );
                assert!(state.settle_close_set(later));
                assert_eq!(
                    state.close_set_summary_deadline(),
                    Some(later + Duration::from_secs(5))
                );
                assert!(!state.settle_close_set(later + Duration::from_secs(4)));
                // The completed form replaces the running count.
                let shown = screen(&state, locale);
                assert!(
                    shown.contains(match locale {
                        Locale::EnUs => "2 Tabs closed",
                        Locale::ZhCn => "已关闭 2 个标签页",
                    }),
                    "{shown}"
                );
            }
            let facts = state.close_set_status();
            match next {
                "timeout" => {
                    assert!(state.settle_close_set(later + Duration::from_secs(5)));
                }
                "action" => state.dismiss_close_set_summary(),
                _ => {
                    state.set_notice(ClientNotice::CloseRequestNotApplied);
                    let shown = screen(&state, locale);
                    assert!(
                        shown.contains(&locale.native_notice(
                            crate::localization::WorkbenchClientNotice::CloseRequestNotApplied
                        )),
                        "{shown}"
                    );
                    assert!(!shown.contains(&locale.close_set_status(facts.unwrap())));
                    state.clear_notice();
                }
            }
            assert_eq!(state.close_set_status(), facts);
            if next == "halted-notice" {
                // A member's Close that may still finish keeps the report past any notice.
                assert!(state.view().close_set_status().unwrap().halted);
                assert!(!state.close_set_read_due(later + Duration::from_secs(1)));
            } else {
                assert!(state.view().close_set_status().is_none());
                assert!(state.close_set_summary_deadline().is_none());
                assert!(!screen(&state, locale).contains(&locale.close_set_status(facts.unwrap())));
                assert!(!state.settle_close_set(later + Duration::from_secs(60)));
            }
        }
    }
}

#[test]
fn a_stopped_set_reports_while_a_close_may_finish_and_then_clears_like_any_notice() {
    use crate::localization::Locale;
    for (locale, left_open, in_doubt) in [
        (
            Locale::EnUs,
            "Closing stopped: 1 closed, 2 still open",
            "Closing stopped: 0 closed, 1 unconfirmed, 2 still open",
        ),
        (
            Locale::ZhCn,
            "关闭已停止：1 个已关闭，2 个仍打开",
            "关闭已停止：0 个已关闭，1 个未确认，2 个仍打开",
        ),
    ] {
        for dismissed in [false, true] {
            // A member's terminals changed: the set stops with nothing left in flight.
            let now = Instant::now();
            let mut state = ready(4);
            let start = open(&mut state, &[1, 2, 3], now);
            settle(
                &mut state,
                &start,
                &available(&start, TabSetCloseStatus::Pending {}, report(&start)),
            );
            let read = read_set(&mut state, 9100, now + Duration::from_millis(250));
            let mut members = report(&start);
            members[0].close = child(61000, closed_status(&state));
            let failed = TabSetCloseStatus::Failed {
                reason: TabSetCloseFailure::SourceChanged,
            };
            settle(&mut state, &read, &available(&start, failed, members));
            let facts = state.close_set_status();
            assert!(facts.unwrap().halted);
            assert!(!facts.unwrap().unconfirmed);
            assert!(screen(&state, locale).contains(left_open));
            assert!(state.settle_close_set(now));
            assert_eq!(
                state.close_set_summary_deadline(),
                Some(now + Duration::from_secs(5))
            );
            assert!(!state.settle_close_set(now + Duration::from_secs(4)));
            assert!(screen(&state, locale).contains(left_open));
            if dismissed {
                state.dismiss_close_set_summary();
            } else {
                // Expiry is a change the next wake redraws.
                assert!(state.settle_close_set(now + Duration::from_secs(5)));
            }
            assert!(state.view().close_set_status().is_none());
            assert!(state.close_set_summary_deadline().is_none());
            assert!(!screen(&state, locale).contains(left_open));
            assert_eq!(state.close_set_status(), facts);
        }

        // The set stops while its first member's own Close may still finish; that report
        // outlasts any idle time.
        let now = Instant::now();
        let mut state = ready(4);
        let first = state.cache().tabs()[1].tab_id().clone();
        let start = open(&mut state, &[1, 2, 3], now);
        settle(
            &mut state,
            &start,
            &available(&start, TabSetCloseStatus::Pending {}, report(&start)),
        );
        let later = now + Duration::from_millis(250);
        let read = read_set(&mut state, 9100, later);
        let mut members = report(&start);
        members[0].close = child(61000, closing());
        let failed = TabSetCloseStatus::Failed {
            reason: TabSetCloseFailure::SessionStopping,
        };
        settle(&mut state, &read, &available(&start, failed, members));
        assert!(state.close_set_status().unwrap().unconfirmed);
        assert!(screen(&state, locale).contains(in_doubt));
        assert!(state.close_set_summary_deadline().is_none());
        state.dismiss_close_set_summary();
        assert!(!state.settle_close_set(later + Duration::from_secs(60)));
        assert!(screen(&state, locale).contains(in_doubt));

        // Its outcome leaves nothing in flight, and the report starts to clear.
        let later = later + Duration::from_secs(60);
        let closed = closed_status(&state);
        read_member(&mut state, &first, 9101, closed, later);
        assert!(!state.close_set_status().unwrap().unconfirmed);
        assert!(screen(&state, locale).contains(left_open));
        assert!(state.settle_close_set(later));
        assert!(state.settle_close_set(later + Duration::from_secs(5)));
        assert!(state.view().close_set_status().is_none());
    }
}

/// Every member leaves the strip as the Server accepts the set, and a selected member hands the
/// selection to the next Tab outside it; with no Tab left, the last removal empties the Client.
/// Nothing leaves or moves before.
#[test]
fn members_leave_and_the_selection_moves_when_the_server_accepts_the_set() {
    for (indexes, successor) in [(&[0, 1][..], Some(2)), (&[0, 1, 2, 3][..], None)] {
        let now = Instant::now();
        let mut state = ready(4);
        let desired = state.desired().cloned();
        let members = tab_ids(&state, indexes);
        let start = open(&mut state, indexes, now);
        assert!(members.iter().all(|tab| !state.view().tab_leaving(tab)));
        assert_eq!(state.leaving_selection_successor(), None);
        settle(
            &mut state,
            &start,
            &available(&start, TabSetCloseStatus::Pending {}, report(&start)),
        );
        assert!(members.iter().all(|tab| state.view().tab_leaving(tab)));
        assert_eq!(state.leaving_selection_successor(), successor);
        assert_eq!(state.view().last_tab_leaving(), successor.is_none());
        assert_eq!(state.desired().cloned(), desired);
    }
}

/// A set the Server holds runs to its end whatever the focus or the marks do meanwhile.
#[test]
fn a_focus_or_mark_change_stops_no_set() {
    let now = Instant::now();
    let mut state = ready(4);
    let marked = state.cache().tabs()[2].tab_id().clone();
    state
        .mark_tabs(&marked, crate::workbench_state::TabSelectionChange::Toggle)
        .unwrap();
    let members = tab_ids(&state, &[1, 2, 3]);
    let start = open(&mut state, &[1, 2, 3], now);
    settle(
        &mut state,
        &start,
        &available(&start, TabSetCloseStatus::Pending {}, report(&start)),
    );
    state.clear_tab_selection();
    rebind_same_focused_pane(&mut state, now);
    assert!(!state.settle_close_set(now));
    let status = state.close_set_status().unwrap();
    assert_eq!(
        (status.completed, status.total, status.halted),
        (0, 3, false)
    );
    assert!(members.iter().all(|tab| state.view().tab_leaving(tab)));

    let read = read_set(&mut state, 9100, now + Duration::from_millis(250));
    let mut closed = report(&start);
    for (index, member) in closed.iter_mut().enumerate() {
        member.close = child(61000 + index as u128, closed_status(&state));
    }
    settle(
        &mut state,
        &read,
        &available(&start, TabSetCloseStatus::Closed {}, closed),
    );
    let status = state.close_set_status().unwrap();
    assert_eq!(
        (status.completed, status.total, status.halted),
        (3, 3, false)
    );
}

/// A stopped set brings back every member it did not close, and the set never goes again.
#[test]
fn a_failed_set_returns_its_open_members() {
    let now = Instant::now();
    let mut state = ready(4);
    let members = tab_ids(&state, &[1, 2, 3]);
    let start = open(&mut state, &[1, 2, 3], now);
    settle(
        &mut state,
        &start,
        &available(&start, TabSetCloseStatus::Pending {}, report(&start)),
    );
    let later = now + Duration::from_millis(250);
    let read = read_set(&mut state, 9100, later);
    let mut stopped = report(&start);
    stopped[0].close = child(61000, closed_status(&state));
    stopped[1].close = child(61001, parked());
    let failed = TabSetCloseStatus::Failed {
        reason: TabSetCloseFailure::ChildBlocked,
    };
    settle(&mut state, &read, &available(&start, failed, stopped));
    // The closed member stays out until the publication removes it.
    assert!(state.view().tab_leaving(&members[0]));
    assert!(!state.view().tab_leaving(&members[1]));
    assert!(!state.view().tab_leaving(&members[2]));
    let status = state.close_set_status().unwrap();
    assert_eq!(
        (
            status.completed,
            status.total,
            status.halted,
            status.unconfirmed
        ),
        (1, 3, true, true)
    );
    assert!(!state.close_set_read_due(later + Duration::from_secs(1)));
}

/// A Start the Server refused for a revision race goes again under a new identity once this
/// Client holds that revision with every member still the Tab it named, at most four times. The
/// fifth race, a member changed meanwhile or any other refusal reports that the Close did not
/// apply, and every member stays.
#[test]
fn a_start_lost_to_a_revision_race_goes_again_while_its_members_stand() {
    let now = Instant::now();
    let deadline = now + Duration::from_secs(50);
    let mut state = ready(3);
    let members = tab_ids(&state, &[1, 2]);
    let mut start = open(&mut state, &[1, 2], now);
    for revision in 2..=5 {
        settle(&mut state, &start, &race(revision));
        assert_eq!(state.notice(), None);
        assert!(members.iter().all(|tab| !state.view().tab_leaving(tab)));
        // It asks for the revision the Server named, and waits for it.
        assert!(state.publication_debt_pending());
        assert!(!state.close_set_resend_due(now));
        advance(&mut state, revision, None);
        assert!(!state.settle_close_set(now));
        let again = state
            .plan_tab_set_resend(
                workbench_request_id(9000 + u128::from(revision)),
                deadline,
                now,
            )
            .unwrap();
        state.begin_close_exchange(&again, now).unwrap();
        let (
            CloseOperationKind::TabSet(TabSetCloseAction::Start {
                operation_id: before,
                ..
            }),
            CloseOperationKind::TabSet(TabSetCloseAction::Start {
                operation_id,
                tab_ids,
                expected_revision,
                ..
            }),
        ) = (start.kind(), again.kind())
        else {
            panic!("set Starts");
        };
        assert_ne!(operation_id, before);
        assert_eq!(tab_ids, &members);
        assert_eq!(*expected_revision, WorkbenchRevision::new(revision));
        start = again;
    }
    settle(&mut state, &start, &race(6));
    advance(&mut state, 6, None);
    assert!(!state.close_set_resend_due(now));
    assert!(state.settle_close_set(now));
    assert_eq!(state.notice(), Some(ClientNotice::CloseRequestNotApplied));
    assert!(state.close_set_status().is_none());

    // A member changed meanwhile.
    let mut state = ready(3);
    let start = open(&mut state, &[1, 2], now);
    settle(&mut state, &start, &race(2));
    advance(&mut state, 2, Some(2));
    assert!(!state.close_set_resend_due(now));
    assert!(state.settle_close_set(now));
    assert_eq!(state.notice(), Some(ClientNotice::CloseRequestNotApplied));

    // Any other refusal.
    let mut state = ready(3);
    let members = tab_ids(&state, &[1, 2]);
    let start = open(&mut state, &[1, 2], now);
    settle(
        &mut state,
        &start,
        &CloseResult::TabSet(TabSetCloseOutcome::Rejected {
            revision: WorkbenchRevision::new(1),
            reason: TabSetCloseRejection::TabUnavailable,
        }),
    );
    assert_eq!(state.notice(), Some(ClientNotice::CloseRequestNotApplied));
    assert!(state.close_set_status().is_none());
    assert!(!state.close_set_resend_due(now));
    assert!(members.iter().all(|tab| !state.view().tab_leaving(tab)));
}

/// A set of one Tab is no set: the Tab's own Close carries it and reports every failure at once,
/// as the key's does, and no count appears. A refusal keeps the Tab with its notice.
#[test]
fn a_set_of_one_tab_closes_and_reports_as_its_own_close() {
    use crate::localization::{CloseStatusText, Locale};
    let failures = [
        WorkbenchTabCloseStatus::CleanupBlocked {
            reason: WorkbenchTabCloseBlockedReason::HostRefused,
            resumable: false,
        },
        WorkbenchTabCloseStatus::OutcomeUnknown {
            phase: WorkbenchTabCloseUnknownPhase::Dispose,
            reason: WorkbenchTabCloseUnknownReason::TransportLost,
        },
        WorkbenchTabCloseStatus::DisposedAwaitingPublication {
            reason: WorkbenchTabClosePublicationReason::PreparationFailed,
            resumable: false,
        },
    ];
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for failure in &failures {
            let now = Instant::now();
            let mut state = ready(3);
            let close = open(&mut state, &[1], now);
            assert_eq!(close.kind(), &CloseOperationKind::Close);
            accept(&mut state, &close);
            read_status(&mut state, &close, failure.clone(), now);
            let status = state.view().announced_close_status().unwrap();
            let told = locale.native_close_status(CloseStatusText {
                label: status.target_label(),
                kind: status.status(),
                stopped: status.stopped(),
            });
            let shown = screen(&state, locale);
            assert!(shown.contains(&told), "{shown}");
            assert!(state.close_set_status().is_none());
        }

        // An exchange whose result was lost, with no read to tell.
        let now = Instant::now();
        let mut state = ready(3);
        let close = open(&mut state, &[1], now);
        state
            .settle_close_exchange(&close, &CloseResult::Ambiguous)
            .unwrap();
        assert!(state.view().announced_close_status().unwrap().unavailable());
        assert!(state.close_set_status().is_none());
    }

    let now = Instant::now();
    let mut state = ready(3);
    let close = open(&mut state, &[1], now);
    state
        .settle_close_exchange(
            &close,
            &CloseResult::Precommit(
                crate::local_client::workbench::WorkbenchTabClosePrecommit::Rejected {
                    revision: WorkbenchRevision::new(2),
                    reason: orchbee_workbench_protocol::WorkbenchRejectionReason::RevisionMismatch,
                },
            ),
        )
        .unwrap();
    assert!(!state.has_retained_close_records());
    assert!(state.close_set_status().is_none());
    assert!(
        screen(&state, Locale::EnUs).contains(
            &Locale::EnUs
                .native_notice(crate::localization::WorkbenchClientNotice::CloseRequestNotApplied)
        )
    );
}

#[test]
fn one_tab_stays_off_the_status_line_unless_its_close_is_slow() {
    use crate::localization::Locale;
    let closing = WorkbenchTabCloseStatus::Closing {
        phase: orchbee_workbench_protocol::WorkbenchTabClosePhase::WaitingGrace,
    };
    for (locale, word) in [(Locale::EnUs, "· Closing"), (Locale::ZhCn, "· 正在关闭")] {
        // Waiting out a shell that ignores its termination, the close grows slow.
        let now = Instant::now();
        let mut state = ready(3);
        let close = open(&mut state, &[1], now);
        accept(&mut state, &close);
        read_status(&mut state, &close, closing.clone(), now);
        assert_eq!(
            state.close_pending_deadline(),
            Some(now + Duration::from_secs(1))
        );
        assert!(!state.settle_close_pending(now + Duration::from_millis(999)));
        assert!(state.view().announced_close_status().is_none());
        assert!(state.view().close_set_status().is_none());
        assert!(!screen(&state, locale).contains(word));
        // A Client control request still finds the close in flight.
        assert!(state.view().close_status().is_some());
        assert!(state.settle_close_pending(now + Duration::from_secs(1)));
        assert_eq!(state.close_pending_deadline(), None);
        assert!(screen(&state, locale).contains(word));

        // The ordinary close finishes first and leaves nothing behind.
        let now = Instant::now();
        let mut state = ready(3);
        let close = open(&mut state, &[1], now);
        let soon = now + Duration::from_millis(700);
        observe_closed(&mut state, &close, soon);
        assert!(state.view().close_set_status().is_none());
        let source = removed(&state, close.key().target().tab_id());
        install(&mut state, &close, source, soon);
        state.settle_close_set(soon);
        assert!(!state.has_retained_close_records());
        assert!(!state.settle_close_pending(now + Duration::from_secs(60)));
        assert!(state.view().announced_close_status().is_none());
        assert!(state.view().close_set_status().is_none());
    }
}

#[test]
fn a_selected_tab_closing_unreported_withholds_its_own_cues() {
    let now = Instant::now();
    let mut state = ready(3);
    let close = open(&mut state, &[0], now);
    assert!(state.view().close_quiet_on_selected());
    accept(&mut state, &close);
    state.settle_close_pending(now + Duration::from_secs(1));
    assert!(!state.view().close_quiet_on_selected());
}

fn rebind_same_focused_pane(state: &mut WorkbenchClientState, now: Instant) {
    let before = state.focus_record().unwrap().pane_ref();
    let panel = before.pane().panel().clone();
    let desired = state.desired().cloned();
    let mode = state.controller_mode().cloned().unwrap();
    assert!(state.sync_pane_set(&[], now));
    assert!(state.settle_pane_detached(&before));
    assert!(state.sync_pane_set(&[(panel, 0, state.latest_geometry())], now));
    let after = state.focus_record().unwrap().pane_ref();
    assert_eq!(before.pane(), after.pane());
    assert_ne!(
        before, after,
        "the real membership owner minted a new incarnation"
    );
    // Complete the record-level fixture too: an unattached record underneath Ready would
    // be an invalid input and only prove the generic membership invariant, not this fence.
    assert!(state.mark_pane_attached(&after));
    assert!(state.record_setup_configuration(&after, mode, Some(state.latest_geometry())));
    assert!(state.stage_drawn_frame(&after, FrameIdentity::new(2)));
    assert!(state.mark_feedback_accepted(&after, FrameIdentity::new(2)));
    assert!(state.invariant_holds());
    assert_eq!(state.desired(), desired.as_ref());
}

#[test]
fn close_set_same_pane_key_rebind_revokes_its_confirmation() {
    let now = Instant::now();
    let mut state = ready(3);
    let source = state.cache().canonical_snapshot().clone();
    let ids = tab_ids(&state, &[1, 2]);
    state.open_close_set(&ids).unwrap();
    let token = state.view().close_confirmation().unwrap().token();
    state.acknowledge_close_draw(Some(token), None);
    rebind_same_focused_pane(&mut state, now);
    assert!(
        state
            .plan_close_confirmation(workbench_request_id(8010), now + Duration::from_secs(50))
            .is_err()
    );
    assert!(state.settle_stale_close_confirmations());
    assert!(state.view().close_confirmation().is_none());
    assert_eq!(state.cache().canonical_snapshot(), &source);
}

#[test]
fn close_set_unseen_cancelled_resized_and_stale_prompts_never_admit() {
    for change in 0..4 {
        let now = Instant::now();
        let mut state = ready(3);
        let ids = vec![
            state.cache().tabs()[1].tab_id().clone(),
            state.cache().tabs()[2].tab_id().clone(),
        ];
        state.open_close_set(&ids).unwrap();
        let token = state.view().close_confirmation().unwrap().token();
        state.acknowledge_close_draw(Some(token), None);
        match change {
            0 => state.cancel_close_confirmation(),
            1 => state.invalidate_close_display(),
            2 => {
                let _ = state.settle_close_confirmation_not_presented();
            }
            3 => {
                let mut peer = state.cache().canonical_snapshot().clone();
                peer.revision = WorkbenchRevision::new(2);
                state.cache = state.cache.project_snapshot(&peer).unwrap();
                assert!(state.settle_stale_close_confirmations());
            }
            _ => unreachable!(),
        }
        assert!(
            state
                .plan_close_confirmation(workbench_request_id(8000), now + Duration::from_secs(50))
                .is_err()
        );
        assert!(!state.has_retained_close_records());
        assert!(state.close_set_status().is_none());
    }
}
