//! Controlled admission traces for direct sidebar navigation and its latest-intent slot.
use super::super::*;
use super::{dimensions, setup};
use crate::{machine_picker::Selection, terminal_session::NativeContextLocation};
use orchbee_workbench_protocol::{SiteId, WorkbenchTerminalContextId, WorkbenchWorkspaceId};
use std::{cell::RefCell, rc::Rc};

struct Navigation {
    current: NativeContextTarget,
    pending: Option<NativeContextTarget>,
    admitted: Vec<NativeContextTarget>,
    unavailable: bool,
}
struct Surface(Rc<RefCell<Navigation>>);
impl crate::terminal_input::TerminalInputPort for Surface {
    fn forward(&mut self, _: &[u8]) -> Result<(), crate::terminal_input::TerminalInputUnavailable> {
        Ok(())
    }
}
impl TerminalSurface for Surface {
    fn navigate_context(&mut self, target: &NativeContextTarget) -> ContextNavigationAdmission {
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
fn targets() -> Vec<NativeContextTarget> {
    (1..=3)
        .map(|n| NativeContextTarget {
            site_id: SiteId::parse(&format!("{:032x}", 9)).unwrap(),
            location: NativeContextLocation::Workspace {
                workspace_id: WorkbenchWorkspaceId::parse(&format!("{n:032x}")).unwrap(),
                terminal_context_id: WorkbenchTerminalContextId::parse(&format!("{:032x}", n + 3))
                    .unwrap(),
            },
        })
        .collect()
}
fn request(view: &mut ClientView, target: &NativeContextTarget) {
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
    let cancel = Arc::new(AtomicU8::new(0));
    manager.attempt = Some(Attempt {
        profile: Some(super::profile()),
        context: Some(targets[1].clone().into()),
        cancel: cancel.clone(),
        task: Some(thread::spawn(move || {
            receive.recv_timeout(Duration::from_secs(3)).unwrap();
            Err(io::Error::other("old completion"))
        })),
    });
    request(&mut view, &targets[2]);
    manager.tick(&mut surface, &mut view, dimensions(), now, &mut |_| {});
    assert_eq!(cancel.load(Ordering::Acquire), 1);
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
