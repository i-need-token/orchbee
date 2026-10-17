mod direct_navigation;
mod startup;

use super::*;
use crate::{
    machines::{Draft, ProfileId},
    terminal_input::{
        PointerProbe, PointerSelectionAdmission, PointerSelectionIntent,
        UnavailableTerminalInputPort,
    },
};
struct Witness {
    name: &'static str,
    focus: Option<bool>,
    log: std::rc::Rc<std::cell::RefCell<Vec<String>>>,
    probe: std::rc::Rc<std::cell::RefCell<PointerProbe>>,
}
impl crate::terminal_input::TerminalInputPort for Witness {
    fn forward(
        &mut self,
        _bytes: &[u8],
    ) -> Result<(), crate::terminal_input::TerminalInputUnavailable> {
        Ok(())
    }

    fn pointer_probe(&mut self) -> PointerProbe {
        self.probe.borrow().clone()
    }

    fn apply_client_action(
        &mut self,
        action: crate::client_view::ClientAction,
    ) -> Option<crate::client_view::SessionActionDisposition> {
        if let crate::client_view::ClientAction::FocusPanel(panel) = action {
            let target = self
                .probe
                .borrow()
                .panes
                .iter()
                .find(|pane| pane.record.pane().panel() == &panel && pane.attached)
                .map(|pane| pane.record.clone());
            if let Some(target) = target {
                let mut probe = self.probe.borrow_mut();
                probe.desired = Some(target.pane().clone());
                probe.focus = Some(target);
            }
        }
        Some(crate::client_view::SessionActionDisposition::KeepNavigator)
    }

    fn pointer_selection(
        &mut self,
        _pane: &crate::terminal_session::state::PaneRef,
        intent: PointerSelectionIntent,
    ) -> PointerSelectionAdmission {
        self.log
            .borrow_mut()
            .push(format!("{} selection {intent:?}", self.name));
        PointerSelectionAdmission::Admitted
    }
}

fn witness(
    name: &'static str,
    focus: Option<bool>,
    log: std::rc::Rc<std::cell::RefCell<Vec<String>>>,
) -> Witness {
    Witness {
        name,
        focus,
        log,
        probe: Default::default(),
    }
}
impl TerminalSurface for Witness {
    fn suspend_machine(&mut self) -> Option<bool> {
        self.log.borrow_mut().push(format!("{} suspend", self.name));
        self.focus.take()
    }
    fn resume_machine(&mut self, focus: Option<bool>) {
        self.log
            .borrow_mut()
            .push(format!("{} resume {focus:?}", self.name));
        self.focus = focus;
    }
}

struct PixelSource {
    reads: std::rc::Rc<std::cell::Cell<usize>>,
}
impl crate::terminal_input::TerminalInputPort for PixelSource {
    fn forward(
        &mut self,
        _bytes: &[u8],
    ) -> Result<(), crate::terminal_input::TerminalInputUnavailable> {
        Ok(())
    }
}
impl TerminalSurface for PixelSource {
    fn retained_frame_bank(&self) -> Option<crate::terminal_session::RetainedFrameBank> {
        self.reads.set(self.reads.get() + 1);
        Some(crate::terminal_session::RetainedFrameBank::default())
    }
}
pub(super) fn profile() -> Profile {
    Profile {
        id: ProfileId::parse("11111111111111111111111111111111").unwrap(),
        draft: Draft::parse("remote", "example.invalid", "default").unwrap(),
        enabled: true,
    }
}
pub(super) fn dimensions() -> WindowDimensions {
    WindowDimensions {
        columns: 100,
        rows: 30,
        pixel_height: 0,
        pixel_width: 0,
    }
}
pub(super) fn setup() -> (Manager, ClientView, SurfaceSlot<'static>) {
    let mut view = ClientView::new(100);
    let mut manager = Manager::new(&mut view);
    manager.refresh = Instant::now() + Duration::from_secs(60);
    manager.catalog_ok = true;
    manager.profiles = vec![profile()];
    view.machines.replace(&manager.profiles);
    (
        manager,
        view,
        SurfaceSlot::Owned(Box::new(UnavailableTerminalInputPort)),
    )
}

