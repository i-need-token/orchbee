//! A new explicit operation may replace only a hidden known success, after its
//! entire source/choice/token admission succeeds. Recovery facts survive refusal.
use super::*;

fn hidden_opened() -> Launcher {
    let (mut launcher, origin) = fixture(1, WorktreeMode::Create);
    let plan = launcher
        .create_with_defaults(request(90), &origin, Locale::EnUs)
        .unwrap();
    assert!(
        launcher.start_settled(
            &plan.operation_id,
            Ok(WorkbenchWorktreeOutcome::Available {
                project_id: plan.project,
                attempt: 1,
                status: Box::new(WorktreeOperationStatus::Opened {
                    checkout: WorktreeCheckoutFacts {
                        repository_root: path("C:/repo"),
                        common_directory: path("C:/repo/.git"),
                        path: path("C:/repo"),
                        branch: None,
                        dirty: false,
                    },
                    workspace_id: orchbee_workbench_protocol::WorkbenchWorkspaceId::parse(
                        &format!("{:032x}", 80)
                    )
                    .unwrap(),
                    reused: false,
                    terminal_outcome: None,
                }),
            })
        )
    );
    assert_eq!(launcher.notice(), WorktreeNotice::Opened);
    launcher.dismiss();
    assert!(launcher.hidden_opened_result());
    launcher
}

#[test]
fn hidden_worktree_replacement_preserves_recovery_on_invalid_source_choice_or_token() {
    for change in 0..10 {
        let mut launcher = hidden_opened();
        let (mut origin, mut sidebar) = sidebar_fixture();
        let mut mode = WorktreeMode::Open;
        let mut checkout = Some(path("C:/trees/猫"));
        match change {
            0 => checkout = Some(path("C:/not-listed")),
            1 => sidebar.discovery.entries[1].checkout.bare = true,
            2 => {
                sidebar.discovery.entries[1].checkout.path =
                    WorktreeEntryText::Utf8("relative/path".into());
            }
            3 => checkout = None,
            4 => mode = WorktreeMode::Create,
            5 | 6 => {
                let mut current = origin.cache.canonical_snapshot().clone();
                if change == 5 {
                    current.navigation.workspace_order.reverse();
                } else {
                    current.terminal_contexts[1].tabs.clear();
                }
                origin.cache = project_preferred_workspace(&current).unwrap();
            }
            7 => {
                sidebar.discovery.worker = WorkerId::parse(&format!("{:032x}", 9999)).unwrap();
            }
            8 => sidebar.observed = None,
            9 => launcher.last_token = u64::MAX,
            _ => unreachable!(),
        }
        let old = launcher.operation().unwrap().clone();
        let token = launcher.last_token;
        assert!(
            !launcher.open_sidebar(origin.clone(), sidebar, mode, checkout.as_ref()),
            "invalid replacement {change} must refuse before retiring the receipt"
        );
        assert!(launcher.hidden_opened_result(), "retained success {change}");
        assert_eq!(launcher.last_token, token);
        let retained = launcher.operation().unwrap();
        assert_eq!(retained.plan.operation_id, old.plan.operation_id);
        assert_eq!(retained.outcome, old.outcome);
        assert!(retained.focus_allowed);
        assert!(
            launcher
                .confirm(WorktreeToken(token), request(91), &origin)
                .is_none()
        );
        assert!(launcher.reopen(), "recovery remains available for {change}");
        assert_eq!(launcher.operation().unwrap().outcome, old.outcome);
        assert!(!launcher.sidebar_open_ready());
    }
}

#[test]
fn visible_uncertain_nonterminal_and_focus_retired_results_never_yield_to_a_sidebar_draft() {
    for change in 0..13 {
        let mut launcher = hidden_opened();
        match change {
            0 => assert!(launcher.reopen()),
            1 => launcher.suppress_focus(),
            2 => launcher.entry_notice(WorktreeNotice::Unknown),
            3 => launcher.entry_notice(WorktreeNotice::NotSubmitted),
            4 => launcher.entry_notice(WorktreeNotice::Failed),
            5 => launcher.entry_notice(WorktreeNotice::SourceChanged),
            6 => launcher.entry_notice(WorktreeNotice::EntryDeferred),
            7 => launcher.entry_notice(WorktreeNotice::EntryFailed),
            8 => {
                let Phase::Result(operation) = &mut launcher.phase else {
                    unreachable!()
                };
                operation.outcome = None;
            }
            9 => {
                let Phase::Result(operation) = &mut launcher.phase else {
                    unreachable!()
                };
                operation.outcome = Some(pending(operation.plan.project.clone()));
            }
            10 => {
                let Phase::Result(operation) = &mut launcher.phase else {
                    unreachable!()
                };
                let Some(WorkbenchWorktreeOutcome::Available { project_id, .. }) =
                    &mut operation.outcome
                else {
                    unreachable!()
                };
                *project_id = Some(WorkbenchProjectId::parse(&format!("{:032x}", 9999)).unwrap());
            }
            11 => {
                let Phase::Result(operation) = std::mem::replace(&mut launcher.phase, Phase::Idle)
                else {
                    unreachable!()
                };
                launcher.phase = Phase::Observing(operation);
            }
            12 => {
                launcher.begin_read().unwrap();
                assert!(launcher.observed(&request(90), Err(true)));
            }
            _ => unreachable!(),
        }
        let old = launcher.operation().unwrap().clone();
        let terminal = launcher.terminal_result();
        let notice = launcher.notice();
        let visible = launcher.visible();
        let token = launcher.last_token;
        assert!(!launcher.hidden_opened_result(), "not replaceable {change}");
        let (origin, sidebar) = sidebar_fixture();
        assert!(!launcher.open_sidebar(origin, sidebar, WorktreeMode::Create, None));
        let retained = launcher.operation().unwrap();
        assert_eq!(retained.plan.operation_id, old.plan.operation_id);
        assert_eq!(retained.outcome, old.outcome);
        assert_eq!(retained.focus_allowed, old.focus_allowed);
        assert_eq!(launcher.terminal_result(), terminal);
        assert_eq!(launcher.notice(), notice);
        assert_eq!(launcher.visible(), visible);
        assert_eq!(launcher.last_token, token);
    }
    let (mut starting, origin) = fixture(1, WorktreeMode::Create);
    let plan = starting
        .create_with_defaults(request(90), &origin, Locale::EnUs)
        .unwrap();
    starting.dismiss();
    let (next_origin, sidebar) = sidebar_fixture();
    assert!(!starting.open_sidebar(next_origin, sidebar, WorktreeMode::Create, None));
    assert!(
        matches!(&starting.phase, Phase::Starting(retained) if retained.operation_id == plan.operation_id)
    );
}
