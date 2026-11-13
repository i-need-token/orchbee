//! Controlled admission traces for direct sidebar navigation and its latest-intent slot.
use super::super::*;
use super::{dimensions, setup};
use crate::workbench_client::ClientContextLocation;
use orchbee_workbench_client::test_support::client_view::ClientViewTestAccess;
use orchbee_workbench_client::test_support::machine_agents::RowTestAccess;
use orchbee_workbench_client::test_support::machine_agents::TargetTestAccess;
use orchbee_workbench_client::test_support::machine_picker::PickerTestAccess;
use orchbee_workbench_client::test_support::machine_picker::Selection;
use orchbee_workbench_protocol::{SessionId, WorkbenchTerminalContextId, WorkbenchWorkspaceId};
use std::{cell::RefCell, rc::Rc, thread};

struct Navigation {
    current: ClientContextTarget,
    pending: Option<ClientContextTarget>,
    admitted: Vec<ClientContextTarget>,
    unavailable: bool,
}
struct Surface(Rc<RefCell<Navigation>>);
impl crate::terminal_input::TerminalInputPort for Surface {
    fn forward(
        &mut self,
        _: &[u8],
    ) -> Result<(), orchbee_workbench_client::test_support::terminal_input::TerminalInputUnavailable>
    {
        Ok(())
    }
}
impl TerminalSurface for Surface {
    fn navigate_context(&mut self, target: &ClientContextTarget) -> ContextNavigationAdmission {
        let mut state = self.0.borrow_mut();
        if state.unavailable {
            ContextNavigationAdmission::Unavailable
        } else if state.pending.is_some() {
            ContextNavigationAdmission::Busy
        } else if state.current == *target {
            ContextNavigationAdmission::Current
        } else {
            state.pending = Some(target.clone());
            state.admitted.push(target.clone());
            ContextNavigationAdmission::Admitted
        }
    }
}
fn targets() -> Vec<ClientContextTarget> {
    (1..=3)
        .map(|n| ClientContextTarget {
            session_id: SessionId::parse(&format!("{:032x}", 9)).unwrap(),
            location: ClientContextLocation::Workspace {
                workspace_id: WorkbenchWorkspaceId::parse(&format!("{n:032x}")).unwrap(),
                terminal_context_id: WorkbenchTerminalContextId::parse(&format!("{:032x}", n + 3))
                    .unwrap(),
            },
        })
        .collect()
}
fn request(view: &mut ClientView, target: &ClientContextTarget) {
    assert!(view.machines.activate_sidebar_workspace(&Selection {
        machine: Choice::Local,
        context: Some(target.clone().into()),
    }));
    assert!(!view.machines.open);
}
fn prepare() -> (
    Manager,
    ClientView,
    SurfaceSlot<'static>,
    Rc<RefCell<Navigation>>,
) {
    let (manager, mut view, _) = setup();
    let targets = targets();
    view.machines.observe_agent_sidebar(true);
    view.machines.observe_local(
        targets
            .iter()
            .map(|target| Context {
                target: target.clone(),
                project_id: None,
                project: None,
                workspace: Some("same label".into()),
            })
            .collect(),
    );
    let state = Rc::new(RefCell::new(Navigation {
        current: targets[0].clone(),
        pending: None,
        admitted: Vec::new(),
        unavailable: false,
    }));
    (
        manager,
        view,
        SurfaceSlot::Owned(Box::new(Surface(state.clone()))),
        state,
    )
}
fn settle(state: &RefCell<Navigation>) {
    let mut state = state.borrow_mut();
    state.current = state.pending.take().unwrap();
}
fn tick(manager: &mut Manager, view: &mut ClientView, surface: &mut SurfaceSlot<'_>, now: Instant) {
    let change = manager.tick(surface, view, dimensions(), now, &mut |_| {});
    assert!(
        !change.switched,
        "same-machine navigation keeps the original surface"
    );
    assert!(!view.machines.open);
    assert!(manager.attempt.is_none());
    assert!(manager.candidate.is_none());
    assert!(manager.cached.is_empty());
}

