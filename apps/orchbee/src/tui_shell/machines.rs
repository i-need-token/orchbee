//! UI-owned selection over read-only saved-machine observers.
use super::{SurfaceSlot, TerminalSurface, terminal_geometry};
use crate::background_thread::BackgroundThread;
use crate::{
    client_view::ClientView,
    local_client::{LocalClient, OwnedCancellation},
    machine_discovery::{Connection, Context, Discovery, display_label, same_endpoint},
    machine_picker::{Choice, SelectionOrigin, SelectionScope},
    machine_refusal::Refusal,
    machines::{Profile, Store},
    terminal_device::WindowDimensions,
    workbench_client::{
        ClientContextTarget, ClientStartupTarget, ContextNavigationAdmission, TerminalStatus,
        WorkbenchClient,
    },
};
use std::{
    io,
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

/// The picker messages that say why no hub is reached; the hub's own state replaces them.
const HUB_MESSAGES: [&str; 3] = ["hub", "hub-message-stopped", "hub-message-blocked"];

struct Attempt {
    profile: Option<Profile>,
    context: Option<ClientStartupTarget>,
    cancel: Arc<OwnedCancellation>,
    task: BackgroundThread,
    result: mpsc::Receiver<io::Result<PreparedSelection>>,
}

impl Attempt {
    /// The result, once the worker delivered it; a worker that ended without one failed.
    fn settled(&self) -> Option<io::Result<PreparedSelection>> {
        match self.result.try_recv() {
            Ok(result) => Some(result),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => {
                Some(Err(io::Error::other("machine connection worker failed")))
            }
        }
    }
}

/// Delivers the result before waking the tick, which has no other reason to look at it soon.
fn spawn_attempt(
    wakeup: crate::workbench_client::UiWakeup,
    body: impl FnOnce() -> io::Result<PreparedSelection> + Send + 'static,
) -> io::Result<(
    BackgroundThread,
    mpsc::Receiver<io::Result<PreparedSelection>>,
)> {
    let (sender, result) = mpsc::sync_channel(1);
    let task = BackgroundThread::spawn("orchbee-machine-connect", move || {
        // A cancelled attempt's owner may already have dropped the receiver.
        let _delivered = sender.send(body());
        wakeup.signal();
    })?;
    Ok((task, result))
}
enum PreparedSelection {
    Connection(LocalClient),
    Agent {
        target: Box<crate::machine_agents::Target>,
        deadline: Instant,
    },
}
impl Drop for Attempt {
    fn drop(&mut self) {
        // Only Manager::stop_before waits; an unfinished connection attempt is detached here.
        self.cancel.cancel();
    }
}
struct Candidate {
    profile: Option<Profile>,
    session: WorkbenchClient,
    targeted: bool,
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
    destination: Destination,
    deadline: Instant,
    /// What was left of the deadline when a login question began; the clock stops while the
    /// person answers, as the hub's does.
    paused: Option<Duration>,
}
enum Destination {
    Startup(Option<ClientStartupTarget>),
    VerifiedAgent(Box<crate::machine_agents::Target>),
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
    selection_scope: Option<SelectionScope>,
    discovery: Discovery,
    agents: crate::machine_agents::Collector,
    spaces: crate::machine_spaces::Collector,
    space_git: Option<bool>,
    wakeup: crate::workbench_client::UiWakeup,
    /// Clients no longer presented, kept until their threads end: dropping one only requests stop
    /// and detaches what still runs, out of process exit's sight.
    retired: Vec<Box<dyn TerminalSurface>>,
}
impl Manager {
    /// Process exit: asks every cached, candidate and retired Client, the connection attempt, and
    /// every machine observer to stop without waiting.
    pub(super) fn request_stop(&mut self) {
        for cached in &mut self.cached {
            cached.surface.request_stop();
        }
        if let Some(candidate) = &mut self.candidate {
            candidate.session.request_stop();
        }
        for retired in &mut self.retired {
            retired.request_stop();
        }
        if let Some(attempt) = &self.attempt {
            attempt.cancel.cancel();
        }
        self.agents.request_stop();
        self.spaces.request_stop();
        self.discovery.request_stop();
    }

    /// Process exit: after every owner was asked to stop, observes each against one deadline.
    pub(super) fn stop_before(&mut self, deadline: Instant) -> io::Result<()> {
        self.request_stop();
        let cached = self
            .cached
            .iter_mut()
            .map(|cached| cached.surface.stop_before(deadline))
            .fold(Ok(()), io::Result::and);
        let candidate = self
            .candidate
            .as_mut()
            .map_or(Ok(()), |candidate| candidate.session.stop_before(deadline));
        let retired = self
            .retired
            .iter_mut()
            .map(|retired| retired.stop_before(deadline))
            .fold(Ok(()), io::Result::and);
        let attempt = self
            .attempt
            .as_mut()
            .map_or(Ok(()), |attempt| attempt.task.join_before(deadline));
        let agents = self.agents.stop_before(deadline);
        let spaces = self.spaces.stop_before(deadline);
        let discovery = self.discovery.stop_before(deadline);
        cached
            .and(candidate)
            .and(retired)
            .and(attempt)
            .and(agents)
            .and(spaces)
            .and(discovery)
    }

    /// Stops presenting a Client: it is asked to stop now and stays owned until its threads end.
    fn retire(&mut self, mut surface: Box<dyn TerminalSurface>) {
        surface.request_stop();
        self.retired.push(surface);
    }
}
impl Drop for Manager {
    fn drop(&mut self) {
        // Drop UI-held source/watch clones before Discovery cancels its background guards.
        // Cancelled reader jobs keep and release any remaining carrier clones on their threads.
        self.agents.clear();
        self.spaces.clear();
    }
}
impl Manager {
    pub(super) fn new(view: &mut ClientView, wakeup: crate::workbench_client::UiWakeup) -> Self {
        view.machines.available = true;
        if !view
            .capabilities()
            .supports(crate::tui_capabilities::Capability::SavedMachines)
        {
            view.machines.replace(&[]);
        }
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
            selection_scope: None,
            discovery: Discovery::new(wakeup.clone()),
            // Host process titles have no publication source; canonical Agent facts also push.
            agents: crate::machine_agents::Collector::new(wakeup.clone(), true),
            spaces: crate::machine_spaces::Collector::new(wakeup.clone(), false),
            space_git: None,
            wakeup,
            retired: Vec::new(),
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
            attempt.cancel.cancel();
        }
        if let Some(candidate) = self.candidate.take() {
            self.retire(Box::new(candidate.session));
        }
        // A selection that ends before its login dismisses the login.
        if let Some(Queued {
            profile: Some(profile),
            ..
        }) = self.queued.take()
        {
            self.discovery.abandon_login(&profile.id);
        }
        self.selection_scope = None;
    }
    fn selection_current(&self, view: &ClientView) -> bool {
        self.selection_scope
            .is_some_and(|scope| view.machines.selection_current(scope))
    }
    fn finish_selection(&mut self, view: &mut ClientView) {
        if self
            .selection_scope
            .take()
            .is_some_and(|scope| view.machines.selection_scope() == Some(scope))
        {
            view.machines.cancel_selection();
        }
    }
    fn fail_selection(&mut self, view: &mut ClientView, error: &'static str) {
        if self.selection_current(view) {
            report_selection_failure(view, error);
        }
        self.selection_scope = None;
    }
    /// A remote refusal names the side that must change instead of inviting a retry.
    fn refuse_selection(&mut self, view: &mut ClientView, refusal: Refusal) {
        if self.selection_current(view) {
            if view.machines.direct_selection_pending() {
                view.machine_refused(refusal);
            }
            view.machines.selection_failed(refusal.key());
        }
        self.selection_scope = None;
    }
    pub(super) fn tick(
        &mut self,
        surface: &mut SurfaceSlot<'_>,
        view: &mut ClientView,
        dimensions: WindowDimensions,
        now: Instant,
        before_switch: &mut dyn FnMut(&mut dyn crate::terminal_input::TerminalInputPort),
    ) -> Change {
        let mut change = Change::default();
        // Observing an elapsed deadline never waits: release the retired Clients that ended.
        self.retired
            .retain_mut(|retired| retired.stop_before(now).is_err());
        self.template
            .notifications
            .configure(view.notifications.policy().clone());
        if now >= self.refresh
            && view
                .capabilities()
                .supports(crate::tui_capabilities::Capability::SavedMachines)
        {
            self.refresh = now + Duration::from_secs(1);
            change = self.refresh_catalog(
                Store::configured().and_then(|store| store.read()),
                surface,
                view,
                dimensions,
                before_switch,
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
        change.redraw |= view.machines.observe_focus(
            choice(self.current.as_ref()),
            active_context,
            focused_member(surface.active()),
        );
        // Command admission and validated snapshots already wake this loop. Do not wait for
        // metadata's push-driven reads or external Git/Host sampling to paint navigation changes.
        let navigation = match surface.active().view() {
            Some(native) => native.sidebar_navigation(),
            _ => None,
        };
        change.redraw |= view
            .machines
            .observe_native_navigation(choice(self.current.as_ref()), navigation);
        self.discovery.tick(now);
        change.redraw |= view.machines.observe(&self.discovery.observations(now));
        // The login question the hub put to this TUI, and the person's act on the last one.
        change.redraw |= view.machines.exchange_login(&mut self.discovery);
        // "Forget password" on a selected remote Machine, and how the last one ended.
        change.redraw |= view.machines.exchange_forget(&mut self.discovery);
        // The current remote Machine needs a login again: its pane offers Enter to log in.
        let lost_link = self
            .current
            .as_ref()
            .is_some_and(|profile| self.discovery.login_needed(profile));
        if view.machines.lost_link != lost_link {
            view.machines.lost_link = lost_link;
            change.redraw = true;
        }
        // The status line says why no hub is reached, without hiding another message or a
        // selection in progress.
        let hub_message = self.discovery.hub_message();
        let shown = view
            .machines
            .error
            .filter(|error| HUB_MESSAGES.contains(error));
        if hub_message != shown
            && (shown.is_some() || view.machines.error.is_none())
            && (hub_message.is_none() || (self.queued.is_none() && self.attempt.is_none()))
        {
            view.machines.error = hub_message;
            change.redraw = true;
        }
        let mut agent_sources = Vec::new();
        let every_machine = view.machines.wants_agents() || view.machines.wants_spaces();
        // The Windows Tab strip summarizes the attached machine's Agents without the Sidebar.
        let current = self.current.as_ref().map(|profile| profile.id.clone());
        let attached = |profile: Option<&Profile>| current.as_ref() == profile.map(|p| &p.id);
        if (every_machine || attached(None))
            && let Some(connection) = self.local_source(surface)
        {
            agent_sources.push(crate::machine_observation::Source {
                choice: Choice::Local,
                connection,
            });
        }
        for profile in &self.profiles {
            if (every_machine || attached(Some(profile)))
                && let Some(connection) = self.discovery.connection(profile, now)
            {
                agent_sources.push(crate::machine_observation::Source {
                    choice: Choice::Remote(profile.id.clone()),
                    connection,
                });
            }
        }
        change.redraw |= view.machines.observe_agents(self.agents.tick(
            agent_sources.clone(),
            now,
            crate::machine_agents::read,
        ));
        if self.agents.hinted(&choice(self.current.as_ref())) {
            surface.active().expedite_agent_reads();
        }
        let space_git = view.machines.space_settings().needs_git();
        if self.space_git != Some(space_git) {
            self.spaces.clear();
            // External Git changes have no Server event source. Metadata-only views do not poll.
            self.spaces.sample_external_sources(space_git);
            self.space_git = Some(space_git);
        }
        let space_sources = if view.machines.wants_spaces() {
            agent_sources
        } else {
            Vec::new()
        };
        // Commits refresh canonical metadata at once; Git keeps its own sampling cooldown, so they
        // cannot queue one Git request per Workspace on the carrier the terminals share. Only a
        // Workspace that moved to a directory without a known answer asks once ahead of it.
        let spaces = if space_git {
            for (machine, workspaces) in view.machines.take_space_git_demand() {
                self.spaces
                    .expedite(&machine, crate::machine_spaces::read_git_of(workspaces));
            }
            self.spaces.tick_split(
                space_sources,
                now,
                crate::machine_spaces::read_metadata,
                crate::machine_spaces::read_with_git,
            )
        } else {
            self.spaces
                .tick(space_sources, now, crate::machine_spaces::read_metadata)
        };
        change.redraw |= view.machines.observe_spaces(spaces);
        // The UI loop runs at least this often, so a hold's bound needs no wake of its own.
        change.redraw |= view.machines.advance_space_holds(now);
        // Failed observers retain their background transport guard until both collectors have
        // dropped cached origins/subscriptions. Remaining cancelled jobs dispose on their threads.
        self.discovery.finish_retirement();
        if let Some(native) = surface.active().view()
            && view
                .machines
                .needs_agent_attention(native.machine_attention_version())
        {
            let bindings = view.machines.agent_attention_bindings();
            let snapshots: Vec<_> = bindings
                .chunks(orchbee_workbench_protocol::MAXIMUM_CLIENT_ATTENTION_SESSIONS)
                .filter_map(|chunk| native.machine_agent_attention(chunk))
                .collect();
            change.redraw |= view.machines.observe_agent_attention(&snapshots);
        }
        change.redraw |= view.machines.drive_agent_navigation(now);
        if !self.selection_current(view) {
            self.cancel();
        }
        if let Some(request) = view.machines.request.take()
            && let Some(scope) = view.machines.selection_scope()
        {
            self.cancel();
            self.selection_scope = Some(scope);
            view.clear_workspace_navigation_notice();
            if request.context.is_none()
                && request.machine == choice(self.current.as_ref())
                && (request.machine == Choice::Local
                    || (surface.active().reusable_machine() && !view.machines.lost_link))
            {
                view.machines.open = false;
                self.finish_selection(view);
            } else if request.context.is_none()
                && self.restore(
                    &request.machine,
                    surface,
                    view,
                    dimensions,
                    true,
                    before_switch,
                )
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
                        destination: Destination::Startup(request.context),
                        deadline: now + Duration::from_secs(60),
                        paused: None,
                    });
                    view.machines.error = None;
                } else {
                    self.fail_selection(view, "removed");
                }
            }
            change.redraw = true;
        }
        if let Some(result) = self.attempt.as_ref().and_then(Attempt::settled) {
            let attempt = self.attempt.take().expect("settled attempt");
            if !attempt.cancel.is_cancelled()
                && self.selection_current(view)
                && attempt.profile.as_ref().is_none_or(|p| self.valid(p))
            {
                match result {
                    Ok(PreparedSelection::Agent { target, deadline }) => {
                        self.queued = Some(Queued {
                            profile: attempt.profile.clone(),
                            destination: Destination::VerifiedAgent(target),
                            deadline,
                            paused: None,
                        });
                    }
                    Ok(PreparedSelection::Connection(mut client)) => {
                        let startup_deadline = now + Duration::from_secs(30);
                        let candidate = (|| {
                            let geometry = terminal_geometry(dimensions, &self.template)?
                                .ok_or_else(|| io::Error::other("viewport too small"))?;
                            client.set_submission_deadline(Some(startup_deadline));
                            match attempt.context.clone() {
                                Some(context) => {
                                    WorkbenchClient::start_target(client, context, geometry)
                                }
                                None => WorkbenchClient::start(client, geometry),
                            }
                        })();
                        match candidate {
                            Ok(session) => {
                                self.candidate = Some(Candidate {
                                    profile: attempt.profile.clone(),
                                    session,
                                    targeted: attempt.context.is_some(),
                                    deadline: startup_deadline,
                                })
                            }
                            Err(_) => self.fail_selection(view, "failed"),
                        }
                    }
                    Err(_) => self.fail_selection(view, "failed"),
                }
            }
            change.redraw = true;
        }
        if self.attempt.is_none()
            && let Some(mut queued) = self.queued.take()
        {
            let asking = queued
                .profile
                .as_ref()
                .is_some_and(|p| self.discovery.login_active(&p.id));
            match (asking, queued.paused) {
                (true, None) => {
                    queued.paused = Some(queued.deadline.saturating_duration_since(now));
                }
                (false, Some(left)) => {
                    queued.deadline = now + left;
                    queued.paused = None;
                }
                _ => {}
            }
            if queued.profile.as_ref().is_some_and(|p| !self.valid(p)) {
                self.fail_selection(view, "removed");
            } else if let Some(refusal) = queued
                .profile
                .as_ref()
                .and_then(|p| self.discovery.refusal(p))
            {
                self.refuse_selection(view, refusal);
            } else if (queued.paused.is_none() && now >= queued.deadline)
                || queued
                    .profile
                    .as_ref()
                    .is_some_and(|p| self.discovery.failed(p))
            {
                // A selection that reached no hub says why instead of naming SSH; one that ended
                // in its login says how the login ended.
                let failure = match &queued.profile {
                    Some(profile) => self
                        .discovery
                        .hub_message()
                        .or_else(|| self.discovery.login_failure(profile)),
                    None => None,
                };
                self.fail_selection(view, failure.unwrap_or("failed"));
            } else if self
                .selection_scope
                .is_some_and(|scope| scope.origin == SelectionOrigin::Sidebar)
                && same_machine_endpoint(self.current.as_ref(), queued.profile.as_ref())
                && let Destination::Startup(Some(ClientStartupTarget::Context(target))) =
                    &queued.destination
            {
                // Reuse this Session's per-location tab/panel memory. One accepted
                // transition settles before the latest queued intent is admitted.
                match surface.active().navigate_context(target) {
                    ContextNavigationAdmission::Current => self.finish_selection(view),
                    ContextNavigationAdmission::Admitted => {
                        before_switch(surface.active());
                        self.finish_selection(view);
                    }
                    ContextNavigationAdmission::Busy => self.queued = Some(queued),
                    ContextNavigationAdmission::Unavailable => self.fail_selection(view, "failed"),
                }
            } else if let Destination::VerifiedAgent(target) = &queued.destination {
                if !same_machine_endpoint(self.current.as_ref(), queued.profile.as_ref()) {
                    self.fail_selection(view, "removed");
                } else {
                    match surface.active().navigate_agent(target) {
                        ContextNavigationAdmission::Current => self.finish_selection(view),
                        ContextNavigationAdmission::Admitted => {
                            before_switch(surface.active());
                            self.finish_selection(view);
                        }
                        ContextNavigationAdmission::Busy => self.queued = Some(queued),
                        ContextNavigationAdmission::Unavailable => {
                            self.fail_selection(view, "failed")
                        }
                    }
                }
            } else if let Some(connection) = match &queued.profile {
                Some(profile) => self.discovery.connection(profile, now),
                None => self.local_source(surface),
            } {
                let Destination::Startup(context) = queued.destination else {
                    unreachable!("verified Agent destinations are handled without a new Session");
                };
                let direct_agent = self
                    .selection_scope
                    .filter(|scope| scope.origin == SelectionOrigin::Sidebar)
                    .filter(|_| {
                        same_machine_endpoint(self.current.as_ref(), queued.profile.as_ref())
                    })
                    .and_then(|_| match &context {
                        Some(ClientStartupTarget::Agent(target)) => Some(target.clone()),
                        _ => None,
                    });
                match OwnedCancellation::new().and_then(|cancel| {
                    let cancel = Arc::new(cancel);
                    let worker_cancel = cancel.clone();
                    let (task, result) = spawn_attempt(self.wakeup.clone(), move || {
                        let mut client =
                            connection.reconnect(queued.deadline, worker_cancel.borrow())?;
                        if let Some(target) = direct_agent {
                            let snapshot = crate::machine_observation::snapshot(
                                &mut client,
                                queued.deadline,
                                worker_cancel.borrow(),
                            )
                            .map_err(|()| {
                                io::Error::other("Agent selection snapshot unavailable")
                            })?;
                            crate::machine_agents::verify(
                                &mut client,
                                &snapshot,
                                &target,
                                queued.deadline,
                                worker_cancel.borrow(),
                            )
                            .map_err(|()| io::Error::other("Agent selection is stale"))?;
                            Ok(PreparedSelection::Agent {
                                target,
                                deadline: queued.deadline,
                            })
                        } else {
                            Ok(PreparedSelection::Connection(client))
                        }
                    })?;
                    Ok((cancel, task, result))
                }) {
                    Ok((cancel, task, result)) => {
                        self.attempt = Some(Attempt {
                            profile: queued.profile,
                            context,
                            cancel,
                            task,
                            result,
                        })
                    }
                    Err(_) => self.fail_selection(view, "failed"),
                }
            } else if queued.profile.is_none() {
                self.fail_selection(view, "failed");
            } else {
                self.queued = Some(queued);
            }
            change.redraw |= self.queued.is_none();
        }
        // Only the remote selection still queued may ask the person; a selection that ended in any
        // way abandons its login.
        let waiting = self
            .queued
            .as_ref()
            .and_then(|queued| queued.profile.as_ref())
            .map(|profile| profile.id.clone());
        self.discovery.wait_for(waiting.as_ref());
        if let Some(mut candidate) = self.candidate.take() {
            candidate.session.tick();
            let drawable = candidate.session.startup_draw_ready();
            let failed = now >= candidate.deadline
                || (!drawable
                    && matches!(
                        candidate.session.status(),
                        TerminalStatus::Unavailable | TerminalStatus::OutcomeUnknown
                    ));
            if !self.selection_current(view)
                || candidate.profile.as_ref().is_some_and(|p| !self.valid(p))
            {
                self.fail_selection(view, "removed");
                change.redraw = true;
                self.retire(Box::new(candidate.session));
            } else if failed {
                self.fail_selection(view, "failed");
                change.redraw = true;
                self.retire(Box::new(candidate.session));
            } else if drawable {
                if let Some(bank) = retained_frames_for_target(
                    self.current.as_ref(),
                    surface.active(),
                    &self.cached,
                    candidate.profile.as_ref(),
                    candidate.targeted,
                ) {
                    candidate.session.inherit_retained_frame_bank(bank);
                }
                candidate
                    .session
                    .configure_agent_notifications(0, self.template.notifications.policy());
                let next = Cached {
                    profile: candidate.profile,
                    surface: Box::new(candidate.session),
                    view: self.template.clone(),
                };
                let keep_old = next.choice() != choice(self.current.as_ref());
                self.install(next, surface, view, dimensions, keep_old, before_switch);
                change.switched = true;
            } else {
                self.candidate = Some(candidate);
            }
        }
        let busy = self.selection_current(view)
            && (self.attempt.is_some() || self.candidate.is_some() || self.queued.is_some());
        change.redraw |= busy != view.machines.busy || change.switched;
        view.machines.busy = busy;
        // The Client's own tick replays held keys before this tick retries the navigation, so it
        // learns here that a newer destination still waits.
        surface
            .active()
            .observe_direct_navigation(self.direct_navigation(view));
        change
    }

    /// The Context the user's current sidebar selection still has to reach through the attached
    /// Session's Client: queued behind that Client's switch or read, or an Agent row in
    /// verification.
    fn direct_navigation(&self, view: &ClientView) -> Option<&ClientContextTarget> {
        if !self
            .selection_scope
            .is_some_and(|scope| scope.origin == SelectionOrigin::Sidebar)
            || !self.selection_current(view)
        {
            return None;
        }
        let (profile, target) = match (&self.queued, &self.attempt) {
            (Some(queued), _) => (
                queued.profile.as_ref(),
                match &queued.destination {
                    Destination::Startup(Some(ClientStartupTarget::Context(target))) => target,
                    Destination::Startup(Some(ClientStartupTarget::Agent(target)))
                    | Destination::VerifiedAgent(target) => target.destination(),
                    Destination::Startup(_) => return None,
                },
            ),
            (None, Some(attempt)) => (
                attempt.profile.as_ref(),
                match attempt.context.as_ref()? {
                    ClientStartupTarget::Context(target) => target,
                    ClientStartupTarget::Agent(target) => target.destination(),
                    _ => return None,
                },
            ),
            (None, None) => return None,
        };
        same_machine_endpoint(self.current.as_ref(), profile).then_some(target)
    }
    fn refresh_catalog(
        &mut self,
        catalog: Result<crate::machines::Catalog, crate::machines::Error>,
        surface: &mut SurfaceSlot<'_>,
        view: &mut ClientView,
        dimensions: WindowDimensions,
        before_switch: &mut dyn FnMut(&mut dyn crate::terminal_input::TerminalInputPort),
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
                    for old in self.profiles.iter().filter(|old| {
                        old.enabled
                            && !catalog
                                .profiles
                                .iter()
                                .any(|profile| profile.enabled && same_endpoint(profile, old))
                    }) {
                        view.machines
                            .forget_space_observations(&Choice::Remote(old.id.clone()));
                    }
                    self.agents.clear();
                    self.spaces.clear();
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
                    report_selection_failure(view, "removed");
                }
                self.profiles = catalog.profiles;
                self.catalog_ok = true;
                self.discovery
                    .reconcile(&self.profiles, Some(catalog.revision), Instant::now());
                view.machines.replace(&self.profiles);
                if view.machines.error == Some("catalog") {
                    view.machines.error = None;
                }
                let profiles = &self.profiles;
                let removed: Vec<_> = self
                    .cached
                    .extract_if(.., |cached| {
                        cached.profile.as_ref().is_some_and(|old| {
                            !profiles.iter().any(|p| p.enabled && same_endpoint(p, old))
                        })
                    })
                    .collect();
                for cached in removed {
                    self.retire(cached.surface);
                }
                if self.current.as_ref().is_some_and(|p| !self.valid(p)) {
                    self.cancel();
                    self.restore(
                        &Choice::Local,
                        surface,
                        view,
                        dimensions,
                        false,
                        before_switch,
                    );
                    view.machines.error = Some("removed");
                    view.workspace_navigation_unavailable();
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
                self.spaces.clear();
                if view
                    .machines
                    .request
                    .as_ref()
                    .is_some_and(|request| request.machine != Choice::Local)
                {
                    report_selection_failure(view, "catalog");
                }
                self.discovery.reconcile(&[], None, Instant::now());
                view.machines.replace(&[]);
                view.machines.error = Some("catalog");
            }
        }
        if self
            .attempt
            .as_ref()
            .is_some_and(|a| a.profile.as_ref().is_some_and(|p| !self.valid(p)))
        {
            self.fail_selection(view, "removed");
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
        before_switch: &mut dyn FnMut(&mut dyn crate::terminal_input::TerminalInputPort),
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
        // A Machine the hub says needs a login has no live link behind its cached surface.
        if next.profile.is_some()
            && (!next.surface.reusable_machine()
                || next
                    .profile
                    .as_ref()
                    .is_some_and(|p| self.discovery.login_needed(p)))
        {
            self.retire(next.surface);
            return false;
        }
        self.install(next, surface, view, dimensions, keep_old, before_switch);
        true
    }
    fn install(
        &mut self,
        mut next: Cached,
        surface: &mut SurfaceSlot<'_>,
        view: &mut ClientView,
        dimensions: WindowDimensions,
        keep_old: bool,
        before_switch: &mut dyn FnMut(&mut dyn crate::terminal_input::TerminalInputPort),
    ) {
        // Retain the picker catalog and host capture truth; all per-Session navigation stays with its surface.
        before_switch(surface.active());
        let focus = surface.active().suspend_machine();
        next.surface.resume_machine(focus);
        let mut picker = view.machines.clone();
        picker.reset_agent_attention_read();
        picker.observe_focus(
            next.choice(),
            active_context(next.surface.as_ref()),
            focused_member(next.surface.as_ref()),
        );
        // A targeted fresh connection supersedes a cached surface of that same machine.
        let machine = next.choice();
        let superseded: Vec<_> = self
            .cached
            .extract_if(.., |cached| cached.choice() == machine)
            .collect();
        for cached in superseded {
            self.retire(cached.surface);
        }
        picker.current_label = next
            .profile
            .as_ref()
            .map_or_else(String::new, |p| p.draft.label.clone());
        picker.open = false;
        picker.busy = false;
        picker.error = None;
        picker.cancel_selection();
        self.selection_scope = None;

        next.view.resize(dimensions.columns);
        let mut old_view = std::mem::replace(view, next.view);
        // Only the presented view holds the login overlay.
        old_view.machines.forget_login();
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
        } else {
            self.retire(old.surface);
        }
        // Local is retained; at most four inactive remote surfaces remain cached.
        while self.cached.iter().filter(|c| c.profile.is_some()).count() > 4 {
            if let Some(index) = self.cached.iter().position(|c| c.profile.is_some()) {
                let evicted = self.cached.remove(index);
                self.retire(evicted.surface);
            }
        }
    }
}

