use super::*;
use crate::terminal_session::state::{
    OperationDeadlines,
    tests::{ready_with_cache, snapshot},
};
use crate::terminal_session::workbench_projection::project_native_standalone_tabs;
use orchbee_workbench_protocol::{
    Project, WorkbenchRevision, WorktreeCheckoutFacts, WorktreeEntry, WorktreeEntryText,
    WorktreeGitEffect, WorktreeProgress, WorktreeQuerySource, WorktreeReference,
    WorktreeWorkspaceDisposition,
};
use std::time::Instant;

fn request(n: u128) -> WorkbenchRequestId {
    WorkbenchRequestId::parse(&format!("{n:032x}")).unwrap()
}
fn path(s: &str) -> WorkbenchDirectoryPath {
    WorkbenchDirectoryPath::parse(s).unwrap()
}
pub(in crate::terminal_session) fn fixture(
    projects: usize,
    mode: WorktreeMode,
) -> (Launcher, Origin) {
    fixture_with_discovery(projects, mode, discovery())
}
fn fixture_with_discovery(
    projects: usize,
    mode: WorktreeMode,
    discovery: Discovery,
) -> (Launcher, Origin) {
    let snapshot = canonical(projects, 1);
    let state = ready_with_cache(project_native_standalone_tabs(&snapshot).unwrap());
    let p = state
        .worktree_preparation(
            mode,
            1,
            WorktreeToken(0),
            OperationDeadlines::from_sample(Instant::now()).unwrap(),
        )
        .unwrap();
    let origin = p.origin.clone();
    let mut launcher = Launcher::default();
    assert!(launcher.open(p));
    let p = launcher.preparation().unwrap().clone();
    let source = WorktreeSource::Directory {
        path: path("C:/repo"),
    };
    assert!(launcher.prepared(&p, Some(source)));
    let token = launcher.token().unwrap();
    assert!(launcher.discovered(token, Ok(discovery)));
    (launcher, origin)
}
pub(in crate::terminal_session) fn canonical(projects: usize, revision: u64) -> Snapshot {
    let mut snapshot = snapshot(1, revision, None);
    for n in 0..projects {
        snapshot.navigation.projects.push(Project {
            project_id: WorkbenchProjectId::parse(&format!("{:032x}", 70 + n)).unwrap(),
            label: "duplicate".into(),
            directories: Vec::new(),
            workspaces: Vec::new(),
        });
    }
    snapshot
}
pub(in crate::terminal_session) fn discovery() -> Discovery {
    Discovery {
        worker: orchbee_workbench_protocol::WorkerId::parse(&format!("{:032x}", 1003)).unwrap(),
        source: WorktreeQuerySource {
            requested_path: path("C:/repo"),
            repository_root: path("C:/repo"),
            common_directory: path("C:/repo/.git"),
            checkout_path: path("C:/repo"),
            workspace_id: None,
        },
        configured_root: None,
        entries: ["C:/repo", "C:/trees/猫"]
            .into_iter()
            .enumerate()
            .map(|(n, p)| WorktreeQueryEntry {
                linked: n > 0,
                workspaces: Vec::new(),
                checkout: WorktreeEntry {
                    path: WorktreeEntryText::Utf8(p.into()),
                    head: Some("a".repeat(40)),
                    branch: Some(WorktreeEntryText::Utf8("refs/heads/same".into())),
                    bare: false,
                    detached: false,
                    locked: false,
                    prunable: false,
                },
            })
            .collect(),
    }
}
fn edit(l: &mut Launcher, e: WorktreeEdit<'_>) {
    assert!(l.edit(l.token().unwrap(), e));
}
pub(in crate::terminal_session) fn confirm(l: &mut Launcher, origin: &Origin) -> Option<Plan> {
    let token = l.token().unwrap();
    l.draw(Some(token));
    l.confirm(
        token,
        request(90),
        origin,
        WorkbenchLabel::parse("Default").unwrap(),
    )
}
fn pending(project: WorkbenchProjectId) -> WorkbenchWorktreeOutcome {
    WorkbenchWorktreeOutcome::Available {
        project_id: project,
        attempt: 1,
        status: Box::new(WorktreeOperationStatus::Pending {
            phase: orchbee_workbench_protocol::WorktreeOperationPhase::ObservingSource,
            progress: WorktreeProgress {
                path: None,
                checkout: None,
                workspace: WorktreeWorkspaceDisposition::Unassociated {},
                git_effect: WorktreeGitEffect::None,
                restoration: None,
            },
        }),
    }
}
#[test]
fn worktree_confirmation_requires_fresh_draw_complete_packet_and_immutable_origin() {
    let (mut l, origin) = fixture(1, WorktreeMode::Create);
    edit(&mut l, WorktreeEdit::Append("feature/猫"));
    let stale = l.token().unwrap();
    assert!(
        l.confirm(
            stale,
            request(90),
            &origin,
            WorkbenchLabel::parse("Default").unwrap()
        )
        .is_none()
    );
    l.draw(Some(stale));
    edit(&mut l, WorktreeEdit::NextField);
    assert!(
        l.confirm(
            stale,
            request(90),
            &origin,
            WorkbenchLabel::parse("Default").unwrap()
        )
        .is_none()
    );
    edit(&mut l, WorktreeEdit::Completeness(false));
    assert!(confirm(&mut l, &origin).is_none());
    edit(&mut l, WorktreeEdit::Completeness(true));
    let mut changed = origin.clone();
    changed.zoom = !changed.zoom;
    assert!(confirm(&mut l, &changed).is_none());
    assert_eq!(l.notice(), WorktreeNotice::SourceChanged);
}
#[test]
fn worktree_exact_checkout_path_survives_duplicate_branch_labels() {
    let (mut l, origin) = fixture(2, WorktreeMode::Open);
    assert_eq!(l.view().unwrap().stage(), WorktreeStage::Project);
    edit(&mut l, WorktreeEdit::Next);
    assert!(confirm(&mut l, &origin).is_none());
    edit(&mut l, WorktreeEdit::Next);
    let plan = confirm(&mut l, &origin).unwrap();
    assert_eq!(
        plan.project,
        Some(origin.cache.projects()[1].project_id.clone())
    );
    assert!(
        matches!(plan.intent, WorktreeIntent::Open { selector: orchbee_workbench_protocol::WorktreeSelector::Path { path: p }, .. } if p == path("C:/trees/猫"))
    );
}
#[test]
fn worktree_reference_utf8_limit_is_atomic_and_base_empty_preserves_head_default() {
    let (mut l, origin) = fixture(1, WorktreeMode::Create);
    let branch = format!("{}a", "猫".repeat(341));
    assert_eq!(branch.len(), 1024);
    edit(&mut l, WorktreeEdit::Paste(&branch));
    edit(&mut l, WorktreeEdit::Append("z"));
    assert_eq!(l.notice(), WorktreeNotice::InputLimit);
    assert_eq!(l.view().unwrap().branch(), branch);
    edit(&mut l, WorktreeEdit::NextField);
    edit(&mut l, WorktreeEdit::Clear);
    let plan = confirm(&mut l, &origin).unwrap();
    assert!(matches!(
        plan.intent,
        WorktreeIntent::Create { base: None, .. }
    ));
}
#[test]
fn worktree_cancellation_and_foreign_late_discovery_never_issue_a_plan() {
    let (mut l, origin) = fixture(1, WorktreeMode::Create);
    let old = l.token().unwrap();
    l.dismiss();
    assert!(!l.discovered(old, Ok(discovery())));
    assert!(l.operation().is_none());
    assert!(!l.visible());
    assert!(
        l.confirm(
            old,
            request(91),
            &origin,
            WorkbenchLabel::parse("Default").unwrap()
        )
        .is_none()
    );
}
#[test]
fn worktree_default_project_receipt_is_retained_before_failed_snapshot_or_hidden_continuation() {
    for hide in [false, true] {
        let (mut l, origin) = fixture(0, WorktreeMode::Create);
        edit(&mut l, WorktreeEdit::Append("feature/one"));
        let plan = confirm(&mut l, &origin).unwrap();
        assert!(plan.project.is_none());
        if hide {
            l.dismiss();
        }
        let project = WorkbenchProjectId::parse(&format!("{:032x}", 79)).unwrap();
        assert!(
            l.project_settled(
                &plan.operation_id,
                Some(project.clone()),
                None,
                Some(&origin),
                false
            )
            .is_none()
        );
        assert_eq!(l.retained_project, Some(project));
        assert_eq!(l.notice(), WorktreeNotice::ProjectRetained);
        assert!(l.operation().unwrap().outcome.is_none());
    }
}
#[test]
fn worktree_unknown_start_only_accepts_correlated_reads_and_never_reconfirms() {
    let (mut l, origin) = fixture(1, WorktreeMode::Create);
    edit(&mut l, WorktreeEdit::Append("feature/one"));
    let plan = confirm(&mut l, &origin).unwrap();
    assert!(l.start_settled(&plan.operation_id, Err(true)));
    assert_eq!(l.notice(), WorktreeNotice::Unknown);
    let token = l.token().unwrap();
    l.draw(Some(token));
    assert!(
        l.confirm(
            token,
            request(92),
            &origin,
            WorkbenchLabel::parse("Default").unwrap()
        )
        .is_none()
    );
    assert!(!l.observed(&request(91), Ok(pending(plan.project.clone().unwrap()))));
    l.dismiss();
    assert!(l.reopen());
    assert!(!l.operation().unwrap().focus_allowed);
    assert!(l.observed(&plan.operation_id, Ok(pending(plan.project.unwrap()))));
    assert!(l.pending());
}
#[test]
fn worktree_opened_keeps_canonical_receipt_after_source_or_entry_failure() {
    let (mut l, origin) = fixture(1, WorktreeMode::Open);
    let plan = confirm(&mut l, &origin).unwrap();
    let opened = WorkbenchWorktreeOutcome::Available {
        project_id: plan.project.clone().unwrap(),
        attempt: 1,
        status: Box::new(WorktreeOperationStatus::Opened {
            checkout: WorktreeCheckoutFacts {
                repository_root: path("C:/foreign"),
                common_directory: path("C:/foreign/.git"),
                path: path("C:/trees/猫"),
                branch: None,
                dirty: false,
            },
            workspace_id: orchbee_workbench_protocol::WorkbenchWorkspaceId::parse(&format!(
                "{:032x}",
                80
            ))
            .unwrap(),
            reused: true,
            terminal_outcome: None,
        }),
    };
    assert!(l.start_settled(&plan.operation_id, Ok(opened)));
    assert_eq!(l.notice(), WorktreeNotice::SourceChanged);
    assert!(!l.operation().unwrap().focus_allowed);
    l.entry_notice(WorktreeNotice::EntryFailed);
    assert_eq!(
        l.view().unwrap().workspace(),
        Some(format!("{:032x}", 80).as_str())
    );
}
#[test]
fn worktree_default_project_success_uses_fresh_revision_and_does_not_repeat_project() {
    let (mut l, origin) = fixture(0, WorktreeMode::Create);
    edit(&mut l, WorktreeEdit::Append("feature/one"));
    let plan = confirm(&mut l, &origin).unwrap();
    let mut fresh = snapshot(1, 2, None);
    let project = WorkbenchProjectId::parse(&format!("{:032x}", 79)).unwrap();
    fresh.navigation.projects.push(Project {
        project_id: project.clone(),
        label: "Default".into(),
        directories: Vec::new(),
        workspaces: Vec::new(),
    });
    let next = l
        .project_settled(
            &plan.operation_id,
            Some(project.clone()),
            Some(&fresh),
            Some(&origin),
            false,
        )
        .unwrap();
    assert_eq!(next.project, Some(project));
    assert_eq!(next.origin.cache.revision(), WorkbenchRevision::new(2));
    assert!(
        l.project_settled(&plan.operation_id, None, Some(&fresh), Some(&origin), false)
            .is_none()
    );
}