#[test]
fn direct_workspace_serializes_accepted_work_and_admits_only_the_latest_waiting_target() {
    let targets = targets();
    for last in [&targets[0], &targets[2]] {
        let (mut manager, mut view, mut surface, state) = prepare();
        let now = Instant::now();
        request(&mut view, &targets[0]);
        tick(&mut manager, &mut view, &mut surface, now);
        assert!(state.borrow().admitted.is_empty(), "current is stable");
        request(&mut view, &targets[1]);
        tick(&mut manager, &mut view, &mut surface, now);
        assert_eq!(state.borrow().admitted, [targets[1].clone()]);
        request(&mut view, &targets[2]);
        tick(&mut manager, &mut view, &mut surface, now);
        request(&mut view, last);
        tick(&mut manager, &mut view, &mut surface, now);
        assert!(view.machines.busy);
        assert_eq!(state.borrow().admitted.len(), 1);
        settle(&state);
        tick(&mut manager, &mut view, &mut surface, now);
        assert_eq!(state.borrow().admitted, [targets[1].clone(), last.clone()]);
        assert!(!view.machines.busy);
        assert!(!view.machines.direct_selection_pending());
        settle(&state);
        request(&mut view, last);
        tick(&mut manager, &mut view, &mut surface, now);
        assert_eq!(state.borrow().current, *last);
        assert_eq!(
            state.borrow().admitted.len(),
            2,
            "repeated current is a no-op"
        );
    }
}

#[test]
fn direct_workspace_cancel_or_explicit_picker_retires_only_unadmitted_intent() {
    for explicit in [false, true] {
        let (mut manager, mut view, mut surface, state) = prepare();
        let targets = targets();
        let now = Instant::now();
        request(&mut view, &targets[1]);
        tick(&mut manager, &mut view, &mut surface, now);
        request(&mut view, &targets[2]);
        tick(&mut manager, &mut view, &mut surface, now);
        if explicit {
            view.machines.show_workspaces();
        } else {
            view.machines.cancel_selection();
        }
        manager.tick(&mut surface, &mut view, dimensions(), now, &mut |_| {});
        assert!(manager.queued.is_none());
        assert!(!view.machines.busy);
        settle(&state);
        manager.tick(&mut surface, &mut view, dimensions(), now, &mut |_| {});
        assert_eq!(state.borrow().admitted, [targets[1].clone()]);
        assert_eq!(state.borrow().current, targets[1]);
        assert_eq!(view.machines.open, explicit);
    }
}

#[test]
fn direct_workspace_failure_and_wait_deadline_keep_current_surface_without_modal() {
    for unavailable in [false, true] {
        let (mut manager, mut view, mut surface, state) = prepare();
        let targets = targets();
        let now = Instant::now();
        state.borrow_mut().unavailable = unavailable;
        if !unavailable {
            state.borrow_mut().pending = Some(targets[0].clone());
        }
        request(&mut view, &targets[1]);
        tick(&mut manager, &mut view, &mut surface, now);
        if !unavailable {
            // Hold catalog refresh out of this controlled deadline trace.
            manager.refresh = now + Duration::from_secs(120);
            tick(
                &mut manager,
                &mut view,
                &mut surface,
                now + Duration::from_secs(60),
            );
        }
        assert_eq!(view.machines.error, Some("failed"));
        assert_eq!(
            view.notice(),
            Some(crate::client_view::Notice::WorkspaceNavigationUnavailable)
        );
        assert!(!view.machines.direct_selection_pending());
        assert!(!view.machines.busy);
        assert_eq!(state.borrow().current, targets[0]);
        assert!(state.borrow().admitted.is_empty());
        state.borrow_mut().unavailable = false;
        state.borrow_mut().pending = None;
        request(&mut view, &targets[1]);
        tick(&mut manager, &mut view, &mut surface, now);
        assert!(
            view.notice().is_none(),
            "retry clears only its prior failure notice"
        );
        assert_eq!(state.borrow().admitted, [targets[1].clone()]);
    }
}

