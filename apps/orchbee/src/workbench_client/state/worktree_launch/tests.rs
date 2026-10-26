use super::*;
use crate::localization::Locale;
use crate::workbench_client::state::{
    OperationDeadlines,
    tests::{ready_with_cache, snapshot},
};
use crate::workbench_client::workbench_projection::project_preferred_workspace;
use orchbee_workbench_protocol::{
    Project, Snapshot, WorktreeCheckoutFacts, WorktreeEntry, WorktreeEntryText, WorktreeGitEffect,
    WorktreeProgress, WorktreeQuerySource, WorktreeReference, WorktreeSelector,
    WorktreeWorkspaceDisposition,
};
use std::time::Instant;

pub(in crate::workbench_client) mod removal;
mod replacement;

fn request(n: u128) -> WorkbenchRequestId {
    WorkbenchRequestId::parse(&format!("{n:032x}")).unwrap()
}
fn path(s: &str) -> WorkbenchDirectoryPath {
    WorkbenchDirectoryPath::parse(s).unwrap()
}
pub(in crate::workbench_client) fn fixture(
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
    let state = ready_with_cache(project_preferred_workspace(&snapshot).unwrap());
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
    let token = launcher.token().unwrap();
    launcher.draw(Some(token));
    (launcher, origin)
}
pub(in crate::workbench_client) fn canonical(projects: usize, revision: u64) -> Snapshot {
    let mut snapshot = snapshot(1, revision, None);
    for n in 0..projects {
        snapshot.navigation.projects.push(Project {
            project_id: WorkbenchProjectId::parse(&format!("{:032x}", 70 + n)).unwrap(),
            label: "duplicate".into(),
        });
    }
    snapshot
}
pub(in crate::workbench_client) fn discovery() -> Discovery {
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
pub(in crate::workbench_client) fn confirm(l: &mut Launcher, origin: &Origin) -> Option<Plan> {
    let token = l.token().unwrap();
    l.draw(Some(token));
    l.confirm(token, request(90), origin)
}
fn pending(project: Option<WorkbenchProjectId>) -> WorkbenchWorktreeOutcome {
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
    assert!(l.confirm(stale, request(90), &origin,).is_none());
    l.draw(Some(stale));
    edit(&mut l, WorktreeEdit::NextField);
    assert!(l.confirm(stale, request(90), &origin,).is_none());
    l.draw(l.token());
    edit(&mut l, WorktreeEdit::Completeness(false));
    assert!(confirm(&mut l, &origin).is_none());
    edit(&mut l, WorktreeEdit::Completeness(true));
    let mut changed = origin.clone();
    changed.zoom = !changed.zoom;
    assert!(confirm(&mut l, &changed).is_none());
    assert_eq!(l.notice(), WorktreeNotice::SourceChanged);
}
#[test]
fn worktree_exact_checkout_path_survives_duplicate_branch_labels_without_project_picker() {
    let (mut l, origin) = fixture(2, WorktreeMode::Open);
    assert_eq!(l.view().unwrap().stage(), WorktreeStage::Checkout);
    edit(&mut l, WorktreeEdit::Next);
    let plan = confirm(&mut l, &origin).unwrap();
    assert!(plan.project.is_none());
    assert!(
        matches!(plan.intent, WorktreeIntent::Open { selector: WorktreeSelector::Path { path: p }, .. } if p == path("C:/trees/猫"))
    );
}

pub(in crate::workbench_client) fn sidebar_fixture() -> (Origin, addressed::SidebarDiscovery) {
    let snapshot = addressed::tests::fixture();
    let state = ready_with_cache(project_preferred_workspace(&snapshot).unwrap());
    let origin = state.worktree_origin().unwrap();
    let addressed = addressed::AddressedWorktreeSource::capture(
        &snapshot,
        &addressed::SidebarWorktreeTarget::Project(
            snapshot.navigation.projects[0].project_id.clone(),
        ),
    )
    .unwrap();
    let mut discovery = discovery();
    discovery.worker = addressed.primary.as_ref().unwrap().worker_id().clone();
    let sidebar = addressed::SidebarDiscovery {
        addressed,
        sample: crate::workbench_client::primary_directory::RootDirectory::Available(Some(path(
            "C:/repo",
        ))),
        source: WorktreeSource::Directory {
            path: path("C:/repo"),
        },
        discovery,
    };
    assert!(sidebar.valid(&snapshot));
    (origin, sidebar)
}

fn start_request(plan: &Plan) -> orchbee_workbench_protocol::WorkbenchWorktreeRequest {
    orchbee_workbench_protocol::WorkbenchWorktreeRequest {
        protocol_version: orchbee_workbench_protocol::ProtocolVersion,
        request_id: request(901),
        site_id: plan.origin.cache.site_id().clone(),
        action: orchbee_workbench_protocol::WorkbenchWorktreeAction::Start {
            operation_id: plan.operation_id.clone(),
            project_id: plan.project.clone(),
            intent: plan.intent.clone(),
        },
    }
}

fn assert_start_roundtrip(plan: &Plan) {
    use orchbee_workbench_protocol::{
        WorkbenchRequest, read_workbench_request, write_workbench_request,
    };
    let start = start_request(plan);
    assert!(start.validate().is_ok());
    let request = WorkbenchRequest::WorktreeOperation(start);
    let mut wire = Vec::new();
    write_workbench_request(&mut wire, &request).unwrap();
    assert_eq!(
        read_workbench_request(&mut wire.as_slice()).unwrap(),
        request
    );
}

#[test]
fn grouped_absent_cwd_and_empty_workspace_fallbacks_keep_required_null_project_at_the_wire_boundary()
 {
    use crate::workbench_client::primary_directory::RootDirectory;
    use orchbee_workbench_protocol::{
        WorkbenchRequest, WorkbenchWorktreeAction, write_workbench_request,
    };
    for empty in [false, true] {
        for mode in [WorktreeMode::Create, WorktreeMode::Open] {
            let mut snapshot = addressed::tests::fixture();
            if empty {
                snapshot.terminal_contexts[1].tabs.clear();
            }
            let origin = ready_with_cache(project_preferred_workspace(&snapshot).unwrap())
                .worktree_origin()
                .unwrap();
            let addressed = addressed::AddressedWorktreeSource::capture(
                &snapshot,
                &addressed::SidebarWorktreeTarget::Project(
                    snapshot.navigation.projects[0].project_id.clone(),
                ),
            )
            .unwrap();
            assert_eq!(addressed.member.is_none(), empty);
            let sample = RootDirectory::Available(None);
            let source = addressed.resolve_sample(sample.clone()).unwrap();
            let WorktreeSource::Workspace { workspace_id } = &source else {
                panic!("same-Workspace fallback");
            };
            let directory = addressed.cache.directory().unwrap();
            let mut discovery = discovery();
            discovery.worker = directory.worker_id.clone();
            discovery.source = WorktreeQuerySource {
                requested_path: directory.path.clone(),
                repository_root: directory.path.clone(),
                common_directory: path(&format!("{}/.git", directory.path.as_str())),
                checkout_path: directory.path.clone(),
                workspace_id: Some(workspace_id.clone()),
            };
            let expected_project = addressed.cache.project_id().unwrap().clone();
            let sidebar = addressed::SidebarDiscovery {
                addressed,
                sample,
                source,
                discovery,
            };
            assert!(sidebar.valid(&snapshot));
            let mut launcher = Launcher::default();
            let checkout = path("C:/trees/猫");
            assert!(launcher.open_sidebar(
                origin.clone(),
                sidebar,
                mode,
                (mode == WorktreeMode::Open).then_some(&checkout)
            ));
            let token = launcher.token().unwrap();
            launcher.draw(Some(token));
            let plan = match mode {
                WorktreeMode::Create => {
                    launcher.create_with_defaults(request(90), &origin, Locale::EnUs)
                }
                WorktreeMode::Open => launcher.confirm(token, request(90), &origin),
                WorktreeMode::Remove => unreachable!("this witness only iterates launch modes"),
            }
            .unwrap();
            assert!(matches!(plan.source, WorktreeSource::Workspace { .. }));
            assert!(
                plan.project.is_none(),
                "Workspace sources keep Server-owned Alongside grouping"
            );
            assert_start_roundtrip(&plan);
            let mut invalid = start_request(&plan);
            let WorkbenchWorktreeAction::Start { project_id, .. } = &mut invalid.action else {
                unreachable!()
            };
            *project_id = Some(expected_project);
            assert!(
                invalid.validate().is_err(),
                "explicit Project applies to Directory sources only"
            );
            let mut wire = Vec::new();
            assert!(
                write_workbench_request(&mut wire, &WorkbenchRequest::WorktreeOperation(invalid))
                    .is_err()
            );
            assert!(
                wire.is_empty(),
                "invalid intent cannot submit even a carrier header"
            );
        }
    }
}

#[test]
fn sidebar_checkout_requires_its_complete_source_draw_then_starts_the_exact_choice_once() {
    let (origin, sidebar) = sidebar_fixture();
    let mut launcher = Launcher::default();
    assert!(!launcher.open_sidebar(origin.clone(), sidebar.clone(), WorktreeMode::Open, None));
    assert!(launcher.token().is_none());
    assert!(launcher.open_sidebar(
        origin.clone(),
        sidebar.clone(),
        WorktreeMode::Open,
        Some(&path("C:/trees/猫"))
    ));
    let token = launcher.token().unwrap();
    assert!(!launcher.sidebar_open_ready());
    assert!(launcher.confirm(token, request(90), &origin).is_none());
    launcher.draw(None);
    assert!(!launcher.sidebar_open_ready());
    launcher.draw(Some(token));
    assert!(launcher.sidebar_open_ready());
    assert!(!launcher.default_create_ready());
    let plan = launcher.confirm(token, request(90), &origin).unwrap();
    assert_eq!(plan.project, sidebar.addressed.cache.project_id().cloned());
    assert_start_roundtrip(&plan);
    assert!(matches!(plan.intent, WorktreeIntent::Open {
        source: WorktreeSource::Directory { path: source },
        selector: WorktreeSelector::Path { path: checkout }, ..
    } if source == path("C:/repo") && checkout == path("C:/trees/猫")));
    assert!(plan.sidebar.is_some());
    assert!(!launcher.sidebar_open_ready());
    assert!(launcher.confirm(token, request(91), &origin).is_none());
}

#[test]
fn sidebar_checkout_blocked_or_cancelled_before_draw_cannot_authorize_a_start() {
    for cancel in [false, true] {
        let (origin, sidebar) = sidebar_fixture();
        let mut launcher = Launcher::default();
        assert!(launcher.open_sidebar(
            origin.clone(),
            sidebar,
            WorktreeMode::Open,
            Some(&path("C:/trees/猫"))
        ));
        let token = launcher.token().unwrap();
        if cancel {
            launcher.dismiss();
        } else {
            launcher.geometry_blocked();
        }
        launcher.draw(Some(token));
        assert!(!launcher.sidebar_open_ready());
        assert!(launcher.confirm(token, request(90), &origin).is_none());
        assert!(launcher.operation().is_none());
    }
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
    l.draw(l.token());
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
    assert!(l.confirm(old, request(91), &origin,).is_none());
}
#[test]
fn worktree_create_uses_defaults_without_a_project_or_second_confirmation() {
    for projects in [0, 1, 2] {
        let (mut l, origin) = fixture(projects, WorktreeMode::Create);
        let plan = l
            .create_with_defaults(request(90), &origin, Locale::EnUs)
            .unwrap();
        assert!(plan.project.is_none());
        assert!(
            matches!(plan.intent, WorktreeIntent::Create { branch: None, label: Some(ref label), .. } if label.as_str() == "Worktree 1")
        );
        assert!(l.start_settled(&plan.operation_id, Ok(pending(None))));
        assert!(l.pending());
        assert!(
            l.create_with_defaults(request(91), &origin, Locale::EnUs)
                .is_none()
        );
        assert_eq!(origin.cache.projects().len(), projects);
    }
}

#[test]
fn worktree_default_names_consider_newer_workspace_labels_across_groups() {
    use orchbee_workbench_protocol::{
        TerminalContext, WorkbenchTerminalContextId, WorkbenchWorkspaceId, Workspace,
    };
    for (locale, occupied, expected) in [
        (Locale::EnUs, "Worktree 1", "Worktree 2"),
        (Locale::ZhCn, "工作树 1", "工作树 2"),
    ] {
        for grouped in [false, true] {
            let (mut launcher, mut origin) = fixture(1, WorktreeMode::Create);
            let mut snapshot = origin.cache.canonical_snapshot().clone();
            let context = WorkbenchTerminalContextId::parse(&format!("{:032x}", 82)).unwrap();
            let workspace = WorkbenchWorkspaceId::parse(&format!("{:032x}", 81)).unwrap();
            snapshot.navigation.workspaces.push(Workspace {
                workspace_id: workspace.clone(),
                project_id: grouped.then(|| snapshot.navigation.projects[0].project_id.clone()),
                label: occupied.into(),
                primary_directory_id: None,
                terminal_context_id: context.clone(),
            });
            snapshot.navigation.workspace_order = snapshot
                .navigation
                .workspaces
                .iter()
                .map(|workspace| workspace.workspace_id.clone())
                .collect();
            snapshot.terminal_contexts.push(TerminalContext {
                terminal_context_id: context,
                tabs: vec![],
                groups: vec![],
            });
            origin.cache = project_preferred_workspace(&snapshot).unwrap();
            let plan = launcher
                .create_with_defaults(request(90), &origin, locale)
                .unwrap();
            assert!(
                matches!(plan.intent, WorktreeIntent::Create { branch: None, label: Some(ref label), .. } if label.as_str() == expected)
            );
            assert_eq!(plan.operation_id, request(90));
        }
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
    assert!(l.confirm(token, request(92), &origin,).is_none());
    assert!(!l.observed(&request(91), Ok(pending(plan.project.clone()))));
    l.dismiss();
    assert!(l.reopen());
    assert!(!l.operation().unwrap().focus_allowed);
    assert!(l.observed(&plan.operation_id, Ok(pending(plan.project))));
    assert!(l.pending());
}
#[test]
fn worktree_opened_keeps_canonical_receipt_after_source_or_entry_failure() {
    let (mut l, origin) = fixture(1, WorktreeMode::Open);
    let plan = confirm(&mut l, &origin).unwrap();
    let opened = WorkbenchWorktreeOutcome::Available {
        project_id: plan.project.clone(),
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
fn default_create_preserves_a_known_partial_receipt_after_observation_loss() {
    let (mut l, origin) = fixture(0, WorktreeMode::Create);
    let plan = l
        .create_with_defaults(request(90), &origin, Locale::EnUs)
        .unwrap();
    let known = pending(None);
    assert!(l.start_settled(&plan.operation_id, Ok(known.clone())));
    assert!(l.observed(&plan.operation_id, Err(true)));
    assert_eq!(l.operation().unwrap().outcome, Some(known));
    assert_eq!(l.notice(), WorktreeNotice::Unknown);
    assert!(
        l.create_with_defaults(request(91), &origin, Locale::EnUs)
            .is_none()
    );
}

#[test]
fn hiding_during_start_permanently_revokes_focus_even_after_reopen() {
    let (mut l, origin) = fixture(1, WorktreeMode::Open);
    let plan = confirm(&mut l, &origin).unwrap();
    l.dismiss();
    assert!(l.reopen());
    assert!(l.start_settled(&plan.operation_id, Ok(pending(plan.project))));
    assert!(!l.operation().unwrap().focus_allowed);
}
#[test]
fn cancel_or_newer_focus_prevents_default_create_after_discovery() {
    for hide in [false, true] {
        let (mut l, origin) = fixture(0, WorktreeMode::Create);
        if hide {
            l.dismiss();
        } else {
            l.suppress_focus();
        }
        assert!(
            l.create_with_defaults(request(90), &origin, Locale::EnUs)
                .is_none()
        );
    }
}

#[test]
fn completing_an_inflight_packet_requires_another_presented_draw() {
    let (mut l, origin) = fixture(1, WorktreeMode::Open);
    let token = l.token().unwrap();
    l.draw(Some(token));
    edit(&mut l, WorktreeEdit::Completeness(false));
    edit(&mut l, WorktreeEdit::Completeness(true));
    assert!(l.confirm(token, request(90), &origin,).is_none());
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
    assert!(l.confirm(token, request(91), &origin,).is_none());
}

#[test]
fn an_expired_read_keeps_the_known_opened_workspace_receipt() {
    let (mut l, origin) = fixture(1, WorktreeMode::Open);
    let plan = confirm(&mut l, &origin).unwrap();
    let workspace =
        orchbee_workbench_protocol::WorkbenchWorkspaceId::parse(&format!("{:032x}", 80)).unwrap();
    let opened = WorkbenchWorktreeOutcome::Available {
        project_id: plan.project.clone(),
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
            project_id: plan.project.clone(),
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
        project_id: plan.project.clone(),
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

#[test]
fn worktree_stage_transition_keeps_admission_but_requires_the_new_token_to_be_drawn() {
    let snapshot = canonical(1, 1);
    let state = ready_with_cache(project_preferred_workspace(&snapshot).unwrap());
    let preparation = state
        .worktree_preparation(
            WorktreeMode::Create,
            1,
            WorktreeToken(0),
            OperationDeadlines::from_sample(Instant::now()).unwrap(),
        )
        .unwrap();
    let mut launcher = Launcher::default();
    assert!(launcher.open(preparation));
    let preparing = launcher.preparation().unwrap().clone();
    launcher.draw(Some(preparing.token));
    assert!(launcher.prepared(
        &preparing,
        Some(WorktreeSource::Directory {
            path: path("C:/repo")
        })
    ));
    assert!(launcher.discovered(preparing.token, Ok(discovery())));

    let draft = launcher.token().unwrap();
    assert_eq!(
        launcher.view().unwrap().presentation(),
        PresentationPhase::Admitted
    );
    assert!(!launcher.edit(draft, WorktreeEdit::Append("before-draw")));
    launcher.draw(Some(draft));
    assert!(launcher.edit(draft, WorktreeEdit::Append("after-draw")));
    assert_eq!(launcher.view().unwrap().branch(), "after-draw");
}

#[test]
fn first_worktree_geometry_block_refuses_only_the_unadmitted_open_attempt() {
    let snapshot = canonical(1, 1);
    let state = ready_with_cache(project_preferred_workspace(&snapshot).unwrap());
    let preparation = state
        .worktree_preparation(
            WorktreeMode::Open,
            1,
            WorktreeToken(0),
            OperationDeadlines::from_sample(Instant::now()).unwrap(),
        )
        .unwrap();
    let mut launcher = Launcher::default();
    assert!(launcher.open(preparation.clone()));
    assert_eq!(
        launcher.view().unwrap().presentation(),
        PresentationPhase::Pending
    );
    launcher.draw(None);
    assert!(launcher.visible());
    assert_eq!(
        launcher.view().unwrap().presentation(),
        PresentationPhase::Pending
    );

    assert_eq!(
        launcher.geometry_blocked(),
        Some(GeometrySettlement::Refused)
    );
    assert!(!launcher.visible());
    assert!(launcher.operation().is_none());
    assert_eq!(launcher.notice(), WorktreeNotice::PresentationUnavailable);
    assert!(!launcher.prepared(
        &preparation,
        Some(WorktreeSource::Directory {
            path: path("C:/repo")
        })
    ));
}

#[test]
fn worktree_geometry_suspension_retains_the_same_draft_until_a_fresh_draw() {
    let (mut launcher, origin) = fixture(1, WorktreeMode::Create);
    edit(&mut launcher, WorktreeEdit::Append("feature/preserved"));
    let token = launcher.token().unwrap();
    assert_eq!(
        launcher.view().unwrap().presentation(),
        PresentationPhase::Admitted
    );
    launcher.draw(Some(token));
    assert_eq!(
        launcher.view().unwrap().presentation(),
        PresentationPhase::Admitted
    );
    let branch = launcher.view().unwrap().branch().to_owned();
    assert!(launcher.edit(token, WorktreeEdit::Left));
    let token = launcher.token().unwrap();
    launcher.draw(Some(token));
    let cursor = launcher
        .view()
        .unwrap()
        .field_window(40)
        .unwrap()
        .cursor_column();
    assert!(launcher.edit(token, WorktreeEdit::Completeness(false)));

    assert_eq!(
        launcher.geometry_blocked(),
        Some(GeometrySettlement::Suspended)
    );
    assert!(launcher.visible());
    assert_eq!(launcher.token(), Some(token));
    assert_eq!(
        launcher.view().unwrap().presentation(),
        PresentationPhase::Suspended
    );
    assert_eq!(launcher.view().unwrap().branch(), branch);
    assert_eq!(
        launcher
            .view()
            .unwrap()
            .field_window(40)
            .unwrap()
            .cursor_column(),
        cursor
    );
    assert!(!launcher.edit(token, WorktreeEdit::Append("-hidden")));
    assert!(launcher.confirm(token, request(90), &origin).is_none());

    launcher.draw(None);
    assert!(!launcher.edit(token, WorktreeEdit::Append("-covered")));
    launcher.draw(Some(token));
    assert!(launcher.edit(token, WorktreeEdit::Completeness(true)));
    assert!(launcher.edit(token, WorktreeEdit::Append("-resumed")));
    assert_eq!(
        launcher.view().unwrap().branch(),
        "feature/preserve-resumedd"
    );
}

#[test]
fn admitted_geometry_suspension_keeps_the_accepted_start_and_revokes_only_entry_focus() {
    let (mut launcher, origin) = fixture(1, WorktreeMode::Open);
    let plan = confirm(&mut launcher, &origin).unwrap();
    let token = launcher.token().unwrap();

    assert_eq!(
        launcher.geometry_blocked(),
        Some(GeometrySettlement::Suspended)
    );
    let suspended = launcher.view().unwrap();
    assert_eq!(suspended.token(), token);
    assert_eq!(suspended.stage(), WorktreeStage::Waiting);
    assert_eq!(suspended.presentation(), PresentationPhase::Suspended);
    assert!(launcher.start_settled(&plan.operation_id, Ok(pending(plan.project.clone()))));
    assert_eq!(
        launcher.operation().unwrap().plan.operation_id,
        plan.operation_id
    );
    assert!(!launcher.operation().unwrap().focus_allowed);

    launcher.draw(Some(token));
    assert_eq!(
        launcher.view().unwrap().presentation(),
        PresentationPhase::Admitted
    );
    assert_eq!(
        launcher.operation().unwrap().plan.operation_id,
        plan.operation_id
    );
    assert!(!launcher.operation().unwrap().focus_allowed);
}

#[test]
fn refused_reopen_keeps_one_accepted_worktree_operation_without_restoring_focus() {
    let (mut launcher, origin) = fixture(1, WorktreeMode::Open);
    let plan = confirm(&mut launcher, &origin).unwrap();
    launcher.dismiss();
    assert!(launcher.reopen());
    assert_eq!(
        launcher.view().unwrap().presentation(),
        PresentationPhase::Pending
    );
    assert_eq!(
        launcher.geometry_blocked(),
        Some(GeometrySettlement::Refused)
    );
    assert!(!launcher.visible());
    assert_eq!(launcher.notice(), WorktreeNotice::PresentationUnavailable);

    assert!(launcher.start_settled(&plan.operation_id, Ok(pending(plan.project.clone()))));
    assert_eq!(
        launcher.operation().unwrap().plan.operation_id,
        plan.operation_id
    );
    assert!(!launcher.operation().unwrap().focus_allowed);
    assert!(launcher.reopen());
    let token = launcher.token().unwrap();
    launcher.draw(Some(token));
    assert_eq!(launcher.notice(), WorktreeNotice::Pending);
    assert_eq!(
        launcher.begin_read().unwrap().operation_id,
        plan.operation_id
    );
    let opened = WorkbenchWorktreeOutcome::Available {
        project_id: plan.project.clone(),
        attempt: 2,
        status: Box::new(WorktreeOperationStatus::Opened {
            checkout: WorktreeCheckoutFacts {
                repository_root: path("C:/repo"),
                common_directory: path("C:/repo/.git"),
                path: path("C:/repo"),
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
    assert!(launcher.observed(&plan.operation_id, Ok(opened)));
    assert_eq!(launcher.notice(), WorktreeNotice::EntryDeferred);
    assert!(!launcher.operation().unwrap().focus_allowed);
}

#[test]
fn covered_completion_retains_the_operation_without_replaying_automatic_entry_on_uncover() {
    let (mut launcher, origin) = fixture(1, WorktreeMode::Open);
    let plan = confirm(&mut launcher, &origin).unwrap();
    launcher.draw(None);
    let opened = WorkbenchWorktreeOutcome::Available {
        project_id: plan.project.clone(),
        attempt: 1,
        status: Box::new(WorktreeOperationStatus::Opened {
            checkout: WorktreeCheckoutFacts {
                repository_root: path("C:/repo"),
                common_directory: path("C:/repo/.git"),
                path: path("C:/repo"),
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

    assert!(launcher.start_settled(&plan.operation_id, Ok(opened)));
    assert_eq!(launcher.notice(), WorktreeNotice::EntryDeferred);
    assert!(!launcher.operation().unwrap().focus_allowed);
    let token = launcher.token().unwrap();
    launcher.draw(Some(token));
    assert!(!launcher.operation().unwrap().focus_allowed);
    assert_eq!(
        launcher.begin_read().unwrap().operation_id,
        plan.operation_id
    );
}