#[test]
fn hiding_during_start_permanently_revokes_focus_even_after_reopen() {
    let (mut l, origin) = fixture(1, WorktreeMode::Open);
    let plan = confirm(&mut l, &origin).unwrap();
    l.dismiss();
    assert!(l.reopen());
    assert!(l.start_settled(&plan.operation_id, Ok(pending(plan.project.unwrap()))));
    assert!(!l.operation().unwrap().focus_allowed);
}
#[test]
fn hiding_during_default_commit_retains_project_without_start_after_reopen() {
    let (mut l, origin) = fixture(0, WorktreeMode::Open);
    let plan = confirm(&mut l, &origin).unwrap();
    l.dismiss();
    assert!(l.reopen());
    let fresh = canonical(1, 2);
    let project = fresh.navigation.projects[0].project_id.clone();
    assert!(
        l.project_settled(
            &plan.operation_id,
            Some(project.clone()),
            Some(&fresh),
            Some(&origin),
            false
        )
        .is_none()
    );
    assert_eq!(l.retained_project, Some(project));
    assert_eq!(l.notice(), WorktreeNotice::ProjectRetained);
}
#[test]
fn completing_an_inflight_packet_requires_another_presented_draw() {
    let (mut l, origin) = fixture(1, WorktreeMode::Open);
    let token = l.token().unwrap();
    l.draw(Some(token));
    edit(&mut l, WorktreeEdit::Completeness(false));
    edit(&mut l, WorktreeEdit::Completeness(true));
    assert!(
        l.confirm(
            token,
            request(90),
            &origin,
            WorkbenchLabel::parse("Default").unwrap()
        )
        .is_none()
    );
    assert!(confirm(&mut l, &origin).is_some());
}
#[test]
fn read_of_a_known_terminal_result_does_not_regain_focus_or_confirm_again() {
    let (mut l, origin) = fixture(1, WorktreeMode::Open);
    let plan = confirm(&mut l, &origin).unwrap();
    assert!(l.start_settled(
        &plan.operation_id,
        Ok(WorkbenchWorktreeOutcome::NotFound {})
    ));
    assert!(l.terminal_result());
    assert_eq!(l.begin_read().unwrap().operation_id, plan.operation_id);
    assert!(!l.operation().unwrap().focus_allowed);
    let token = l.token().unwrap();
    l.draw(Some(token));
    assert!(
        l.confirm(
            token,
            request(91),
            &origin,
            WorkbenchLabel::parse("Default").unwrap()
        )
        .is_none()
    );
}

