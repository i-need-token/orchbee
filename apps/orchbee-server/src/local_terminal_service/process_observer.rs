//! Windows Session-owned periodic process observations with one retained viewer cleanup responsibility.
use super::{LocalTerminalService, viewer_connection::PendingCleanup};
use crate::agent_screen::{ScreenDetector, updating};
use crate::session_coordinator::ProcessObservationTarget;
use std::{
    io,
    sync::{
        Arc, Condvar, Mutex, Weak,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
const INTERVAL: Duration = Duration::from_millis(50);
const DISCOVERY_INTERVAL: Duration = Duration::from_millis(250);
const TURN_BUDGET: Duration = Duration::from_millis(100);
const TARGETS_PER_TURN: usize = 8;
type Cursor = (String, String);
#[derive(Default)]
pub(super) struct Schedule {
    agents: Option<Cursor>,
    discovery: Option<Cursor>,
}
#[cfg(windows)]
mod agent_resume;
mod close;
mod directory;
mod explanation;
mod host_sampling;
mod manifests;
mod on_demand;
#[derive(Default)]
struct Wake {
    stop: AtomicBool,
    wait: Mutex<()>,
    changed: Condvar,
    pending: Mutex<Option<PendingCleanup>>,
    closes: close::Requests,
    directories: directory::Requests,
    explanations: explanation::Requests,
    #[cfg(windows)]
    resumes: agent_resume::Requests,
    manifests: manifests::Registry,
}
impl Wake {
    /// A read queued while the sampler was busy, whose wake-up therefore found no waiter.
    fn reads_waiting(&self) -> bool {
        #[cfg(windows)]
        if self.resumes.waiting() {
            return true;
        }
        self.closes.waiting() || self.directories.waiting() || self.explanations.waiting()
    }
    /// A stopping observer serves no read, so none may keep its wait from sleeping.
    fn cancel_reads(&self) {
        self.closes.cancel();
        self.directories.cancel();
        self.explanations.cancel();
        #[cfg(windows)]
        self.resumes.cancel();
    }
}
#[derive(Default)]
pub(super) struct ProcessObserver {
    wake: Arc<Wake>,
    handle: Mutex<Option<JoinHandle<()>>>,
}
impl ProcessObserver {
    pub(super) fn apply_configuration(
        &self,
        settings: &orchbee_local_runtime::UserSettings,
    ) -> bool {
        self.wake.manifests.apply_configuration(settings)
    }
    fn cancel(&self) {
        self.wake.stop.store(true, Ordering::Release);
        self.wake.cancel_reads();
        self.wake.manifests.cancel();
        if let Ok(_guard) = self.wake.wait.lock() {
            self.wake.changed.notify_all();
        }
    }
    fn start(
        &self,
        service: &Arc<LocalTerminalService>,
        current: orchbee_local_runtime::UserSettingsLoad,
        background: bool,
    ) -> io::Result<()> {
        let mut handle = self
            .handle
            .lock()
            .map_err(|_| io::Error::other("Process observer owner poisoned"))?;
        if self.wake.stop.load(Ordering::Acquire) {
            return Err(io::ErrorKind::NotConnected.into());
        }
        if handle.is_some() {
            return Err(io::ErrorKind::AlreadyExists.into());
        }
        let settings = &current.settings;
        // Detection rules live in the data root, whichever settings file the Session reads.
        let root = orchbee_local_runtime::DataRoot::resolve(|name| std::env::var_os(name))
            .ok()
            .map(|root| root.path().to_owned());
        // One startup read freezes the catalog for both rule selection and maintenance.
        let source = updating::configured_source();
        let detector = ScreenDetector::load(
            root.as_deref(),
            updating::remote_cache(root.as_deref(), &source),
        )
        .map_err(io::Error::other)?;
        // Validate the complete authoritative rev0 response before manifest policy or its schedule
        // can become visible. Installation remains after manifest initialization so a later
        // manifest failure cannot leave the configuration owner half-started.
        let configuration = super::configuration::Registry::prepare_initial(&current, true)?;
        self.wake.manifests.initialize_settings(
            root,
            settings,
            &detector,
            source,
            background,
            !cfg!(debug_assertions) && !cfg!(test),
        )?;
        service.configuration.install_initial(configuration)?;
        if !service.apply_configuration(settings) {
            return Err(io::Error::other(
                "Configuration launch policy is unavailable",
            ));
        }
        let service = Arc::downgrade(service);
        let wake = Arc::clone(&self.wake);
        let spawned = thread::Builder::new()
            .name("orchbee-process-observer".into())
            .spawn(move || run(service, wake, detector));
        match spawned {
            Ok(thread) => *handle = Some(thread),
            Err(error) => {
                self.cancel();
                return Err(error);
            }
        }
        Ok(())
    }
    #[cfg(windows)]
    fn stop_before(&self, deadline: Instant) -> io::Result<()> {
        self.cancel();
        loop {
            {
                let mut handle = self
                    .handle
                    .lock()
                    .map_err(|_| io::Error::other("Process observer handle poisoned"))?;
                if handle.as_ref().is_none_or(|h| h.is_finished()) {
                    if let Some(handle) = handle.take() {
                        handle
                            .join()
                            .map_err(|_| io::Error::other("Process observer panicked"))?;
                    }
                    return Ok(());
                }
            }
            if Instant::now() >= deadline {
                return Err(io::ErrorKind::TimedOut.into());
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
}
impl Drop for ProcessObserver {
    fn drop(&mut self) {
        self.cancel();
    }
}
impl LocalTerminalService {
    /// Clean stop reuses the observer's sole cleanup slot after its sampling thread has joined.
    #[cfg(windows)]
    pub(super) fn settle_checkpoint_viewer(&self, new: Option<PendingCleanup>) -> io::Result<()> {
        let observer = &self.process_observer;
        if !observer.wake.stop.load(Ordering::Acquire)
            || observer
                .handle
                .lock()
                .map_err(|_| io::Error::other("observer poisoned"))?
                .is_some()
        {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        let mut slot = observer
            .wake
            .pending
            .lock()
            .map_err(|_| io::Error::other("observer cleanup poisoned"))?;
        if let Some(new) = new {
            debug_assert!(
                slot.is_none(),
                "checkpoint cannot attach before prior cleanup"
            );
            *slot = Some(new);
            return Err(io::Error::other("checkpoint viewer cleanup unconfirmed"));
        }
        if let Some(pending) = slot.take()
            && !pending.settle(self)
        {
            *slot = Some(pending);
            return Err(io::Error::other("checkpoint viewer cleanup unconfirmed"));
        }
        Ok(())
    }

    #[cfg(all(test, windows))]
    pub(super) fn process_observer_turn_for_test(&self, schedule: &mut Schedule) {
        turn(
            self,
            &self.process_observer.wake,
            schedule,
            &ScreenDetector::bundled().unwrap(),
        );
    }
    #[cfg(all(test, windows))]
    pub(super) fn process_observer_pending_for_test(&self) -> bool {
        self.process_observer.wake.pending.lock().unwrap().is_some()
    }
    /// Starts the sole periodic sampler; a stopped or already running owner cannot restart.
    /// `configuration` is the Server's single startup read. Revision zero, diagnostics, every
    /// live profile and the frozen reload location all come from it; after that the sampler
    /// observes the file.
    /// Starts the observer where Hosts sample processes. Elsewhere it installs the startup read as
    /// the Session's configuration and refuses with `Unsupported`; the Session then runs without
    /// an observer, and nothing observes the file.
    pub(crate) fn start_process_observer(
        self: &Arc<Self>,
        configuration: impl FnOnce() -> io::Result<orchbee_local_runtime::UserSettingsLoad>,
        background: bool,
    ) -> io::Result<()> {
        let configuration = configuration()?;
        if let Err(unsupported) = host_sampling::require() {
            self.install_configuration(&configuration)?;
            return Err(unsupported);
        }
        self.start_process_observer_with_configuration(configuration, background)
    }
    pub(crate) fn start_process_observer_with_configuration(
        self: &Arc<Self>,
        configuration: orchbee_local_runtime::UserSettingsLoad,
        background: bool,
    ) -> io::Result<()> {
        self.process_observer.start(self, configuration, background)
    }
    /// Stops new sampling and joins admitted work; unresolved viewer cleanup transfers to Session shutdown.
    #[cfg(windows)]
    pub(crate) fn stop_process_observer_before(&self, deadline: Instant) -> io::Result<()> {
        self.process_observer.stop_before(deadline)
    }
    /// Only the managed Session owner calls this after matching Worker/Host stop plus actual Worker exit.
    #[cfg(windows)]
    pub(crate) fn finish_process_observer_after_worker_exit(
        &self,
        worker: &crate::worker_obcn_carrier::WorkerRequestHandle,
    ) -> io::Result<()> {
        if !self.process_observer.wake.stop.load(Ordering::Acquire)
            || self
                .process_observer
                .handle
                .lock()
                .map_err(|_| io::Error::other("Process observer handle poisoned"))?
                .is_some()
        {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        let mut pending = self
            .process_observer
            .wake
            .pending
            .lock()
            .map_err(|_| io::Error::other("Process observer cleanup poisoned"))?;
        if pending.as_ref().is_some_and(|p| !p.matches_worker(worker)) {
            return Err(io::Error::other(
                "Process observer cleanup belongs to another Worker",
            ));
        }
        *pending = None;
        Ok(())
    }
}
fn select_turn(
    targets: &[ProcessObservationTarget],
    cursor: Option<&Cursor>,
    limit: usize,
) -> Vec<usize> {
    if targets.is_empty() {
        return vec![];
    }
    let start = cursor
        .and_then(|cursor| targets.iter().position(|t| t.key() > *cursor))
        .unwrap_or(0);
    (0..limit.min(targets.len()))
        .map(|n| (start + n) % targets.len())
        .collect()
}
fn turn(
    service: &LocalTerminalService,
    wake: &Wake,
    schedule: &mut Schedule,
    detector: &ScreenDetector,
) -> Duration {
    let deadline = Instant::now() + TURN_BUDGET;
    let pending = match wake.pending.lock() {
        Ok(mut p) => p.take(),
        Err(_) => return DISCOVERY_INTERVAL,
    };
    if let Some(pending) = pending
        && !pending.settle(service)
    {
        *wake
            .pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(pending);
        return DISCOVERY_INTERVAL;
    }
    if wake.stop.load(Ordering::Acquire) {
        return DISCOVERY_INTERVAL;
    }
    let (agents, discovery): (Vec<_>, Vec<_>) = {
        let Ok(mut coordinator) = service.coordinator.lock() else {
            return DISCOVERY_INTERVAL;
        };
        let Ok(targets) = coordinator.process_observation_targets() else {
            return DISCOVERY_INTERVAL;
        };
        targets
            .into_iter()
            .partition(|target| coordinator.process_observation_has_agent(target))
    };
    if agents.is_empty() {
        schedule.agents = None;
    }
    if discovery.is_empty() {
        schedule.discovery = None;
    }
    // Reserve one fair discovery sample, then return to known Agents. Ten idle shells must
    // not force a detected Agent to wait for an entire scan; discovery must not starve either.
    let discovery_limit = if agents.is_empty() {
        TARGETS_PER_TURN
    } else {
        1
    };
    let ordinary = select_turn(&discovery, schedule.discovery.as_ref(), discovery_limit);
    let active = select_turn(
        &agents,
        schedule.agents.as_ref(),
        TARGETS_PER_TURN - ordinary.len(),
    );
    let mut observed_agent = false;
    for (target, is_agent) in ordinary
        .into_iter()
        .map(|i| (&discovery[i], false))
        .chain(active.into_iter().map(|i| (&agents[i], true)))
    {
        // A queued read waits at most one sample; the cursors resume here next turn.
        if wake.stop.load(Ordering::Acquire)
            || (Instant::now() >= deadline && (!is_agent || observed_agent))
            || wake.reads_waiting()
        {
            break;
        }
        observed_agent |= is_agent;
        if is_agent {
            schedule.agents = Some(target.key());
        } else {
            schedule.discovery = Some(target.key());
        }
        if let Some(pending) = service.sample_process_target(target, &wake.stop, detector) {
            *wake
                .pending
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(pending);
            break;
        }
    }
    if agents.is_empty() {
        DISCOVERY_INTERVAL
    } else {
        INTERVAL
    }
}
fn run(service: Weak<LocalTerminalService>, wake: Arc<Wake>, mut detector: ScreenDetector) {
    let _manifest_lifetime = manifests::Lifetime(&wake.manifests);
    let mut schedule = Schedule::default();
    let mut updates = manifests::Worker::default();
    let mut configuration = super::configuration::Sampler::default();
    loop {
        let started = Instant::now();
        let mut interval = DISCOVERY_INTERVAL;
        let service = service.upgrade();
        if service.is_none() {
            wake.stop.store(true, Ordering::Release);
        }
        if wake.stop.load(Ordering::Acquire) {
            wake.manifests.cancel();
            wake.cancel_reads();
        }
        wake.manifests.schedule(Instant::now());
        updates.serve(service.as_deref(), &wake, &mut detector);
        if wake.stop.load(Ordering::Acquire) {
            if !updates.active() {
                return;
            }
        } else if let Some(service) = service.as_deref() {
            // A person waits on a close, so its read goes first.
            close::serve(service, &wake);
            directory::serve(service, &wake);
            #[cfg(windows)]
            agent_resume::serve(service, &wake);
            explanation::serve(service, &wake, &detector);
            service.sample_configuration(&mut configuration, Instant::now());
            interval = turn(service, &wake, &mut schedule, &detector);
        }
        drop(service);
        match wake.wait.lock() {
            // Checked under the wait lock, a read queued after this point still wakes the wait.
            Ok(_guard) if wake.reads_waiting() => {}
            Ok(guard) => {
                // Include sampling time in the cadence instead of adding a fixed wait to it.
                let wait = interval
                    .saturating_sub(started.elapsed())
                    .max(Duration::from_millis(1));
                if wake.changed.wait_timeout(guard, wait).is_err() {
                    wake.stop.store(true, Ordering::Release);
                }
            }
            Err(_) => {
                wake.stop.store(true, Ordering::Release);
                thread::sleep(Duration::from_millis(10));
            }
        }
    }
}
#[cfg(all(test, windows))]
mod tests;
