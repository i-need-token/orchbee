//! UI-owned saved-machine connections. Only an explicit selection starts network work.
use super::{SurfaceSlot, TerminalSurface, terminal_geometry};
use crate::{
    client_view::ClientView,
    local_client::LocalClient,
    machine_picker::Choice,
    machines::{Profile, Store},
    terminal_device::WindowDimensions,
    terminal_session::{TerminalIntent, TerminalSession, TerminalStatus},
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
    profile: Profile,
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
    profile: Profile,
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
fn same_endpoint(a: &Profile, b: &Profile) -> bool {
    a.id == b.id && a.draft.target == b.draft.target && a.draft.session == b.draft.session
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
    queued: Option<Choice>,
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
        }
        if !view.machines.open {
            self.cancel();
        }
        if let Some(request) = view.machines.request.take() {
            self.cancel();
            if request == choice(self.current.as_ref())
                && (request == Choice::Local || surface.active().reusable_machine())
            {
                view.machines.open = false;
            } else if self.restore(&request, surface, view, dimensions, true) {
                change.switched = true;
            } else if let Choice::Remote(id) = &request {
                if self.catalog_ok && self.profiles.iter().any(|p| &p.id == id && p.enabled) {
                    self.queued = Some(request);
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
                && self.valid(&attempt.profile)
            {
                let candidate = result.and_then(|client| {
                    let geometry = terminal_geometry(dimensions, &self.template, None)?
                        .ok_or_else(|| io::Error::other("viewport too small"))?;
                    TerminalSession::start(client, TerminalIntent::Standalone, geometry)
                });
                match candidate {
                    Ok(session) => {
                        self.candidate = Some(Candidate {
                            profile: attempt.profile.clone(),
                            session,
                            deadline: now + Duration::from_secs(30),
                        })
                    }
                    Err(_) => view.machines.error = Some("failed"),
                }
            }
            change.redraw = true;
        }
        if self.attempt.is_none()
            && let Some(Choice::Remote(id)) = self.queued.take()
        {
            if let Some(profile) = self
                .profiles
                .iter()
                .find(|p| p.id == id && p.enabled)
                .filter(|_| self.catalog_ok)
                .cloned()
            {
                let cancel = Arc::new(AtomicU8::new(0));
                let worker_cancel = cancel.clone();
                let worker_profile = profile.clone();
                match thread::Builder::new()
                    .name("orchbee-machine-connect".into())
                    .spawn(move || {
                        crate::machine_tunnel::connect(
                            &worker_profile,
                            Instant::now() + Duration::from_secs(60),
                            &worker_cancel,
                        )
                    }) {
                    Ok(task) => {
                        self.attempt = Some(Attempt {
                            profile,
                            cancel,
                            task: Some(task),
                        })
                    }
                    Err(_) => view.machines.error = Some("failed"),
                }
            } else {
                view.machines.error = Some("removed");
            }
            change.redraw = true;
        }
        if let Some(mut candidate) = self.candidate.take() {
            candidate.session.tick();
            let failed = now >= candidate.deadline
                || matches!(
                    candidate.session.status(),
                    TerminalStatus::Unavailable | TerminalStatus::OutcomeUnknown
                );
            if !view.machines.open || !self.valid(&candidate.profile) {
                view.machines.error = Some("removed");
                change.redraw = true;
            } else if failed {
                view.machines.error = Some("failed");
                change.redraw = true;
            } else if candidate.session.startup_draw_ready() {
                let next = Cached {
                    profile: Some(candidate.profile),
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
                self.profiles = catalog.profiles;
                self.catalog_ok = true;
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
                view.machines.replace(&[]);
                view.machines.error = Some("catalog");
            }
        }
        if self
            .attempt
            .as_ref()
            .is_some_and(|a| !self.valid(&a.profile))
        {
            self.cancel();
        }
        change.redraw |= view.machines != before;
        change
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
    fn profile() -> Profile {
        Profile {
            id: ProfileId::parse("11111111111111111111111111111111").unwrap(),
            draft: Draft::parse("remote", "example.invalid", "default").unwrap(),
            enabled: true,
        }
    }
    fn dimensions() -> WindowDimensions {
        WindowDimensions {
            columns: 100,
            rows: 30,
            pixel_height: 0,
            pixel_width: 0,
        }
    }
    fn setup() -> (Manager, ClientView, SurfaceSlot<'static>) {
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
        view.machines.request = Some(Choice::Remote(profile().id));
        let change = manager.tick(&mut surface, &mut view, dimensions(), Instant::now());
        assert!(change.switched);
        assert!(!view.machines.open);
        assert!(view.pointer_released());
        assert_eq!(manager.cached[0].choice(), Choice::Local);
        view.machines.show();
        view.machines.request = Some(Choice::Local);
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
            profile: profile(),
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
            profile: profile(),
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
    struct DisconnectedLocal;
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
        view.machines.request = Some(Choice::Local);
        manager.tick(&mut surface, &mut view, dimensions(), Instant::now());
        assert!(
            !view.machines.open,
            "even a disconnected Local surface must remain reachable"
        );
    }
}