#[test]
fn an_expired_read_keeps_the_known_opened_workspace_receipt() {
    let (mut l, origin) = fixture(1, WorktreeMode::Open);
    let plan = confirm(&mut l, &origin).unwrap();
    let workspace =
        orchbee_workbench_protocol::WorkbenchWorkspaceId::parse(&format!("{:032x}", 80)).unwrap();
    let opened = WorkbenchWorktreeOutcome::Available {
        project_id: plan.project.clone().unwrap(),
        attempt: 1,
        status: Box::new(WorktreeOperationStatus::Opened {
            checkout: WorktreeCheckoutFacts {
                repository_root: path("C:/repo"),
                common_directory: path("C:/repo/.git"),
                path: path("C:/repo"),
                branch: None,
                dirty: false,
            },
            workspace_id: workspace.clone(),
            reused: true,
            terminal_outcome: None,
        }),
    };
    assert!(l.start_settled(&plan.operation_id, Ok(opened)));
    assert_eq!(l.notice(), WorktreeNotice::Opened);
    assert!(l.begin_read().is_some());
    assert!(l.observed(
        &plan.operation_id,
        Ok(WorkbenchWorktreeOutcome::NotFound {})
    ));
    assert_eq!(l.notice(), WorktreeNotice::Unknown);
    assert_eq!(l.view().unwrap().workspace(), Some(workspace.as_str()));
}