fn report_selection_failure(view: &mut ClientView, error: &'static str) {
    if view.machines.direct_selection_pending() {
        view.workspace_navigation_unavailable();
    }
    view.machines.selection_failed(error);
}

fn same_machine_endpoint(left: Option<&Profile>, right: Option<&Profile>) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => same_endpoint(left, right),
        _ => false,
    }
}

fn retained_frames_for_target(
    current: Option<&Profile>,
    current_surface: &dyn TerminalSurface,
    cached: &[Cached],
    candidate: Option<&Profile>,
    targeted: bool,
) -> Option<crate::workbench_client::RetainedFrameBank> {
    if !targeted {
        return None;
    }
    if same_machine_endpoint(current, candidate) {
        return current_surface.retained_frame_bank();
    }
    cached
        .iter()
        .find(|source| same_machine_endpoint(source.profile.as_ref(), candidate))?
        .surface
        .retained_frame_bank()
}

fn navigation(surface: &dyn TerminalSurface) -> Vec<Context> {
    let Some(native) = surface.view() else {
        return Vec::new();
    };
    let Some(session) = native.navigation_session() else {
        return Vec::new();
    };
    (0..native.context_count())
        .filter_map(|index| native.context_at(index))
        .map(|context| Context {
            target: ClientContextTarget {
                session_id: session.clone(),
                location: context.location().clone(),
            },
            project_id: context.project_id().cloned(),
            project: context.project_label().map(display_label),
            workspace: context.workspace_label().map(display_label),
        })
        .collect()
}
fn active_context(surface: &dyn TerminalSurface) -> Option<ClientContextTarget> {
    let native = surface.view()?;
    let location = native
        .context_at(native.active_context_position()?.checked_sub(1)?)?
        .location()
        .clone();
    Some(ClientContextTarget {
        session_id: native.navigation_session()?.clone(),
        location,
    })
}

fn focused_member(
    surface: &dyn TerminalSurface,
) -> Option<crate::workbench_client::ClientMemberTarget> {
    let native = surface.view()?;
    native.focused_member()
}

#[cfg(test)]
mod tests;
