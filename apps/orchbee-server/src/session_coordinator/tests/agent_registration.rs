use super::super::agent_registration::RegistrationFact;
use super::panel_split::{close_leaf, workspace_leaf};
use super::tab_create::{
    configure_workspace, context_tab_command, dispatch_tab, immediate_completion, pending_first_tab,
};
use super::*;
use crate::terminal_coordinator::TerminalPhase;
use orchbee_capability_node_protocol::{
    HostTerminalIdentity, WorkerGeneration, WorkerId as CapabilityWorkerId,
};
use orchbee_server::WorkbenchTerminalInvariantError;
use orchbee_workbench_protocol::TerminalPresentationId;

fn complete_opened_member(coordinator: &mut SessionCoordinator) -> TerminalOperationAuthorization {
    let pending = pending_first_tab(coordinator, 1, 101);
    let authorization = launch_authorization(pending.command());
    let completion = immediate_completion(pending.command());
    coordinator
        .complete_terminal_tab_create(pending, completion, Some(worker(1)))
        .unwrap();
    authorization
}

fn member_binding(
    session: &mut SessionCoordinator,
) -> (TerminalPresentationId, PresentationBinding) {
    let committed = snapshot(session);
    let mut members = Vec::new();
    for context in &committed.terminal_contexts {
        let membership = session
            .workbench
            .as_ref()
            .unwrap()
            .terminal_tree_membership(&context.terminal_context_id)
            .unwrap();
        for member in membership.members() {
            members.push((member.presentation_id().clone(), member.binding().clone()));
        }
    }
    assert_eq!(members.len(), 1);
    members.pop().unwrap()
}

#[test]
fn empty_and_missing_workbench_sessions_have_no_registrations() {
    let coordinator = SessionCoordinator::new(None);
    assert_eq!(coordinator.agent_registration_snapshot(), Ok(vec![]));
    let coordinator = session();
    assert_eq!(coordinator.agent_registration_snapshot(), Ok(vec![]));
}

#[test]
fn pending_joint_creates_commit_membership_without_registration() {
    let mut coordinator = session();
    let context_id = configure_workspace(&mut coordinator, "/tmp/workspace");
    let command = context_tab_command(&coordinator, 710, 3, context_id, "workspace");
    let _pending = dispatch_tab(&mut coordinator, command, 701);
    // The member is already committed while the retained create is still Creating.
    assert_eq!(coordinator.agent_registration_snapshot(), Ok(vec![]));

    let _initial_workspace = pending_first_tab(&mut coordinator, 2, 801);
    assert_eq!(coordinator.agent_registration_snapshot(), Ok(vec![]));
}

#[test]
fn accepted_initial_workspace_member_registers_one_typed_fact() {
    let mut coordinator = session();
    let authorization = complete_opened_member(&mut coordinator);
    let (presentation_id, binding) = member_binding(&mut coordinator);

    let facts = coordinator.agent_registration_snapshot().unwrap();
    let [fact] = facts.as_slice() else {
        panic!("exactly one registration expected: {facts:?}")
    };
    assert_eq!(fact.presentation_id(), &presentation_id);
    assert_eq!(fact.binding(), &binding);
    assert_eq!(fact.worker_id(), authorization.worker_id());
    assert_eq!(fact.worker_generation(), authorization.worker_generation());
    assert_eq!(fact.identity(), identity());
    assert_eq!(
        coordinator.agent_registration_for_bindings(&[]).unwrap(),
        vec![]
    );
    assert_eq!(
        coordinator
            .agent_registration_for_bindings(std::slice::from_ref(&binding))
            .unwrap(),
        facts
    );
    let mut unrelated = binding.clone();
    unrelated.worker_id =
        orchbee_workbench_protocol::WorkerId::parse(&format!("{:032x}", 899)).unwrap();
    assert!(
        coordinator
            .agent_registration_for_bindings(&[unrelated])
            .unwrap()
            .is_empty()
    );
}