#[test]
fn configured_worktree_create_consumes_canonical_worker_receipt_not_operand_spelling() {
    for root in [
        r"C:\trees",
        r"\\?\C:\trees",
        "c:/Trees/",
        "C:/trees/../trees",
    ] {
        let mut listing = discovery();
        listing.configured_root = Some(path(root));
        let (mut l, origin) = fixture_with_discovery(1, WorktreeMode::Create, listing);
        edit(&mut l, WorktreeEdit::Append("feature/one"));
        let plan = confirm(&mut l, &origin).unwrap();
        assert!(matches!(
            &plan.intent,
            WorktreeIntent::Create { path: Some(_), .. }
        ));
        let opened = WorkbenchWorktreeOutcome::Available {
            project_id: plan.project.clone().unwrap(),
            attempt: 1,
            status: Box::new(WorktreeOperationStatus::Opened {
                checkout: WorktreeCheckoutFacts {
                    repository_root: path("C:/repo"),
                    common_directory: path("C:/repo/.git"),
                    path: path("C:/trees/repo/feature-one"),
                    branch: Some(WorktreeReference::parse("feature/one").unwrap()),
                    dirty: false,
                },
                workspace_id: orchbee_workbench_protocol::WorkbenchWorkspaceId::parse(&format!(
                    "{:032x}",
                    80
                ))
                .unwrap(),
                reused: false,
                terminal_outcome: None,
            }),
        };
        assert!(l.start_settled(&plan.operation_id, Ok(opened)), "{root}");
        assert_eq!(l.notice(), WorktreeNotice::Opened, "{root}");
        assert!(l.operation().unwrap().focus_allowed, "{root}");
    }
}

#[test]
fn worktree_open_rejects_a_different_canonical_checkout_in_the_same_repository() {
    let (mut l, origin) = fixture(1, WorktreeMode::Open);
    let plan = confirm(&mut l, &origin).unwrap();
    let opened = WorkbenchWorktreeOutcome::Available {
        project_id: plan.project.clone().unwrap(),
        attempt: 1,
        status: Box::new(WorktreeOperationStatus::Opened {
            checkout: WorktreeCheckoutFacts {
                repository_root: path("C:/repo"),
                common_directory: path("C:/repo/.git"),
                path: path("C:/trees/猫"),
                branch: None,
                dirty: false,
            },
            workspace_id: orchbee_workbench_protocol::WorkbenchWorkspaceId::parse(&format!(
                "{:032x}",
                80
            ))
            .unwrap(),
            reused: true,
            terminal_outcome: None,
        }),
    };
    assert!(l.start_settled(&plan.operation_id, Ok(opened)));
    assert_eq!(l.notice(), WorktreeNotice::SourceChanged);
    assert!(!l.operation().unwrap().focus_allowed);
}