#[test]
fn targeted_pixel_handoff_uses_only_the_exact_active_or_cached_endpoint() {
    let active_reads = std::rc::Rc::new(std::cell::Cell::new(0));
    let cached_reads = std::rc::Rc::new(std::cell::Cell::new(0));
    let active = PixelSource {
        reads: active_reads.clone(),
    };
    let cached = vec![Cached {
        profile: Some(profile()),
        surface: Box::new(PixelSource {
            reads: cached_reads.clone(),
        }),
        view: ClientView::new(100),
    }];

    assert!(
        retained_frames_for_target(None, &active, &cached, None, true).is_some(),
        "a targeted fresh Local lane inherits the active Local bank"
    );
    assert_eq!(active_reads.get(), 1);
    assert!(
        retained_frames_for_target(None, &active, &cached, Some(&profile()), true).is_some(),
        "a targeted cached remote inherits that exact endpoint's bank"
    );
    assert_eq!(active_reads.get(), 1);
    assert_eq!(cached_reads.get(), 1);

    let mut replaced = profile();
    replaced.draft.target = "replacement.invalid".into();
    assert!(
        retained_frames_for_target(None, &active, &cached, Some(&replaced), true).is_none(),
        "a reused profile id cannot bridge an endpoint replacement"
    );
    assert_eq!(cached_reads.get(), 1);
    assert!(
        retained_frames_for_target(None, &active, &cached, Some(&profile()), false).is_none(),
        "ordinary machine restore retains its surface instead of copying pixels"
    );
    assert_eq!(active_reads.get(), 1);
    assert_eq!(cached_reads.get(), 1);
}
#[test]
fn machine_switch_restores_local_view_and_host_mouse_state_without_reconnecting() {
    let (mut manager, mut view, _) = setup();
    view.settle_pointer_released(true);
    let log = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let mut surface = SurfaceSlot::Owned(Box::new(witness("local", Some(true), log.clone())));
    manager.cached.push(Cached {
        profile: Some(profile()),
        surface: Box::new(witness("remote", None, log.clone())),
        view: ClientView::new(100),
    });
    view.machines.show();
    view.machines.request = Some(Choice::Remote(profile().id).into());
    let change = manager.tick(
        &mut surface,
        &mut view,
        dimensions(),
        Instant::now(),
        &mut |_| {},
    );
    assert!(change.switched);
    assert!(!view.machines.open);
    assert!(view.pointer_released());
    assert_eq!(manager.cached[0].choice(), Choice::Local);
    view.machines.show();
    view.machines.request = Some(Choice::Local.into());
    assert!(
        manager
            .tick(
                &mut surface,
                &mut view,
                dimensions(),
                Instant::now(),
                &mut |_| {},
            )
            .switched
    );
    assert_eq!(view.machines.current, Choice::Local);
    assert_eq!(
        *log.borrow(),
        [
            "local suspend",
            "remote resume Some(true)",
            "remote suspend",
            "local resume Some(true)"
        ]
    );
    assert!(manager.attempt.is_none());
    assert!(view.pointer_released());
}
#[test]
fn machine_cancel_and_failed_attempt_preserve_active_surface_and_ignore_stale_completion() {
    let (mut manager, mut view, mut surface) = setup();
    let cancelled = Arc::new(AtomicU8::new(0));
    let worker = cancelled.clone();
    let task = thread::spawn(move || {
        while worker.load(Ordering::Acquire) == 0 {
            thread::sleep(Duration::from_millis(1));
        }
        Err(io::Error::other("cancelled"))
    });
    manager.attempt = Some(Attempt {
        profile: Some(profile()),
        context: None,
        cancel: cancelled.clone(),
        task: Some(task),
    });
    view.machines.open = false;
    let deadline = Instant::now() + Duration::from_secs(3);
    while manager.attempt.is_some() {
        let change = manager.tick(
            &mut surface,
            &mut view,
            dimensions(),
            Instant::now(),
            &mut |_| {},
        );
        assert!(!change.switched);
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(cancelled.load(Ordering::Acquire), 1);
    assert_eq!(view.machines.current, Choice::Local);
    assert!(view.machines.error.is_none());
    view.machines.show();
    manager.selection_scope = view.machines.selection_scope();
    manager.attempt = Some(Attempt {
        profile: Some(profile()),
        context: None,
        cancel: Arc::new(AtomicU8::new(0)),
        task: Some(thread::spawn(|| Err(io::Error::other("unavailable")))),
    });
    while manager.attempt.is_some() {
        assert!(
            !manager
                .tick(
                    &mut surface,
                    &mut view,
                    dimensions(),
                    Instant::now(),
                    &mut |_| {},
                )
                .switched
        );
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(view.machines.error, Some("failed"));
    assert_eq!(view.machines.current, Choice::Local);
}
#[test]
fn machine_pending_click_is_fenced_by_catalog_identity_before_queue_admission() {
    for change in 0..5 {
        let (mut manager, mut view, mut surface) = setup();
        let request = crate::machine_picker::Selection {
            machine: Choice::Remote(profile().id),
            context: Some(NativeStartupTarget::Context(NativeContextTarget {
                site_id: orchbee_workbench_protocol::SiteId::parse(
                    "11111111111111111111111111111111",
                )
                .unwrap(),
                location: crate::terminal_session::NativeContextLocation::Standalone(
                    orchbee_workbench_protocol::WorkbenchTerminalContextId::parse(
                        "22222222222222222222222222222222",
                    )
                    .unwrap(),
                ),
            })),
        };
        view.machines.request = Some(request.clone());
        let mut updated = profile();
        match change {
            0 => updated.draft.target = "replacement.invalid".into(),
            1 => {
                updated.draft.session = Draft::parse("remote", "example.invalid", "replacement")
                    .unwrap()
                    .session
            }
            2 => updated.enabled = false,
            3 => {}
            _ => updated.draft.label = "renamed".into(),
        }
        manager.refresh_catalog(
            Ok(crate::machines::Catalog {
                revision: 2,
                profiles: if change == 3 {
                    Vec::new()
                } else {
                    vec![updated]
                },
            }),
            &mut surface,
            &mut view,
            dimensions(),
            &mut |_| {},
        );
        assert_eq!(view.machines.request, (change == 4).then_some(request));
        assert_eq!(view.machines.error, (change != 4).then_some("removed"));
        assert!(manager.attempt.is_none());
        assert!(manager.queued.is_none());
    }
}
struct DisconnectedLocal;
#[test]
fn machine_wait_expiry_endpoint_edit_and_escape_preserve_current_surface() {
    let (mut manager, mut view, mut surface) = setup();
    let now = Instant::now();
    view.machines.show();
    view.machines.show();
    manager.selection_scope = view.machines.selection_scope();
    manager.queued = Some(Queued {
        profile: Some(profile()),
        destination: Destination::Startup(None),
        deadline: now,
    });
    assert!(
        !manager
            .tick(&mut surface, &mut view, dimensions(), now, &mut |_| {})
            .switched
    );
    assert_eq!(view.machines.error, Some("failed"));
    assert!(manager.queued.is_none());
    view.machines.show();
    manager.selection_scope = view.machines.selection_scope();
    manager.queued = Some(Queued {
        profile: Some(profile()),
        destination: Destination::Startup(None),
        deadline: now + Duration::from_secs(60),
    });
    manager.profiles[0].draft.target = "replacement.invalid".into();
    assert!(
        !manager
            .tick(&mut surface, &mut view, dimensions(), now, &mut |_| {})
            .switched
    );
    assert_eq!(view.machines.error, Some("removed"));
    assert!(manager.queued.is_none());
    manager.profiles[0] = profile();
    view.machines.show();
    manager.selection_scope = view.machines.selection_scope();
    manager.queued = Some(Queued {
        profile: Some(profile()),
        destination: Destination::Startup(None),
        deadline: now + Duration::from_secs(60),
    });
    view.machines.open = false;
    assert!(
        !manager
            .tick(&mut surface, &mut view, dimensions(), now, &mut |_| {})
            .switched
    );
    assert!(manager.queued.is_none());
    assert!(manager.attempt.is_none());
    assert_eq!(view.machines.current, Choice::Local);
}
impl crate::terminal_input::TerminalInputPort for DisconnectedLocal {
    fn forward(
        &mut self,
        _bytes: &[u8],
    ) -> Result<(), crate::terminal_input::TerminalInputUnavailable> {
        Ok(())
    }
}
impl TerminalSurface for DisconnectedLocal {
    fn reusable_machine(&self) -> bool {
        false
    }
}
#[test]
fn forced_catalog_fallback_cancels_once_through_the_old_surface_before_replace() {
    let (mut manager, mut view, _) = setup();
    let log = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let tab = orchbee_workbench_protocol::WorkbenchTabId::parse("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        .unwrap();
    let pane = |panel: &str| {
        crate::terminal_session::state::PaneRef::new(
            crate::terminal_session::state::PaneKey::new(
                crate::terminal_session::state::AttachmentKey::new(1, tab.clone()),
                orchbee_workbench_protocol::WorkbenchPanelId::parse(panel).unwrap(),
            ),
            0,
        )
    };
    let previous = pane("11111111111111111111111111111111");
    let target = pane("22222222222222222222222222222222");
    let remote_probe = std::rc::Rc::new(std::cell::RefCell::new(PointerProbe {
        focus: Some(previous.clone()),
        desired: Some(previous.pane().clone()),
        input_ready: true,
        selection_ready: true,
        panes: vec![
            crate::terminal_input::PointerPane {
                right_click_reporting: false,
                right_click: Default::default(),
                record: previous,
                attached: true,
                controlling: Some(true),
                frame_available: true,
            },
            crate::terminal_input::PointerPane {
                right_click_reporting: false,
                right_click: Default::default(),
                record: target.clone(),
                attached: true,
                controlling: Some(true),
                frame_available: false,
            },
        ],
        tabs: vec![crate::terminal_input::PointerTab {
            id: tab,
            position: 1,
        }],
        ..PointerProbe::default()
    }));
    manager.current = Some(profile());
    manager.cached.push(Cached {
        profile: None,
        surface: Box::new(witness("local", None, log.clone())),
        view: ClientView::new(100),
    });
    let mut surface = SurfaceSlot::Owned(Box::new(Witness {
        name: "remote",
        focus: Some(true),
        log: log.clone(),
        probe: remote_probe.clone(),
    }));
    let content = ratatui::layout::Rect::new(0, 1, 10, 5);
    let mut hits = crate::tui_pointer::HitMapBuilder::new();
    hits.record(
        crate::tui_pointer::HitLayer::Foreground,
        content,
        crate::tui_pointer::HitTarget::Pane {
            pane: target.clone(),
            content,
        },
    );
    let mut pointer = crate::tui_pointer::PointerState::default();
    pointer.set_capture(true);
    pointer.publish(hits.finish());
    let at = Instant::now();
    pointer.event(
        &mut view,
        surface.active(),
        at,
        crate::terminal_input::MouseEvent {
            action: crate::terminal_input::MouseAction::Press,
            button: crate::terminal_input::MouseButton::Left,
            mods: crate::client_view::Modifiers::NONE,
            column: 2,
            row: 2,
        },
    );
    assert!(
        log.borrow()
            .iter()
            .all(|entry| !entry.contains(" selection ")),
        "Pending focus emits nothing before its frame is Ready"
    );
    {
        let mut probe = remote_probe.borrow_mut();
        probe.frame = Some(crate::terminal_input::PointerFrameFacts {
            tracking: orchbee_terminal_protocol::MouseTracking::None,
            format: orchbee_terminal_protocol::MouseFormat::Sgr,
            cell_pixel: (0, 0),
            alternate_screen: false,
            alternate_scroll: true,
            application_cursor: false,
        });
        probe
            .panes
            .iter_mut()
            .find(|pane| pane.record == target)
            .unwrap()
            .frame_available = true;
    }
    pointer.tick(
        &mut view,
        surface.active(),
        at + Duration::from_millis(1),
        None,
    );
    assert!(
        log.borrow()
            .iter()
            .any(|entry| entry.starts_with("remote selection Start")),
        "the real pointer path must first admit Down on the old surface"
    );

    let mut before_switch = |old: &mut dyn crate::terminal_input::TerminalInputPort| {
        pointer.invalidate_map_with_port(old);
    };
    let mut disabled = profile();
    disabled.enabled = false;

    let change = manager.refresh_catalog(
        Ok(crate::machines::Catalog {
            revision: 2,
            profiles: vec![disabled],
        }),
        &mut surface,
        &mut view,
        dimensions(),
        &mut before_switch,
    );

    assert!(change.switched);
    assert!(manager.current.is_none());
    let selections: Vec<_> = log
        .borrow()
        .iter()
        .filter(|entry| entry.contains(" selection "))
        .cloned()
        .collect();
    assert_eq!(selections.len(), 2);
    assert!(selections[0].starts_with("remote selection Start"));
    assert_eq!(selections[1], "remote selection Cancel");
    assert!(selections.iter().all(|entry| !entry.starts_with("local ")));
    let log = log.borrow();
    let cancel = log
        .iter()
        .position(|entry| entry == "remote selection Cancel")
        .unwrap();
    let suspend = log
        .iter()
        .position(|entry| entry == "remote suspend")
        .unwrap();
    assert!(cancel < suspend, "Cancel must precede old-surface suspend");
}

#[test]
fn machine_catalog_failure_preserves_active_identity_but_disable_returns_local() {
    let (mut manager, mut view, mut surface) = setup();
    drop(surface.replace(Box::new(DisconnectedLocal)));
    manager.install(
        Cached {
            profile: Some(profile()),
            surface: Box::new(UnavailableTerminalInputPort),
            view: ClientView::new(100),
        },
        &mut surface,
        &mut view,
        dimensions(),
        true,
        &mut |_| {},
    );
    let change = manager.refresh_catalog(
        Err(crate::machines::Error::InvalidCatalog),
        &mut surface,
        &mut view,
        dimensions(),
        &mut |_| {},
    );
    assert!(!change.switched);
    assert_eq!(view.machines.current, Choice::Remote(profile().id));
    assert_eq!(view.machines.rows.len(), 1);
    let mut disabled = profile();
    disabled.enabled = false;
    let change = manager.refresh_catalog(
        Ok(crate::machines::Catalog {
            revision: 2,
            profiles: vec![disabled],
        }),
        &mut surface,
        &mut view,
        dimensions(),
        &mut |_| {},
    );
    assert!(change.switched);
    assert_eq!(view.machines.current, Choice::Local);
    assert!(manager.cached.is_empty());
    assert!(!view.machines.open);
    assert_eq!(
        view.notice(),
        Some(crate::client_view::Notice::WorkspaceNavigationUnavailable)
    );
    view.machines.show();
    view.machines.request = Some(Choice::Local.into());
    manager.tick(
        &mut surface,
        &mut view,
        dimensions(),
        Instant::now(),
        &mut |_| {},
    );
    assert!(
        !view.machines.open,
        "even a disconnected Local surface must remain reachable"
    );
}