#[test]
fn accepted_workspace_member_registers_one_typed_fact() {
    let mut coordinator = session();
    let context_id = configure_workspace(&mut coordinator, "/tmp/workspace");
    let command = context_tab_command(&coordinator, 710, 3, context_id, "workspace");
    let pending = dispatch_tab(&mut coordinator, command, 701);
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(launch) = pending.command() else {
        panic!("launch expected")
    };
    let authorization = launch.authorization();
    let completion = immediate_completion(pending.command());
    coordinator
        .complete_terminal_tab_create(pending, completion, Some(worker(1)))
        .unwrap();
    let (presentation_id, binding) = member_binding(&mut coordinator);

    let facts = coordinator.agent_registration_snapshot().unwrap();
    let [fact] = facts.as_slice() else {
        panic!("exactly one registration expected: {facts:?}")
    };
    assert_eq!(fact.presentation_id(), &presentation_id);
    assert_eq!(fact.binding(), &binding);
    assert_eq!(fact.worker_id(), authorization.worker_id());
    assert_eq!(fact.worker_generation(), authorization.worker_generation());
    assert_eq!(fact.identity(), identity());
}

#[test]
fn raw_terminal_creates_without_membership_never_register() {
    let mut coordinator = session();
    let pending = dispatch(coordinator.prepare(
        create(50),
        Some(worker(1)),
        Some(expiry()),
        &LaunchCapture::absent(),
    ));
    assert_eq!(coordinator.agent_registration_snapshot(), Ok(vec![]));
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(launch) = pending.command() else {
        panic!("launch expected")
    };
    let completion = Completion::Launch(
        TerminalLaunchResult::new(
            launch.authorization(),
            TerminalLaunchOutcome::HostAccepted(HostLaunchAccepted::new(
                identity(),
                HostLaunchState::AwaitingStart,
            )),
        )
        .unwrap(),
    );
    coordinator
        .complete(pending, completion, Some(worker(1)))
        .unwrap();
    assert_eq!(coordinator.agent_registration_snapshot(), Ok(vec![]));
}

#[test]
fn every_identity_retaining_phase_remains_eligible() {
    let mut coordinator = session();
    let _authorization = complete_opened_member(&mut coordinator);
    for (phase, expected) in [
        (TerminalPhase::AwaitingStart(identity()), 1),
        (TerminalPhase::Starting(identity()), 1),
        (
            TerminalPhase::Started {
                identity: identity(),
                geometry: geometry(),
            },
            1,
        ),
        (TerminalPhase::Unknown(Some(identity())), 1),
        (TerminalPhase::Unavailable(Some(identity())), 1),
        (TerminalPhase::Unknown(None), 0),
        (TerminalPhase::Unavailable(None), 0),
    ] {
        // Member bindings are immediate-only today, so joint transitions cannot reach
        // these retained phases; the predicate is exercised through the test setter on
        // the real committed member and retained binding. `Creating` is covered by the
        // pending-create test; forcing it here would instead violate the outcome/phase
        // invariant because this member already carries a create outcome.
        coordinator.terminals.set_phase_for_test(&id(101), phase);
        let facts = coordinator.agent_registration_snapshot().unwrap();
        assert_eq!(facts.len(), expected, "phase {phase:?}");
        if expected == 1 {
            assert_eq!(facts[0].identity(), identity());
        }
    }
}

#[test]
fn removed_membership_disappears_from_the_snapshot() {
    let mut coordinator = session();
    let _authorization = complete_opened_member(&mut coordinator);
    assert_eq!(coordinator.agent_registration_snapshot().unwrap().len(), 1);
    let (context, tab, panel) = workspace_leaf(&mut coordinator);
    let _close = close_leaf(&mut coordinator, context, tab, panel, 900);
    assert_eq!(coordinator.agent_registration_snapshot(), Ok(vec![]));
}

