use super::super::agent_registration::RegistrationFact;
use super::panel_split::{close_leaf, standalone_leaf};
use super::tab_create::{
    configure_workspace, context_tab_command, dispatch_tab, immediate_completion,
    pending_first_open,
};
use super::*;
use crate::terminal_coordinator::TerminalPhase;
use orchbee_capability_node_protocol::{
    HostTerminalIdentity, WorkerGeneration, WorkerId as CapabilityWorkerId,
};
use orchbee_server::WorkbenchTerminalInvariantError;
use orchbee_workbench_protocol::TerminalPresentationId;

fn complete_opened_member(coordinator: &mut SiteCoordinator) -> TerminalOperationAuthorization {
    let pending = pending_first_open(coordinator, 1, 101);
    let authorization = launch_authorization(&pending);
    let completion = immediate_completion(pending.command());
    coordinator
        .complete(pending, completion, Some(worker(1)))
        .unwrap();
    authorization
}

fn member_binding(site: &mut SiteCoordinator) -> (TerminalPresentationId, PresentationBinding) {
    let committed = snapshot(site);
    let mut members = Vec::new();
    for context in &committed.terminal_contexts {
        let membership = site
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
fn empty_and_missing_workbench_sites_have_no_registrations() {
    let coordinator = SiteCoordinator::new(None);
    assert_eq!(coordinator.agent_registration_snapshot(), Ok(vec![]));
    let coordinator = site();
    assert_eq!(coordinator.agent_registration_snapshot(), Ok(vec![]));
}

#[test]
fn pending_joint_creates_commit_membership_without_registration() {
    let mut coordinator = site();
    let context_id = configure_workspace(&mut coordinator, "/tmp/workspace");
    let command = context_tab_command(&coordinator, 710, 3, context_id, "workspace");
    let _pending = dispatch_tab(&mut coordinator, command, 701);
    // The member is already committed while the retained create is still Creating.
    assert_eq!(coordinator.agent_registration_snapshot(), Ok(vec![]));

    let _standalone = pending_first_open(&mut coordinator, 2, 801);
    assert_eq!(coordinator.agent_registration_snapshot(), Ok(vec![]));
}

#[test]
fn accepted_standalone_member_registers_one_typed_fact() {
    let mut coordinator = site();
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
    let mut coordinator = site();
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
    let mut coordinator = site();
    let pending = dispatch(coordinator.prepare(create(50), Some(worker(1)), Some(expiry())));
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
    let mut coordinator = site();
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
    let mut coordinator = site();
    let _authorization = complete_opened_member(&mut coordinator);
    assert_eq!(coordinator.agent_registration_snapshot().unwrap().len(), 1);
    let (context, tab, panel) = standalone_leaf(&mut coordinator);
    let (_close, _closed) = close_leaf(&mut coordinator, context, tab, panel, 900);
    assert_eq!(coordinator.agent_registration_snapshot(), Ok(vec![]));
}

#[test]
fn coordinator_invariant_failures_abort_the_whole_snapshot() {
    for corrupt in [
        (|coordinator: &mut SiteCoordinator| {
            // A completed member forced back to Creating violates the outcome/phase
            // invariant; the snapshot must not silently drop or keep either member.
            coordinator
                .terminals
                .set_phase_for_test(&id(701), TerminalPhase::Creating);
        }) as fn(&mut SiteCoordinator),
        |coordinator: &mut SiteCoordinator| {
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
        let mut coordinator = site();
        let context_id = configure_workspace(&mut coordinator, "/tmp/workspace");
        let command = context_tab_command(&coordinator, 710, 3, context_id, "workspace");
        let pending = dispatch_tab(&mut coordinator, command, 701);
        let completion = immediate_completion(pending.command());
        coordinator
            .complete_terminal_tab_create(pending, completion, Some(worker(1)))
            .unwrap();
        let _raw = dispatch(coordinator.prepare(create(900), Some(worker(1)), Some(expiry())));
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

    let mut coordinator = site();
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
    use orchbee_terminal_protocol::{
        AttachmentId, TerminalId, TerminalProcess, TerminalProcessInfo,
    };
    use orchbee_workbench_protocol::{
        PanelAgentAction, PanelAgentOutcome, PanelAgentRequest, PanelAgentState,
    };
    use std::time::{Duration, Instant};
    fn fixture() -> (
        SiteCoordinator,
        crate::terminal_coordinator::BindingViewerTarget,
        TerminalProcessInfo,
    ) {
        let mut site = site();
        complete_opened_member(&mut site);
        let fact = site.agent_registration_snapshot().unwrap().remove(0);
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
                    image: Some("C:/Windows/cmd.exe".into()),
                    command_line: Some("cmd.exe".into()),
                    current_directory: None,
                },
                TerminalProcess {
                    pid: 20,
                    parent_pid: Some(10),
                    creation_time: 200,
                    image: Some("C:/tools/pi.exe".into()),
                    command_line: Some("pi.exe".into()),
                    current_directory: None,
                },
            ],
        };
        (site, target, sample)
    }
    fn read(
        site: &mut SiteCoordinator,
    ) -> (
        orchbee_workbench_protocol::WorkbenchRevision,
        orchbee_workbench_protocol::PanelAgentView,
    ) {
        let snap = site.workbench.as_ref().unwrap().snapshot();
        let PanelNode::Panel { panel_id, .. } = &snap.terminal_contexts[0].tabs[0].root else {
            panic!("panel")
        };
        let request = PanelAgentRequest {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(&format!("{:032x}", 777)).unwrap(),
            site_id: snap.site_id.clone(),
            action: PanelAgentAction::Read {
                panel_ids: vec![panel_id.clone()],
            },
        };
        let PanelAgentOutcome::Read {
            report_revision,
            mut panels,
            ..
        } = site.panel_agent(request, Instant::now()).unwrap().outcome
        else {
            panic!("read")
        };
        (report_revision, panels.remove(0).report.unwrap())
    }
    #[test]
    fn periodic_discovery_retains_physical_identity_and_prunes_empty_membership() {
        let (mut site, target, sample) = fixture();
        let targets = site.process_observation_targets().unwrap();
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].target, target);
        assert_eq!(
            targets[0].site_id,
            *site.workbench.as_ref().unwrap().site_id()
        );
        let now = Instant::now();
        assert!(site.observe_host_process(target, &sample, now, now));
        assert_eq!(site.process_samples.len(), 1);
        // An invalid registration scan must not discard earlier tracking.
        site.terminals
            .set_phase_for_test(&id(101), TerminalPhase::Creating);
        assert!(site.process_observation_targets().is_err());
        assert_eq!(site.process_samples.len(), 1);
        site.terminals
            .set_phase_for_test(&id(101), TerminalPhase::Unavailable(None));
        assert!(site.process_observation_targets().unwrap().is_empty());
        assert!(site.process_samples.is_empty());
        // Removing the Workbench owner also releases its retained observations.
        site.terminals.set_phase_for_test(
            &id(101),
            TerminalPhase::Started {
                identity: identity(),
                geometry: geometry(),
            },
        );
        let later = now + Duration::from_millis(1);
        assert!(site.observe_host_process(target, &sample, later, later));
        site.workbench = None;
        assert!(site.process_observation_targets().unwrap().is_empty());
        assert!(site.process_samples.is_empty());
    }
    #[test]
    fn complete_samples_confirm_without_revision_churn_then_prove_pid_reuse_exit() {
        let (mut site, target, mut sample) = fixture();
        let now = Instant::now();
        assert!(site.observe_host_process(target, &sample, now, now));
        let (rev, view) = read(&mut site);
        assert_eq!(view.agent.as_deref(), Some("pi"));
        assert_eq!(view.state, PanelAgentState::Unknown);
        let later = now + Duration::from_millis(1);
        assert!(site.observe_host_process(target, &sample, later, later));
        assert_eq!(read(&mut site).0, rev);
        assert!(!site.observe_host_process(target, &sample, now, later));
        sample.processes[1].creation_time += 1;
        let later = later + Duration::from_millis(1);
        assert!(site.observe_host_process(target, &sample, later, later));
        assert_eq!(read(&mut site).1.agent, None);
        let later = later + Duration::from_millis(1);
        assert!(site.observe_host_process(target, &sample, later, later));
        assert_eq!(read(&mut site).1.agent.as_deref(), Some("pi"));
    }
    #[test]
    fn wrong_physical_generation_missing_image_and_ambiguous_chains_do_not_erase_reports() {
        let (mut site, target, mut sample) = fixture();
        let now = Instant::now();
        assert!(site.observe_host_process(target, &sample, now, now));
        let before = read(&mut site);
        let later = now + Duration::from_millis(1);
        let mut stale = target;
        stale.worker = worker(2);
        assert!(!site.observe_host_process(stale, &sample, later, later));
        assert_eq!(read(&mut site), before);
        sample.processes[1].image = None;
        assert!(site.observe_host_process(target, &sample, later, later));
        assert_eq!(read(&mut site), before);
        sample.processes[1].image = Some("pi.exe".into());
        let mut sibling = sample.processes[1].clone();
        sibling.pid = 30;
        sibling.creation_time = 300;
        sibling.image = Some("codex.exe".into());
        sample.processes.push(sibling);
        let later = later + Duration::from_millis(1);
        assert!(site.observe_host_process(target, &sample, later, later));
        assert_eq!(read(&mut site), before);
        assert!(!site.observe_host_process(
            target,
            &sample,
            later + Duration::from_millis(1),
            later
        ));
    }
    #[test]
    fn process_confirmation_activates_deferred_lifecycle_and_departure_clears_it() {
        use orchbee_workbench_protocol::PanelAgentDisposition;
        let (mut site, target, mut sample) = fixture();
        let snapshot = site.workbench.as_ref().unwrap().snapshot().clone();
        let PanelNode::Panel {
            panel_id,
            primary_presentation,
            ..
        } = &snapshot.terminal_contexts[0].tabs[0].root
        else {
            panic!("panel")
        };
        for (n,report) in [
            serde_json::json!({"kind":"state","source":"herdr:pi","agent":"pi","state":"blocked","message":"approval","sequence":"2","agentSessionId":"provider-session"}),
            serde_json::json!({"kind":"session","source":"herdr:pi","agent":"pi","sequence":"1","agentSessionId":"provider-session","sessionStartSource":"startup"}),
        ].into_iter().enumerate() {
            let request=PanelAgentRequest{protocol_version:ProtocolVersion,request_id:WorkbenchRequestId::parse(&format!("{:032x}",810+n)).unwrap(),site_id:snapshot.site_id.clone(),
                action:PanelAgentAction::Report {panel_id:panel_id.clone(),expected_binding:primary_presentation.binding.clone().unwrap(),report:Box::new(serde_json::from_value(report).unwrap())}};
            assert!(matches!(site.panel_agent(request,Instant::now()).unwrap().outcome,PanelAgentOutcome::Reported{disposition:PanelAgentDisposition::Deferred,..}));
        }
        assert_eq!(read(&mut site).1.authority_source, None);
        let now = Instant::now();
        assert!(site.observe_host_process(target, &sample, now, now));
        let (revision, view) = read(&mut site);
        assert_eq!(view.authority_source.as_deref(), Some("herdr:pi"));
        assert_eq!(view.state, PanelAgentState::Blocked);
        assert!(view.session.is_some());
        let next = now + Duration::from_millis(1);
        assert!(site.observe_host_process(target, &sample, next, next));
        assert_eq!(read(&mut site).0, revision);
        sample.processes.pop();
        let next = next + Duration::from_millis(1);
        assert!(site.observe_host_process(target, &sample, next, next));
        let view = read(&mut site).1;
        assert_eq!(view.agent, None);
        assert_eq!(view.authority_source, None);
        assert_eq!(view.session, None);
    }
    #[test]
    fn screen_grace_idle_hold_missing_samples_and_generation_reset() {
        use crate::agent_screen::Detection;
        let (mut site, target, mut sample) = fixture();
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
        let admit = |site: &mut SiteCoordinator, ms, detection| {
            let at = now + Duration::from_millis(ms);
            assert!(site.observe_host_screen(target, &sample, detection, at, at));
        };
        admit(&mut site, 0, Some(working));
        admit(&mut site, 2999, Some(working));
        assert_eq!(read(&mut site).1.state, PanelAgentState::Unknown);
        admit(&mut site, 3000, Some(working));
        let working_view = read(&mut site);
        assert_eq!(working_view.1.state, PanelAgentState::Working);
        admit(&mut site, 3100, None);
        assert_eq!(read(&mut site), working_view);
        for ms in [3200, 3300, 3400] {
            admit(&mut site, ms, Some(idle));
            assert_eq!(read(&mut site), working_view);
        }
        // Transcript skip clears the pending idle timer; ordinary idle starts from zero again.
        admit(&mut site, 3500, Some(Detection { skip: true, ..idle }));
        admit(&mut site, 4200, Some(idle));
        assert_eq!(read(&mut site), working_view);
        admit(&mut site, 4900, Some(idle));
        assert_eq!(read(&mut site).1.state, PanelAgentState::Idle);
        sample.processes[1].creation_time += 1;
        for ms in [5000, 5100, 8000] {
            let at = now + Duration::from_millis(ms);
            assert!(site.observe_host_screen(target, &sample, Some(working), at, at));
            assert_eq!(read(&mut site).1.state, PanelAgentState::Unknown);
        }
        let at = now + Duration::from_millis(8100);
        assert!(site.observe_host_screen(target, &sample, Some(working), at, at));
        assert_eq!(read(&mut site).1.state, PanelAgentState::Working);
    }
    #[test]
    fn manifest_reload_resets_screen_hysteresis_without_changing_process_or_report_facts() {
        use crate::agent_screen::Detection;
        let (mut site, target, sample) = fixture();
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
        for ms in [0, 3000] {
            let at = now + Duration::from_millis(ms);
            assert!(site.observe_host_screen(target, &sample, Some(working), at, at));
        }
        let before = read(&mut site);
        assert_eq!(before.1.state, PanelAgentState::Working);
        let at = now + Duration::from_millis(3100);
        assert!(site.observe_host_screen(target, &sample, Some(idle), at, at));
        assert_eq!(read(&mut site), before);
        site.reset_screen_stabilization();
        assert_eq!(read(&mut site), before);
        assert_eq!(site.process_samples.len(), 1);
        let at = now + Duration::from_millis(3200);
        assert!(site.observe_host_screen(target, &sample, Some(idle), at, at));
        let after = read(&mut site);
        assert_eq!(after.1.agent, before.1.agent);
        assert_eq!(after.1.state, PanelAgentState::Idle);
    }
    #[test]
    fn process_only_query_cannot_consume_screen_republication_after_clear() {
        use crate::agent_screen::Detection;
        let (mut site, target, sample) = fixture();
        let origin = Instant::now() - Duration::from_secs(10);
        let working = Detection {
            state: PanelAgentState::Working,
            skip: false,
            visible_working: true,
            visible_idle: false,
            visible_blocker: false,
        };
        assert!(site.observe_host_screen(target, &sample, Some(working), origin, origin));
        let snapshot = site.workbench.as_ref().unwrap().snapshot().clone();
        let PanelNode::Panel {
            panel_id,
            primary_presentation,
            ..
        } = &snapshot.terminal_contexts[0].tabs[0].root
        else {
            panic!("panel")
        };
        let send = |site: &mut SiteCoordinator, report: serde_json::Value| {
            let request = PanelAgentRequest {
                protocol_version: ProtocolVersion,
                request_id: WorkbenchRequestId::parse(&format!("{:032x}", 901)).unwrap(),
                site_id: snapshot.site_id.clone(),
                action: PanelAgentAction::Report {
                    panel_id: panel_id.clone(),
                    expected_binding: primary_presentation.binding.clone().unwrap(),
                    report: Box::new(serde_json::from_value(report).unwrap()),
                },
            };
            assert!(matches!(
                site.panel_agent(request, Instant::now()).unwrap().outcome,
                PanelAgentOutcome::Reported { .. }
            ));
        };
        send(
            &mut site,
            serde_json::json!({"kind":"session","source":"herdr:pi","agent":"pi","sequence":"1","agentSessionId":"full","sessionStartSource":"startup"}),
        );
        send(
            &mut site,
            serde_json::json!({"kind":"state","source":"herdr:pi","agent":"pi","sequence":"2","agentSessionId":"full","state":"blocked"}),
        );
        let at = Instant::now();
        assert!(site.observe_host_screen(target, &sample, Some(working), at, at));
        assert_eq!(read(&mut site).1.state, PanelAgentState::Blocked);
        send(
            &mut site,
            serde_json::json!({"kind":"clear","source":"herdr:pi","sequence":"3"}),
        );
        let at = Instant::now();
        assert!(site.observe_host_process(target, &sample, at, at));
        assert_eq!(read(&mut site).1.state, PanelAgentState::Unknown);
        let at = Instant::now();
        assert!(site.observe_host_screen(target, &sample, Some(working), at, at));
        let (_, view) = read(&mut site);
        assert_eq!(view.state, PanelAgentState::Working);
        assert_eq!(view.authority_source, None);
    }
}
