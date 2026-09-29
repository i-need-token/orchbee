//! UI-owned selection over read-only saved-machine observers.
use super::{SurfaceSlot, TerminalSurface, terminal_geometry};
use crate::{
    client_view::ClientView,
    local_client::LocalClient,
    machine_discovery::{Connection, Context, Discovery, display_label, same_endpoint},
    machine_picker::Choice,
    machines::{Profile, Store},
    terminal_device::WindowDimensions,
    terminal_session::{
        NativeContextTarget, NativeStartupTarget, TerminalIntent, TerminalSession,
        TerminalSessionView, TerminalStatus,
    },
};
use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

struct Attempt {
    profile: Option<Profile>,
    context: Option<NativeStartupTarget>,
    cancel: Arc<AtomicU8>,
    task: Option<JoinHandle<io::Result<LocalClient>>>,
}
impl Drop for Attempt {
    fn drop(&mut self) {
        self.cancel.store(1, Ordering::Release);
        if let Some(task) = self.task.take() {
            let _settled = task.join();
        }
    }
}
struct Candidate {
    profile: Option<Profile>,
    session: TerminalSession,
    deadline: Instant,
}
struct Cached {
    profile: Option<Profile>,
    surface: Box<dyn TerminalSurface>,
    view: ClientView,
}
impl Cached {
    fn choice(&self) -> Choice {
        choice(self.profile.as_ref())
    }
}
fn choice(profile: Option<&Profile>) -> Choice {
    profile.map_or(Choice::Local, |p| Choice::Remote(p.id.clone()))
}
struct Queued {
    profile: Option<Profile>,
    context: Option<NativeStartupTarget>,
    deadline: Instant,
}
#[derive(Default)]
pub(super) struct Change {
    pub(super) redraw: bool,
    pub(super) switched: bool,
}
pub(super) struct Manager {
    profiles: Vec<Profile>,
    catalog_ok: bool,
    refresh: Instant,
    current: Option<Profile>,
    cached: Vec<Cached>,
    template: ClientView,
    attempt: Option<Attempt>,
    candidate: Option<Candidate>,
    queued: Option<Queued>,
    discovery: Discovery,
    agents: crate::machine_agents::Collector,
}
impl Manager {
    pub(super) fn new(view: &mut ClientView) -> Self {
        view.machines.available = true;
        Self {
            profiles: Vec::new(),
            catalog_ok: false,
            refresh: Instant::now(),
            current: None,
            cached: Vec::new(),
            template: view.clone(),
            attempt: None,
            candidate: None,
            queued: None,
            discovery: Discovery::default(),
            agents: crate::machine_agents::Collector::default(),
        }
    }
    fn valid(&self, profile: &Profile) -> bool {
        self.catalog_ok
            && self
                .profiles
                .iter()
                .any(|p| p.enabled && same_endpoint(p, profile))
    }
    fn cancel(&mut self) {
        if let Some(attempt) = &self.attempt {
            attempt.cancel.store(1, Ordering::Release);
        }
        self.candidate = None;
        self.queued = None;
    }
    pub(super) fn tick(
        &mut self,
        surface: &mut SurfaceSlot<'_>,
        view: &mut ClientView,
        dimensions: WindowDimensions,
        now: Instant,
    ) -> Change {
        let mut change = Change::default();
        if now >= self.refresh {
            self.refresh = now + Duration::from_secs(1);
            change = self.refresh_catalog(
                Store::configured().and_then(|store| store.read()),
                surface,
                view,
                dimensions,
            );
            let local = if self.current.is_none() {
                navigation(surface.active())
            } else {
                self.cached
                    .iter()
                    .find(|c| c.profile.is_none())
                    .map_or_else(Vec::new, |c| navigation(c.surface.as_ref()))
            };
            change.redraw |= view.machines.observe_local(local);
        }
        let active_context = active_context(surface.active());
        change.redraw |= view.machines.current_context != active_context;
        view.machines.current_context = active_context;
        self.discovery.tick(now);
        change.redraw |= view.machines.observe(&self.discovery.observations(now));
        let mut agent_sources = Vec::new();
        if view.machines.wants_agents() {
            if let Some(connection) = self.local_source(surface) {
                agent_sources.push(crate::machine_agents::Source {
                    choice: Choice::Local,
                    connection,
                });
            }
            for profile in &self.profiles {
                if let Some(connection) = self.discovery.connection(profile, now) {
                    agent_sources.push(crate::machine_agents::Source {
                        choice: Choice::Remote(profile.id.clone()),
                        connection,
                    });
                }
            }
        }
        change.redraw |= view
            .machines
            .observe_agents(self.agents.tick(agent_sources, now));
        if !view.machines.open {
            self.cancel();
        }
        if let Some(request) = view.machines.request.take() {
            self.cancel();
            if request.context.is_none()
                && request.machine == choice(self.current.as_ref())
                && (request.machine == Choice::Local || surface.active().reusable_machine())
            {
                view.machines.open = false;
            } else if request.context.is_none()
                && self.restore(&request.machine, surface, view, dimensions, true)
            {
                change.switched = true;
            } else {
                let profile = match &request.machine {
                    Choice::Local => Some(None),
                    Choice::Remote(id) => self
                        .profiles
                        .iter()
                        .find(|p| &p.id == id && p.enabled)
                        .filter(|_| self.catalog_ok)
                        .cloned()
                        .map(Some),
                };
                if let Some(profile) = profile {
                    if let Some(profile) = &profile {
                        self.discovery.retry(&profile.id, now);
                    }
                    self.queued = Some(Queued {
                        profile,
                        context: request.context,
                        deadline: now + Duration::from_secs(60),
                    });
                    view.machines.error = None;
                } else {
                    view.machines.error = Some("removed");
                }
            }
            change.redraw = true;
        }
        if self
            .attempt
            .as_ref()
            .is_some_and(|a| a.task.as_ref().is_some_and(|t| t.is_finished()))
        {
            let mut attempt = self.attempt.take().expect("finished attempt");
            let result = attempt
                .task
                .take()
                .expect("attempt task")
                .join()
                .unwrap_or_else(|_| Err(io::Error::other("machine connection worker failed")));
            if attempt.cancel.load(Ordering::Acquire) == 0
                && view.machines.open
                && attempt.profile.as_ref().is_none_or(|p| self.valid(p))
            {
                let startup_deadline = now + Duration::from_secs(30);
                let candidate = result.and_then(|mut client| {
                    let geometry = terminal_geometry(dimensions, &self.template, None)?
                        .ok_or_else(|| io::Error::other("viewport too small"))?;
                    client.set_submission_deadline(Some(startup_deadline));
                    match attempt.context.clone() {
                        Some(context) => TerminalSession::start_target(client, context, geometry),
                        None => {
                            TerminalSession::start(client, TerminalIntent::Standalone, geometry)
                        }
                    }
                });
                match candidate {
                    Ok(session) => {
                        self.candidate = Some(Candidate {
                            profile: attempt.profile.clone(),
                            session,
                            deadline: startup_deadline,
                        })
                    }
                    Err(_) => view.machines.error = Some("failed"),
                }
            }
            change.redraw = true;
        }
        if self.attempt.is_none()
            && let Some(queued) = self.queued.take()
        {
            if queued.profile.as_ref().is_some_and(|p| !self.valid(p)) {
                view.machines.error = Some("removed");
            } else if now >= queued.deadline
                || queued
                    .profile
                    .as_ref()
                    .is_some_and(|p| self.discovery.failed(p))
            {
                view.machines.error = Some("failed");
            } else if let Some(connection) = match &queued.profile {
                Some(profile) => self.discovery.connection(profile, now),
                None => self.local_source(surface),
            } {
                let cancel = Arc::new(AtomicU8::new(0));
                let worker_cancel = cancel.clone();
                match thread::Builder::new()
                    .name("orchbee-machine-connect".into())
                    .spawn(move || connection.reconnect(queued.deadline, &worker_cancel))
                {
                    Ok(task) => {
                        self.attempt = Some(Attempt {
                            profile: queued.profile,
                            context: queued.context,
                            cancel,
                            task: Some(task),
                        })
                    }
                    Err(_) => view.machines.error = Some("failed"),
                }
            } else if queued.profile.is_none() {
                view.machines.error = Some("failed");
            } else {
                self.queued = Some(queued);
            }
            change.redraw |= self.queued.is_none();
        }
        if let Some(mut candidate) = self.candidate.take() {
            candidate.session.tick();
            let drawable = candidate.session.startup_draw_ready();
            let failed = now >= candidate.deadline
                || (!drawable
                    && matches!(
                        candidate.session.status(),
                        TerminalStatus::Unavailable | TerminalStatus::OutcomeUnknown
                    ));
            if !view.machines.open || candidate.profile.as_ref().is_some_and(|p| !self.valid(p)) {
                view.machines.error = Some("removed");
                change.redraw = true;
            } else if failed {
                view.machines.error = Some("failed");
                change.redraw = true;
            } else if drawable {
                let next = Cached {
                    profile: candidate.profile,
                    surface: Box::new(candidate.session),
                    view: self.template.clone(),
                };
                let keep_old = next.choice() != choice(self.current.as_ref());
                self.install(next, surface, view, dimensions, keep_old);
                change.switched = true;
            } else {
                self.candidate = Some(candidate);
            }
        }
        let busy = self.attempt.is_some() || self.candidate.is_some() || self.queued.is_some();
        change.redraw |= busy != view.machines.busy || change.switched;
        view.machines.busy = busy;
        change
    }
    fn refresh_catalog(
        &mut self,
        catalog: Result<crate::machines::Catalog, crate::machines::Error>,
        surface: &mut SurfaceSlot<'_>,
        view: &mut ClientView,
        dimensions: WindowDimensions,
    ) -> Change {
        let mut change = Change::default();
        let before = view.machines.clone();
        match catalog {
            Ok(catalog) => {
                if self.profiles.iter().filter(|p| p.enabled).any(|old| {
                    !catalog
                        .profiles
                        .iter()
                        .any(|p| p.enabled && same_endpoint(p, old))
                }) {
                    self.agents.clear();
                }
                // Input is decoded before this refresh. A pending click still
                // belongs to the previously painted endpoint, never its replacement.
                if view
                    .machines
                    .request
                    .as_ref()
                    .is_some_and(|request| match &request.machine {
                        Choice::Local => false,
                        Choice::Remote(id) => self
                            .profiles
                            .iter()
                            .find(|p| &p.id == id)
                            .is_none_or(|old| {
                                !catalog
                                    .profiles
                                    .iter()
                                    .any(|p| p.enabled && same_endpoint(p, old))
                            }),
                    })
                {
                    view.machines.request = None;
                    view.machines.error = Some("removed");
                }
                self.profiles = catalog.profiles;
                self.catalog_ok = true;
                self.discovery.reconcile(&self.profiles, Instant::now());
                view.machines.replace(&self.profiles);
                if view.machines.error == Some("catalog") {
                    view.machines.error = None;
                }
                self.cached.retain(|cached| {
                    cached.profile.as_ref().is_none_or(|old| {
                        self.profiles
                            .iter()
                            .any(|p| p.enabled && same_endpoint(p, old))
                    })
                });
                if self.current.as_ref().is_some_and(|p| !self.valid(p)) {
                    self.cancel();
                    self.restore(&Choice::Local, surface, view, dimensions, false);
                    view.machines.error = Some("removed");
                    view.machines.open = true;
                    change.switched = true;
                } else if let Some(current) = &mut self.current
                    && let Some(profile) = self.profiles.iter().find(|p| p.id == current.id)
                {
                    *current = profile.clone();
                    view.machines.current_label = profile.draft.label.clone();
                }
            }
            Err(_) => {
                self.catalog_ok = false;
                self.agents.clear();
                if view
                    .machines
                    .request
                    .as_ref()
                    .is_some_and(|request| request.machine != Choice::Local)
                {
                    view.machines.request = None;
                }
                self.discovery.reconcile(&[], Instant::now());
                view.machines.replace(&[]);
                view.machines.error = Some("catalog");
            }
        }
        if self
            .attempt
            .as_ref()
            .is_some_and(|a| a.profile.as_ref().is_some_and(|p| !self.valid(p)))
        {
            self.cancel();
        }
        change.redraw |= view.machines != before;
        change
    }
    fn local_source(&self, surface: &mut SurfaceSlot<'_>) -> Option<Connection> {
        let (origin, clock) = if self.current.is_none() {
            surface.active().machine_source()?
        } else {
            self.cached
                .iter()
                .find(|c| c.profile.is_none())?
                .surface
                .machine_source()?
        };
        Some(Connection::from_origin(origin, clock))
    }
    fn restore(
        &mut self,
        requested: &Choice,
        surface: &mut SurfaceSlot<'_>,
        view: &mut ClientView,
        dimensions: WindowDimensions,
        keep_old: bool,
    ) -> bool {
        let Some(index) = self.cached.iter().position(|c| &c.choice() == requested) else {
            return false;
        };
        if self.cached[index]
            .profile
            .as_ref()
            .is_some_and(|p| !self.valid(p))
        {
            return false;
        }
        let next = self.cached.remove(index);
        if next.profile.is_some() && !next.surface.reusable_machine() {
            return false;
        }
        self.install(next, surface, view, dimensions, keep_old);
        true
    }
    fn install(
        &mut self,
        mut next: Cached,
        surface: &mut SurfaceSlot<'_>,
        view: &mut ClientView,
        dimensions: WindowDimensions,
        keep_old: bool,
    ) {
        // Retain the picker catalog and host capture truth; all per-Site navigation stays with its surface.
        let focus = surface.active().suspend_machine();
        next.surface.resume_machine(focus);
        let mut picker = view.machines.clone();
        picker.current = next.choice();
        picker.current_context = active_context(next.surface.as_ref());
        // A targeted fresh connection supersedes a cached surface of that same machine.
        self.cached
            .retain(|cached| cached.choice() != next.choice());
        picker.current_label = next
            .profile
            .as_ref()
            .map_or_else(String::new, |p| p.draft.label.clone());
        picker.open = false;
        picker.busy = false;
        picker.error = None;
        picker.request = None;
        picker.invalidate();
        next.view.resize(dimensions.columns);
        let old_view = std::mem::replace(view, next.view);
        view.settle_pointer_released(old_view.pointer_released());
        view.machines = picker;
        let old = Cached {
            profile: self.current.take(),
            surface: surface.replace(next.surface),
            view: old_view,
        };
        self.current = next.profile;
        if keep_old {
            self.cached.push(old);
        }
        // Local is retained; at most four inactive remote surfaces remain owned.
        while self.cached.iter().filter(|c| c.profile.is_some()).count() > 4 {
            if let Some(index) = self.cached.iter().position(|c| c.profile.is_some()) {
                self.cached.remove(index);
            }
        }
    }
}