#[test]
fn coordinator_invariant_failures_abort_the_whole_snapshot() {
    for corrupt in [
        (|coordinator: &mut SessionCoordinator| {
            // A completed member forced back to Creating violates the outcome/phase
            // invariant; the snapshot must not silently drop or keep either member.
            coordinator
                .terminals
                .set_phase_for_test(&id(701), TerminalPhase::Creating);
        }) as fn(&mut SessionCoordinator),
        |coordinator: &mut SessionCoordinator| {
            // A second retained create sharing the member's identity pair is ambiguous.
            let authorization = coordinator
                .terminals
                .retained_authorization(&id(701))
                .unwrap();
            coordinator
                .terminals
                .replace_authorization_for_test(&id(900), authorization);
        },
    ] {
        let mut coordinator = session();
        let context_id = configure_workspace(&mut coordinator, "/tmp/workspace");
        let command = context_tab_command(&coordinator, 710, 3, context_id, "workspace");
        let pending = dispatch_tab(&mut coordinator, command, 701);
        let completion = immediate_completion(pending.command());
        coordinator
            .complete_terminal_tab_create(pending, completion, Some(worker(1)))
            .unwrap();
        let _raw = dispatch(coordinator.prepare(
            create(900),
            Some(worker(1)),
            Some(expiry()),
            &LaunchCapture::absent(),
        ));
        corrupt(&mut coordinator);
        assert_eq!(
            coordinator.agent_registration_snapshot(),
            Err(WorkbenchTerminalInvariantError)
        );
    }
}

#[test]
fn registration_facts_convert_for_a_consumer_shaped_reader() {
    fn consumer_view(
        facts: &[RegistrationFact],
    ) -> Vec<(
        TerminalPresentationId,
        CapabilityWorkerId,
        WorkerGeneration,
        HostTerminalIdentity,
    )> {
        facts
            .iter()
            .map(|fact| {
                (
                    fact.presentation_id().clone(),
                    fact.worker_id(),
                    fact.worker_generation(),
                    fact.identity(),
                )
            })
            .collect()
    }

    let mut coordinator = session();
    let authorization = complete_opened_member(&mut coordinator);
    let facts = coordinator.agent_registration_snapshot().unwrap();
    let view = consumer_view(&facts);
    assert_eq!(view.len(), 1);
    assert_eq!(view[0].1, authorization.worker_id());
    assert_eq!(view[0].2, authorization.worker_generation());
    assert_eq!(view[0].3, identity());
}