#[test]
fn superseded_connection_completion_cannot_fail_or_replace_new_direct_navigation() {
    let (mut manager, mut view, mut surface, state) = prepare();
    let targets = targets();
    let now = Instant::now();
    request(&mut view, &targets[1]);
    manager.selection_scope = view.machines.selection_scope();
    view.machines.request.take();
    let (release, receive) = std::sync::mpsc::channel();
    let cancel = Arc::new(OwnedCancellation::new().unwrap());
    let (task, result) = spawn_attempt(crate::workbench_client::UiWakeup::current(), move || {
        receive.recv_timeout(Duration::from_secs(3)).unwrap();
        Err(io::Error::other("old completion"))
    })
    .unwrap();
    manager.attempt = Some(Attempt {
        profile: Some(super::profile()),
        context: Some(targets[1].clone().into()),
        cancel: cancel.clone(),
        task,
        result,
    });
    request(&mut view, &targets[2]);
    manager.tick(&mut surface, &mut view, dimensions(), now, &mut |_| {});
    assert!(cancel.is_cancelled());
    assert!(manager.queued.is_some());
    assert!(state.borrow().admitted.is_empty());
    release.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while manager.attempt.is_some() {
        manager.tick(&mut surface, &mut view, dimensions(), now, &mut |_| {});
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(state.borrow().admitted, [targets[2].clone()]);
    assert!(view.machines.error.is_none());
    assert!(view.notice().is_none());
    assert!(!view.machines.open);
    assert!(!view.machines.direct_selection_pending());
}

struct AgentSurface(Rc<RefCell<AgentNavigation>>);
struct AgentNavigation {
    current: crate::machine_agents::Target,
    pending: Option<crate::machine_agents::Target>,
    admitted: Vec<crate::machine_agents::Target>,
    unavailable: bool,
}
impl crate::terminal_input::TerminalInputPort for AgentSurface {
    fn forward(
        &mut self,
        _: &[u8],
    ) -> Result<(), orchbee_workbench_client::test_support::terminal_input::TerminalInputUnavailable>
    {
        Ok(())
    }
}
impl TerminalSurface for AgentSurface {
    fn navigate_agent(
        &mut self,
        target: &crate::machine_agents::Target,
    ) -> ContextNavigationAdmission {
        let mut state = self.0.borrow_mut();
        if state.unavailable {
            ContextNavigationAdmission::Unavailable
        } else if state.pending.is_some() {
            ContextNavigationAdmission::Busy
        } else if state.current == *target {
            ContextNavigationAdmission::Current
        } else {
            state.pending = Some(target.clone());
            state.admitted.push(target.clone());
            ContextNavigationAdmission::Admitted
        }
    }
}
fn agent_checkpoint() -> Arc<orchbee_workbench_client::test_support::machine_agents::Checkpoint> {
    orchbee_workbench_client::test_support::machine_agents::checkpoint(
        [23, 24, 25]
            .into_iter()
            .map(orchbee_workbench_client::test_support::machine_agents::row)
            .collect(),
    )
}
fn prepare_agents() -> (
    Manager,
    ClientView,
    SurfaceSlot<'static>,
    Rc<RefCell<AgentNavigation>>,
    Vec<crate::machine_agents::Target>,
) {
    let (manager, mut view, _) = setup();
    let checkpoint = agent_checkpoint();
    let targets: Vec<_> = checkpoint.iter().map(|row| row.target().clone()).collect();
    view.machines.observe_agent_sidebar(true);
    view.machines.observe_local(vec![Context {
        target: targets[0].context().clone(),
        project_id: None,
        project: None,
        workspace: Some("same label".into()),
    }]);
    view.machines.observe_agents(vec![(
        Choice::Local,
        orchbee_workbench_client::test_support::machine_agents::Observation::Ready(checkpoint),
    )]);
    let state = Rc::new(RefCell::new(AgentNavigation {
        current: targets[0].clone(),
        pending: None,
        admitted: Vec::new(),
        unavailable: false,
    }));
    (
        manager,
        view,
        SurfaceSlot::Owned(Box::new(AgentSurface(state.clone()))),
        state,
        targets,
    )
}
// The read lane's verification is a separate contract; these traces begin at its typed result.
fn verified_agent(
    manager: &mut Manager,
    view: &mut ClientView,
    target: &crate::machine_agents::Target,
    now: Instant,
) {
    // AgentSurface models admission without a machine observation source, so each Manager tick
    // withdraws its rows. Publish the next click's checkpoint through the normal Picker guard.
    view.machines.observe_agents(vec![(
        Choice::Local,
        orchbee_workbench_client::test_support::machine_agents::Observation::Ready(
            agent_checkpoint(),
        ),
    )]);
    assert!(view.machines.activate_sidebar_agent(&Selection {
        machine: Choice::Local,
        context: Some(ClientStartupTarget::Agent(Box::new(target.clone())))
    }));
    assert_eq!(
        view.machines.request.take().unwrap(),
        Selection {
            machine: Choice::Local,
            context: Some(ClientStartupTarget::Agent(Box::new(target.clone()))),
        }
    );
    manager.cancel();
    manager.selection_scope = view.machines.selection_scope();
    manager.queued = Some(Queued {
        profile: None,
        destination: Destination::VerifiedAgent(Box::new(target.clone())),
        deadline: now + Duration::from_secs(60),
        paused: None,
    });
}

#[test]
fn direct_agent_reuses_session_serializes_admitted_switch_and_keeps_latest_waiting_target() {
    let (mut manager, mut view, mut surface, state, targets) = prepare_agents();
    let now = Instant::now();
    verified_agent(&mut manager, &mut view, &targets[0], now);
    tick(&mut manager, &mut view, &mut surface, now);
    assert!(state.borrow().admitted.is_empty());
    assert!(
        !view.machines.activate_sidebar_agent(&Selection {
            machine: Choice::Local,
            context: Some(ClientStartupTarget::Agent(Box::new(targets[1].clone()))),
        }),
        "the source-less fixture cannot click a withdrawn Agent row"
    );
    verified_agent(&mut manager, &mut view, &targets[1], now);
    tick(&mut manager, &mut view, &mut surface, now);
    assert_eq!(state.borrow().admitted, [targets[1].clone()]);
    verified_agent(&mut manager, &mut view, &targets[2], now);
    tick(&mut manager, &mut view, &mut surface, now);
    verified_agent(&mut manager, &mut view, &targets[0], now);
    tick(&mut manager, &mut view, &mut surface, now);
    assert!(view.machines.busy);
    assert_eq!(state.borrow().admitted.len(), 1);
    {
        let mut state = state.borrow_mut();
        state.current = state.pending.take().unwrap();
    }
    tick(&mut manager, &mut view, &mut surface, now);
    assert_eq!(
        state.borrow().admitted,
        [targets[1].clone(), targets[0].clone()]
    );
    assert!(!view.machines.busy);
    assert!(!view.machines.direct_selection_pending());
}

#[test]
fn direct_agent_cancel_search_failure_and_deadline_never_replace_session_or_open_navigator() {
    for outcome in 0..4 {
        let (mut manager, mut view, mut surface, state, targets) = prepare_agents();
        let now = Instant::now();
        verified_agent(&mut manager, &mut view, &targets[1], now);
        match outcome {
            0 => view.machines.cancel_selection(),
            1 => view.machines.show_workspaces(),
            2 => state.borrow_mut().unavailable = true,
            _ => state.borrow_mut().pending = Some(targets[0].clone()),
        }
        manager.tick(&mut surface, &mut view, dimensions(), now, &mut |_| {});
        if outcome == 3 {
            manager.refresh = now + Duration::from_secs(120);
            manager.tick(
                &mut surface,
                &mut view,
                dimensions(),
                now + Duration::from_secs(60),
                &mut |_| {},
            );
        }
        assert_eq!(view.machines.open, outcome == 1);
        assert!(state.borrow().admitted.is_empty());
        assert_eq!(state.borrow().current, targets[0]);
        assert!(
            manager.candidate.is_none() && manager.attempt.is_none() && manager.cached.is_empty()
        );
        assert!(!view.machines.busy && !view.machines.direct_selection_pending());
        assert_eq!(view.notice().is_some(), outcome >= 2);
    }
}

#[test]
fn cancelled_verification_result_cannot_switch_or_reopen_explicit_navigator() {
    for explicit in [false, true] {
        let (mut manager, mut view, mut surface, state, targets) = prepare_agents();
        let now = Instant::now();
        verified_agent(&mut manager, &mut view, &targets[1], now);
        manager.queued.take();
        let target = Box::new(targets[1].clone());
        let (task, result) =
            spawn_attempt(crate::workbench_client::UiWakeup::current(), move || {
                Ok(PreparedSelection::Agent {
                    target,
                    deadline: now + Duration::from_secs(60),
                })
            })
            .unwrap();
        while !task.is_finished() {
            thread::yield_now();
        }
        let cancel = Arc::new(OwnedCancellation::new().unwrap());
        manager.attempt = Some(Attempt {
            profile: None,
            context: Some(ClientStartupTarget::Agent(Box::new(targets[1].clone()))),
            cancel: cancel.clone(),
            task,
            result,
        });
        if explicit {
            view.machines.show_workspaces();
        } else {
            view.machines.cancel_selection();
        }
        manager.tick(&mut surface, &mut view, dimensions(), now, &mut |_| {});
        assert!(cancel.is_cancelled());
        assert!(state.borrow().admitted.is_empty());
        assert_eq!(view.machines.open, explicit);
        assert!(
            manager.attempt.is_none() && manager.candidate.is_none() && manager.queued.is_none()
        );
        assert!(view.notice().is_none());
    }
}