fn navigation(surface: &dyn TerminalSurface) -> Vec<Context> {
    let Some(TerminalSessionView::Native(native)) = surface.view() else {
        return Vec::new();
    };
    let Some(site) = native.navigation_site() else {
        return Vec::new();
    };
    (0..native.context_count())
        .filter_map(|index| native.context_at(index))
        .map(|context| Context {
            target: NativeContextTarget {
                site_id: site.clone(),
                location: context.location().clone(),
            },
            project: context.project_label().map(display_label),
            workspace: context.workspace_label().map(display_label),
        })
        .collect()
}
fn active_context(surface: &dyn TerminalSurface) -> Option<NativeContextTarget> {
    let Some(TerminalSessionView::Native(native)) = surface.view() else {
        return None;
    };
    let location = native
        .context_at(native.active_context_position()?.checked_sub(1)?)?
        .location()
        .clone();
    Some(NativeContextTarget {
        site_id: native.navigation_site()?.clone(),
        location,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        machines::{Draft, ProfileId},
        terminal_input::UnavailableTerminalInputPort,
    };
    struct Witness {
        name: &'static str,
        focus: Option<bool>,
        log: std::rc::Rc<std::cell::RefCell<Vec<String>>>,
    }
    impl crate::terminal_input::TerminalInputPort for Witness {
        fn forward(
            &mut self,
            _bytes: &[u8],
        ) -> Result<(), crate::terminal_input::TerminalInputUnavailable> {
            Ok(())
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
    fn machine_switch_restores_local_view_and_host_mouse_state_without_reconnecting() {
        let (mut manager, mut view, _) = setup();
        view.settle_pointer_released(true);
        let log = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let mut surface = SurfaceSlot::Owned(Box::new(Witness {
            name: "local",
            focus: Some(true),
            log: log.clone(),
        }));
        manager.cached.push(Cached {
            profile: Some(profile()),
            surface: Box::new(Witness {
                name: "remote",
                focus: None,
                log: log.clone(),
            }),
            view: ClientView::new(100),
        });
        view.machines.show();
        view.machines.request = Some(Choice::Remote(profile().id).into());
        let change = manager.tick(&mut surface, &mut view, dimensions(), Instant::now());
        assert!(change.switched);
        assert!(!view.machines.open);
        assert!(view.pointer_released());
        assert_eq!(manager.cached[0].choice(), Choice::Local);
        view.machines.show();
        view.machines.request = Some(Choice::Local.into());
        assert!(
            manager
                .tick(&mut surface, &mut view, dimensions(), Instant::now())
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
            let change = manager.tick(&mut surface, &mut view, dimensions(), Instant::now());
            assert!(!change.switched);
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(cancelled.load(Ordering::Acquire), 1);
        assert_eq!(view.machines.current, Choice::Local);
        assert!(view.machines.error.is_none());
        view.machines.show();
        manager.attempt = Some(Attempt {
            profile: Some(profile()),
            context: None,
            cancel: Arc::new(AtomicU8::new(0)),
            task: Some(thread::spawn(|| Err(io::Error::other("unavailable")))),
        });
        while manager.attempt.is_some() {
            assert!(
                !manager
                    .tick(&mut surface, &mut view, dimensions(), Instant::now())
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
        manager.queued = Some(Queued {
            profile: Some(profile()),
            context: None,
            deadline: now,
        });
        assert!(
            !manager
                .tick(&mut surface, &mut view, dimensions(), now)
                .switched
        );
        assert_eq!(view.machines.error, Some("failed"));
        assert!(manager.queued.is_none());
        manager.queued = Some(Queued {
            profile: Some(profile()),
            context: None,
            deadline: now + Duration::from_secs(60),
        });
        manager.profiles[0].draft.target = "replacement.invalid".into();
        assert!(
            !manager
                .tick(&mut surface, &mut view, dimensions(), now)
                .switched
        );
        assert_eq!(view.machines.error, Some("removed"));
        assert!(manager.queued.is_none());
        manager.profiles[0] = profile();
        manager.queued = Some(Queued {
            profile: Some(profile()),
            context: None,
            deadline: now + Duration::from_secs(60),
        });
        view.machines.open = false;
        assert!(
            !manager
                .tick(&mut surface, &mut view, dimensions(), now)
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
        );
        let change = manager.refresh_catalog(
            Err(crate::machines::Error::InvalidCatalog),
            &mut surface,
            &mut view,
            dimensions(),
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
        );
        assert!(change.switched);
        assert_eq!(view.machines.current, Choice::Local);
        assert!(manager.cached.is_empty());
        assert!(view.machines.open);
        view.machines.request = Some(Choice::Local.into());
        manager.tick(&mut surface, &mut view, dimensions(), Instant::now());
        assert!(
            !view.machines.open,
            "even a disconnected Local surface must remain reachable"
        );
    }
}

#[cfg(test)]
mod startup_tests;