#[cfg(windows)]
mod process_samples {
    use super::*;
    use crate::session_coordinator::PROCESS_IDENTITY_SETTLE as SETTLE;
    use orchbee_terminal_protocol::{
        AttachmentId, TerminalId, TerminalProcess, TerminalProcessConsole, TerminalProcessInfo,
    };
    use orchbee_workbench_protocol::{
        PanelAgentAction, PanelAgentOutcome, PanelAgentRequest, PanelAgentState,
    };
    use std::time::{Duration, Instant};
    fn fixture() -> (
        SessionCoordinator,
        crate::terminal_coordinator::BindingViewerTarget,
        TerminalProcessInfo,
    ) {
        let mut session = session();
        complete_opened_member(&mut session);
        let fact = session.agent_registration_snapshot().unwrap().remove(0);
        let target = crate::terminal_coordinator::BindingViewerTarget {
            terminal_id: orchbee_capability_node_protocol::TerminalId::from_bytes(
                u128::from_str_radix(fact.binding().terminal_id.as_str(), 16)
                    .unwrap()
                    .to_be_bytes(),
            )
            .unwrap(),
            identity: fact.identity(),
            worker: worker(1),
        };
        let sample = TerminalProcessInfo {
            attachment_id: AttachmentId::from_bytes([1; 16]).unwrap(),
            terminal_id: TerminalId::from_bytes(target.identity.terminal_id().as_bytes()).unwrap(),
            root_pid: 10,
            processes: vec![
                TerminalProcess {
                    pid: 10,
                    parent_pid: None,
                    creation_time: 100,
                    console: TerminalProcessConsole::Attached,
                    image: Some("C:/Windows/cmd.exe".into()),
                    command_line: Some("cmd.exe".into()),
                    current_directory: None,
                },
                TerminalProcess {
                    pid: 20,
                    parent_pid: Some(10),
                    creation_time: 200,
                    console: TerminalProcessConsole::Attached,
                    image: Some("C:/tools/claude.exe".into()),
                    command_line: Some("claude.exe".into()),
                    current_directory: None,
                },
            ],
            launch_kind: None,
        };
        (session, target, sample)
    }
    fn read(
        session: &mut SessionCoordinator,
    ) -> (
        orchbee_workbench_protocol::WorkbenchRevision,
        orchbee_workbench_protocol::PanelAgentView,
    ) {
        let snap = session.workbench.as_ref().unwrap().snapshot();
        let PanelNode::Panel { panel_id, .. } = &snap.terminal_contexts[0].tabs[0].root else {
            panic!("panel")
        };
        let request = PanelAgentRequest {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(&format!("{:032x}", 777)).unwrap(),
            session_id: snap.session_id.clone(),
            action: PanelAgentAction::Read {
                panel_ids: vec![panel_id.clone()],
            },
        };
        let PanelAgentOutcome::Read {
            report_revision,
            mut panels,
            ..
        } = session
            .panel_agent(request, Instant::now())
            .unwrap()
            .outcome
        else {
            panic!("read")
        };
        (report_revision, panels.remove(0).report.unwrap())
    }
    #[test]
    fn periodic_discovery_retains_physical_identity_and_prunes_empty_membership() {
        let (mut session, target, sample) = fixture();
        let targets = session.process_observation_targets().unwrap();
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].target, target);
        assert_eq!(
            targets[0].session_id,
            *session.workbench.as_ref().unwrap().session_id()
        );
        let now = Instant::now();
        assert!(session.observe_host_process(target, &sample, now, now));
        assert_eq!(session.process_samples.len(), 1);
        // An invalid registration scan must not discard earlier tracking.
        session
            .terminals
            .set_phase_for_test(&id(101), TerminalPhase::Creating);
        assert!(session.process_observation_targets().is_err());
        assert_eq!(session.process_samples.len(), 1);
        session
            .terminals
            .set_phase_for_test(&id(101), TerminalPhase::Unavailable(None));
        assert!(session.process_observation_targets().unwrap().is_empty());
        assert!(session.process_samples.is_empty());
        // Removing the Workbench owner also releases its retained observations.
        session.terminals.set_phase_for_test(
            &id(101),
            TerminalPhase::Started {
                identity: identity(),
                geometry: geometry(),
            },
        );
        let later = now + Duration::from_millis(1);
        assert!(session.observe_host_process(target, &sample, later, later));
        session.workbench = None;
        assert!(session.process_observation_targets().unwrap().is_empty());
        assert!(session.process_samples.is_empty());
    }
    #[test]
    fn checkpoint_directory_cache_rejects_stale_samples_and_physical_generations() {
        use orchbee_workbench_protocol::WorkbenchDirectoryPath;
        let (mut session, target, sample) = fixture();
        let targets = session.checkpoint_observation_targets().unwrap().1;
        let first = WorkbenchDirectoryPath::parse("C:/first").unwrap();
        let second = WorkbenchDirectoryPath::parse("C:/second").unwrap();
        let now = Instant::now();
        assert!(session.cached_checkpoint_directories(&targets).is_empty());
        assert!(session.observe_host_screen_with_directory(
            target,
            &sample,
            None,
            Some(first.clone()),
            now,
            now
        ));
        let later = now + Duration::from_millis(1);
        assert!(session.observe_host_screen_with_directory(
            target,
            &sample,
            None,
            Some(second.clone()),
            later,
            later
        ));
        assert!(!session.observe_host_screen_with_directory(
            target,
            &sample,
            None,
            Some(first),
            now,
            later
        ));
        assert_eq!(
            session.cached_checkpoint_directories(&targets),
            vec![(targets[0].binding.clone(), second.clone())]
        );
        // A new observation without a directory retains the last known directory for this physical owner.
        let last = later + Duration::from_millis(1);
        assert!(
            session.observe_host_screen_with_directory(target, &sample, None, None, last, last)
        );
        assert_eq!(session.cached_checkpoint_directories(&targets)[0].1, second);
        let mut stale = targets.clone();
        stale[0].target.worker = worker(2);
        assert!(session.cached_checkpoint_directories(&stale).is_empty());
        assert!(session.cached_checkpoint_directories(&[]).is_empty());
        session
            .terminals
            .set_phase_for_test(&id(101), TerminalPhase::Unavailable(None));
        assert!(session.process_observation_targets().unwrap().is_empty());
        assert!(session.cached_checkpoint_directories(&targets).is_empty());
    }
    #[test]
    fn identify_answers_the_current_generation_identity_and_fences_unknown_presentations() {
        use orchbee_agent_observation_protocol::{
            AgentIdentityGrade, AgentIdentityUnknownReason, AgentProcessAnchor,
            AgentProcessIdentity, AgentReportProvider,
        };
        use orchbee_workbench_protocol::{PanelAgentFailure, TerminalPresentationId};
        let (mut session, target, sample) = fixture();
        let snap = session.workbench.as_ref().unwrap().snapshot().clone();
        let PanelNode::Panel {
            primary_presentation,
            ..
        } = &snap.terminal_contexts[0].tabs[0].root
        else {
            panic!("panel")
        };
        let identify = |session: &mut SessionCoordinator, presentation_id| {
            session.identify_presentation(
                &PanelAgentRequest {
                    protocol_version: ProtocolVersion,
                    request_id: WorkbenchRequestId::parse(&format!("{:032x}", 778)).unwrap(),
                    session_id: snap.session_id.clone(),
                    action: PanelAgentAction::Identify { presentation_id },
                },
                Instant::now(),
            )
        };
        let presentation = primary_presentation.presentation_id.clone();
        let identity_of = |outcome| match outcome {
            PanelAgentOutcome::Identified {
                workbench_revision,
                binding,
                identity,
                anchor,
            } => {
                // An `agent` result always names the published run's provider.
                if let AgentProcessIdentity::Agent { provider, .. } = identity {
                    assert_eq!(anchor.map(|anchor| anchor.provider), Some(provider));
                }
                assert_eq!(workbench_revision, snap.revision);
                assert_eq!(binding.presentation_id.as_str(), presentation.as_str());
                assert_eq!(
                    binding.host_terminal_id.as_str(),
                    format!(
                        "{:032x}",
                        u128::from_be_bytes(target.identity.terminal_id().as_bytes())
                    )
                );
                (identity, anchor)
            }
            other => panic!("{other:?}"),
        };
        assert_eq!(
            identity_of(identify(&mut session, presentation.clone())),
            (
                AgentProcessIdentity::Unknown(AgentIdentityUnknownReason::Settling),
                None
            )
        );
        let now = Instant::now();
        assert!(session.observe_host_process(target, &sample, now - SETTLE, now - SETTLE));
        assert!(session.observe_host_process(target, &sample, now, now));
        let claude = Some(AgentProcessAnchor {
            pid: 20,
            provider: AgentReportProvider::Claude,
        });
        assert_eq!(
            identity_of(identify(&mut session, presentation.clone())),
            (
                AgentProcessIdentity::Agent {
                    provider: AgentReportProvider::Claude,
                    grade: AgentIdentityGrade::Recognized,
                },
                claude
            )
        );
        let unknown = TerminalPresentationId::parse(&format!("{:032x}", 999)).unwrap();
        assert!(matches!(
            identify(&mut session, unknown),
            PanelAgentOutcome::Failed {
                reason: PanelAgentFailure::NotFound
            }
        ));
    }
    /// The `identify` answer stays one Server fact along a run: the anchor is exactly the run
    /// Panel reports attribute, and an `agent` result never appears without it.
    #[test]
    fn identify_anchor_follows_the_published_run_across_a_trajectory() {
        use orchbee_agent_observation_protocol::{
            AgentIdentityUnknownReason, AgentProcessAnchor, AgentProcessIdentity,
            AgentReportProvider,
        };
        let (mut session, target, sample) = fixture();
        let snap = session.workbench.as_ref().unwrap().snapshot().clone();
        let PanelNode::Panel {
            primary_presentation,
            ..
        } = &snap.terminal_contexts[0].tabs[0].root
        else {
            panic!("panel")
        };
        let presentation_id = primary_presentation.presentation_id.clone();
        let identify = |session: &mut SessionCoordinator| match session.identify_presentation(
            &PanelAgentRequest {
                protocol_version: ProtocolVersion,
                request_id: WorkbenchRequestId::parse(&format!("{:032x}", 779)).unwrap(),
                session_id: snap.session_id.clone(),
                action: PanelAgentAction::Identify {
                    presentation_id: presentation_id.clone(),
                },
            },
            Instant::now(),
        ) {
            PanelAgentOutcome::Identified {
                identity, anchor, ..
            } => (identity, anchor),
            other => panic!("{other:?}"),
        };
        let claude = Some(AgentProcessAnchor {
            pid: 20,
            provider: AgentReportProvider::Claude,
        });
        let start = Instant::now();
        // One sample only names a settling anchor: no run, so neither an anchor nor a report.
        let _ = session.observe_host_process(target, &sample, start, start);
        assert_eq!(
            identify(&mut session),
            (
                AgentProcessIdentity::Unknown(AgentIdentityUnknownReason::Settling),
                None
            )
        );
        assert_eq!(read(&mut session).1.agent, None);
        let settled = start + SETTLE;
        let _ = session.observe_host_process(target, &sample, settled, settled);
        let (identity, anchor) = identify(&mut session);
        assert!(matches!(identity, AgentProcessIdentity::Agent { .. }));
        assert_eq!(anchor, claude);
        assert_eq!(read(&mut session).1.agent.as_deref(), Some("claude"));
        // An opaque row outside the anchor leaves the result unknown, but the published run, its
        // anchor, and the Panel report stay the same Server fact.
        let mut opaque = sample.clone();
        opaque.processes.push(TerminalProcess {
            pid: 30,
            parent_pid: Some(10),
            creation_time: 300,
            console: TerminalProcessConsole::Attached,
            image: None,
            command_line: None,
            current_directory: None,
        });
        let later = settled + Duration::from_millis(1);
        let _ = session.observe_host_process(target, &opaque, later, later);
        assert_eq!(
            identify(&mut session),
            (
                AgentProcessIdentity::Unknown(AgentIdentityUnknownReason::CommandLineUnreadable),
                claude
            )
        );
        assert_eq!(read(&mut session).1.agent.as_deref(), Some("claude"));
        // A complete sample without the anchor ends the run: no anchor and no report.
        let mut shell = sample.clone();
        shell.processes.truncate(1);
        let ended = later + Duration::from_millis(1);
        let _ = session.observe_host_process(target, &shell, ended, ended);
        assert_eq!(identify(&mut session), (AgentProcessIdentity::None, None));
        assert_eq!(read(&mut session).1.agent, None);
    }
    #[test]
    fn complete_samples_confirm_without_revision_churn_then_prove_pid_reuse_exit() {
        let (mut session, target, mut sample) = fixture();
        let now = Instant::now();
        assert!(session.observe_host_process(target, &sample, now - SETTLE, now - SETTLE));
        assert!(session.observe_host_process(target, &sample, now, now));
        let (rev, view) = read(&mut session);
        assert_eq!(view.agent.as_deref(), Some("claude"));
        assert_eq!(view.state, PanelAgentState::Unknown);
        let later = now + Duration::from_millis(1);
        assert!(session.observe_host_process(target, &sample, later, later));
        assert_eq!(read(&mut session).0, rev);
        assert!(!session.observe_host_process(target, &sample, now, later));
        sample.processes[1].creation_time += 1;
        let later = later + Duration::from_millis(1);
        assert!(session.observe_host_process(target, &sample, later, later));
        assert_eq!(read(&mut session).1.agent, None);
        // A restart is a new run that settles again.
        let later = later + SETTLE;
        assert!(session.observe_host_process(target, &sample, later, later));
        assert_eq!(read(&mut session).1.agent.as_deref(), Some("claude"));
    }
    #[test]
    fn wrong_physical_generation_missing_image_and_ambiguous_chains_do_not_erase_reports() {
        let (mut session, target, mut sample) = fixture();
        let now = Instant::now();
        assert!(session.observe_host_process(target, &sample, now - SETTLE, now - SETTLE));
        assert!(session.observe_host_process(target, &sample, now, now));
        let before = read(&mut session);
        let later = now + Duration::from_millis(1);
        let mut stale = target;
        stale.worker = worker(2);
        assert!(!session.observe_host_process(stale, &sample, later, later));
        assert_eq!(read(&mut session), before);
        sample.processes[1].image = None;
        assert!(session.observe_host_process(target, &sample, later, later));
        assert_eq!(read(&mut session), before);
        sample.processes[1].image = Some("claude.exe".into());
        let mut sibling = sample.processes[1].clone();
        sibling.pid = 30;
        sibling.creation_time = 300;
        sibling.image = Some("codex.exe".into());
        sample.processes.push(sibling);
        let later = later + Duration::from_millis(1);
        assert!(session.observe_host_process(target, &sample, later, later));
        assert_eq!(read(&mut session), before);
        assert!(!session.observe_host_process(
            target,
            &sample,
            later + Duration::from_millis(1),
            later
        ));
    }
    #[test]
    fn screen_grace_idle_hold_missing_samples_and_generation_reset() {
        use crate::agent_screen::Detection;
        let (mut session, target, mut sample) = fixture();
        let now = Instant::now();
        let working = Detection {
            state: PanelAgentState::Working,
            skip: false,
            visible_working: false,
            visible_idle: false,
            visible_blocker: false,
        };
        let idle = Detection {
            state: PanelAgentState::Idle,
            visible_working: false,
            ..working
        };
        let admit = |session: &mut SessionCoordinator, ms, detection| {
            let at = now + Duration::from_millis(ms);
            assert!(session.observe_host_screen(target, &sample, detection, at, at));
        };
        assert!(session.observe_host_process(target, &sample, now - SETTLE, now - SETTLE));
        admit(&mut session, 0, Some(working));
        admit(&mut session, 2999, Some(working));
        assert_eq!(read(&mut session).1.state, PanelAgentState::Unknown);
        admit(&mut session, 3000, Some(working));
        let working_view = read(&mut session);
        assert_eq!(working_view.1.state, PanelAgentState::Working);
        admit(&mut session, 3100, None);
        assert_eq!(read(&mut session), working_view);
        for ms in [3200, 3300, 3400] {
            admit(&mut session, ms, Some(idle));
            assert_eq!(read(&mut session), working_view);
        }
        // Transcript skip clears the pending idle timer; ordinary idle starts from zero again.
        admit(&mut session, 3500, Some(Detection { skip: true, ..idle }));
        admit(&mut session, 4200, Some(idle));
        assert_eq!(read(&mut session), working_view);
        admit(&mut session, 4900, Some(idle));
        assert_eq!(read(&mut session).1.state, PanelAgentState::Idle);
        sample.processes[1].creation_time += 1;
        for ms in [5000, 5100, 5500, 8400] {
            let at = now + Duration::from_millis(ms);
            assert!(session.observe_host_screen(target, &sample, Some(working), at, at));
            assert_eq!(read(&mut session).1.state, PanelAgentState::Unknown);
        }
        let at = now + Duration::from_millis(8500);
        assert!(session.observe_host_screen(target, &sample, Some(working), at, at));
        assert_eq!(read(&mut session).1.state, PanelAgentState::Working);
    }
    #[test]
    fn short_visible_working_then_idle_is_published_during_startup_grace() {
        use crate::agent_screen::Detection;
        let (mut session, target, sample) = fixture();
        let origin = Instant::now();
        assert!(session.observe_host_process(target, &sample, origin - SETTLE, origin - SETTLE));
        for (ms, state) in [
            (0, PanelAgentState::Working),
            (200, PanelAgentState::Idle),
            (400, PanelAgentState::Blocked),
        ] {
            let at = origin + Duration::from_millis(ms);
            let detection = Detection {
                state,
                skip: false,
                visible_working: state == PanelAgentState::Working,
                visible_idle: state == PanelAgentState::Idle,
                visible_blocker: state == PanelAgentState::Blocked,
            };
            assert!(session.observe_host_screen(target, &sample, Some(detection), at, at));
            assert_eq!(read(&mut session).1.state, state);
        }
    }
    #[test]
    fn manifest_reload_resets_screen_hysteresis_without_changing_process_or_report_facts() {
        use crate::agent_screen::Detection;
        let (mut session, target, sample) = fixture();
        let now = Instant::now();
        let working = Detection {
            state: PanelAgentState::Working,
            skip: false,
            visible_working: true,
            visible_idle: false,
            visible_blocker: false,
        };
        let idle = Detection {
            state: PanelAgentState::Idle,
            visible_working: false,
            ..working
        };
        assert!(session.observe_host_process(target, &sample, now - SETTLE, now - SETTLE));
        for ms in [0, 3000] {
            let at = now + Duration::from_millis(ms);
            assert!(session.observe_host_screen(target, &sample, Some(working), at, at));
        }
        let before = read(&mut session);
        assert_eq!(before.1.state, PanelAgentState::Working);
        let at = now + Duration::from_millis(3100);
        assert!(session.observe_host_screen(target, &sample, Some(idle), at, at));
        assert_eq!(read(&mut session), before);
        session.reset_screen_stabilization();
        assert_eq!(read(&mut session), before);
        assert_eq!(session.process_samples.len(), 1);
        let at = now + Duration::from_millis(3200);
        assert!(session.observe_host_screen(target, &sample, Some(idle), at, at));
        let after = read(&mut session);
        assert_eq!(after.1.agent, before.1.agent);
        assert_eq!(after.1.state, PanelAgentState::Idle);
    }
    #[test]
    fn workspace_terminal_directory_follows_only_the_first_tab_exact_physical_owner() {
        use super::super::tab_create::{
            configure_workspace, context_tab_command, dispatch_tab, immediate_completion,
        };
        use orchbee_workbench_protocol::WorkbenchDirectoryPath;
        let mut session = session();
        let context = configure_workspace(&mut session, "C:/plain");
        for request in [100, 110] {
            let revision = snapshot(&mut session).revision.value();
            let create = context_tab_command(&session, request, revision, context.clone(), "tab");
            let pending = dispatch_tab(&mut session, create, request + 1);
            let completion = immediate_completion(pending.command());
            session
                .complete_terminal_tab_create(pending, completion, Some(worker(1)))
                .unwrap();
        }
        let workspace_id = snapshot(&mut session)
            .navigation
            .workspaces
            .iter()
            .find(|workspace| workspace.terminal_context_id == context)
            .unwrap()
            .workspace_id
            .clone();
        let targets = session.process_observation_targets().unwrap();
        let first_binding = snapshot(&mut session)
            .terminal_contexts
            .iter()
            .find(|candidate| candidate.terminal_context_id == context)
            .and_then(|context| match &context.tabs[0].root {
                orchbee_workbench_protocol::PanelNode::Panel {
                    primary_presentation,
                    ..
                } => primary_presentation.binding.clone(),
                orchbee_workbench_protocol::PanelNode::Split { .. } => None,
            })
            .unwrap();
        let observe = |session: &mut SessionCoordinator, first: bool, path: &str, at: Instant| {
            let target = targets
                .iter()
                .find(|target| (target.binding == first_binding) == first)
                .unwrap()
                .target;
            let sample = TerminalProcessInfo {
                attachment_id: AttachmentId::from_bytes([1; 16]).unwrap(),
                terminal_id: TerminalId::from_bytes(target.identity.terminal_id().as_bytes())
                    .unwrap(),
                root_pid: 10,
                processes: vec![],
                launch_kind: None,
            };
            let directory = WorkbenchDirectoryPath::parse(path).unwrap();
            assert!(session.observe_host_screen_with_directory(
                target,
                &sample,
                None,
                Some(directory),
                at,
                at
            ));
        };
        assert_eq!(targets.len(), 2);
        assert_eq!(session.workspace_terminal_directory(&workspace_id), None);
        let now = Instant::now();
        // A later Tab never stands in for the Workspace's first terminal.
        observe(&mut session, false, "C:/other", now);
        assert_eq!(session.workspace_terminal_directory(&workspace_id), None);
        observe(&mut session, true, "C:/repo", now);
        let expected = |path: &str| {
            Some((
                first_binding.worker_id.clone(),
                WorkbenchDirectoryPath::parse(path).unwrap(),
            ))
        };
        assert_eq!(
            session.workspace_terminal_directory(&workspace_id),
            expected("C:/repo")
        );
        // A later `cd` replaces the observed directory without a canonical change.
        observe(
            &mut session,
            true,
            "C:/repo/nested",
            now + Duration::from_millis(1),
        );
        assert_eq!(
            session.workspace_terminal_directory(&workspace_id),
            expected("C:/repo/nested")
        );
    }
}
