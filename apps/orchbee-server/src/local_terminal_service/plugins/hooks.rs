//! Runs validated plugin startup commands once per managed Session start, and event hooks for
//! journaled semantic events, as ordinary Worker plugin jobs with the action log, cancellation
//! and process owner. Hooks never queue or retry: an occurrence that arrives while the same
//! plugin's hook for that event still runs is dropped with a diagnostic line.

use super::{JobRequestError, command_environment, hex, request_plugin_job};
use crate::local_terminal_service::LocalTerminalService;
use orchbee_capability_node_protocol as node;
use orchbee_local_runtime::{PluginRegistry, PluginRegistryEntry};
use orchbee_server::{SessionChangeSignal, SessionEventJournal};
use orchbee_workbench_protocol::{
    MAXIMUM_EVENT_PAGE, PluginInvocationContext, SemanticEvent, SemanticEventCursor,
    SemanticEventData as Data, SemanticEventFilter, SemanticEventKind as Kind, SemanticEventMatch,
    SemanticEventScope,
};
use std::{
    collections::BTreeMap,
    fmt::Write as _,
    io::{self, Write as _},
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

/// Startup commands carry this trigger as their log command ID and `ORCHBEE_PLUGIN_EVENT`.
const STARTUP: &str = "startup";
/// The Worker admits environment values up to 16 KiB; a larger event payload is never cut.
const MAXIMUM_PAYLOAD: usize = 16 * 1024;
/// Bounds how long a dropped service can leave the owner waiting without a stop request.
const LIVENESS: Duration = Duration::from_secs(1);

/// Events a hook may name. Output changes are excluded for their volume; output-match and
/// scroll observations exist only as subscription probes and never reach the Session journal.
fn deliverable(kind: Kind) -> bool {
    !matches!(
        kind,
        Kind::PaneOutputChanged | Kind::PaneOutputMatched | Kind::PaneScrollChanged
    )
}

/// The managed Session's single hook owner.
#[derive(Default)]
pub(crate) struct PluginHooks {
    handle: Mutex<Option<JoinHandle<()>>>,
    stop: Arc<AtomicBool>,
    signal: Arc<SessionChangeSignal>,
}

impl LocalTerminalService {
    /// Starts the hook owner once the managed Session admits Clients, because plugin commands
    /// may call back into it. A refusal is reported and never blocks the Session.
    pub(crate) fn start_plugin_hooks(self: &Arc<Self>) {
        if self.try_start_plugin_hooks().is_err() {
            report("start", "unavailable", None, None);
        }
    }

    fn try_start_plugin_hooks(self: &Arc<Self>) -> io::Result<()> {
        let journal = self
            .event_journal()
            .ok_or_else(|| io::Error::other("Session event journal unavailable"))?;
        let mut slot = self
            .plugin_hooks
            .handle
            .lock()
            .map_err(|_| io::Error::other("Plugin hook owner poisoned"))?;
        if slot.is_some() {
            return Err(io::ErrorKind::AlreadyExists.into());
        }
        // Events committed after this head reach the hooks, including those during startup.
        journal.watch(&self.plugin_hooks.signal);
        let cursor = journal
            .head()
            .map_err(|_| io::Error::other("Session event journal unavailable"))?;
        let service = Arc::downgrade(self);
        let stop = Arc::clone(&self.plugin_hooks.stop);
        let signal = Arc::clone(&self.plugin_hooks.signal);
        *slot = Some(
            thread::Builder::new()
                .name("orchbee-plugin-hooks".into())
                .spawn(move || run(&service, &journal, cursor, &stop, &signal))?,
        );
        Ok(())
    }

    /// Stops dispatch and joins the owner. Admitted hook jobs stay with the Worker, whose
    /// shutdown cancels them.
    pub(crate) fn stop_plugin_hooks_before(&self, deadline: Instant) -> io::Result<()> {
        self.plugin_hooks.stop.store(true, Ordering::Release);
        self.plugin_hooks.signal.wake();
        loop {
            {
                let mut handle = self
                    .plugin_hooks
                    .handle
                    .lock()
                    .map_err(|_| io::Error::other("Plugin hook owner poisoned"))?;
                if handle.as_ref().is_none_or(JoinHandle::is_finished) {
                    if let Some(handle) = handle.take() {
                        handle
                            .join()
                            .map_err(|_| io::Error::other("Plugin hook owner panicked"))?;
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

fn run(
    service: &Weak<LocalTerminalService>,
    journal: &SessionEventJournal,
    mut cursor: SemanticEventCursor,
    stop: &AtomicBool,
    signal: &SessionChangeSignal,
) {
    let Ok(registry) = PluginRegistry::from_environment(|name| std::env::var_os(name)) else {
        report("start", "registry-unavailable", None, None);
        return;
    };
    let mut hooks = Dispatcher::default();
    {
        let Some(service) = service.upgrade() else {
            return;
        };
        hooks.startup(
            &registry,
            &mut WorkerJobs {
                service: &service,
                stop,
            },
        );
    }
    let filters: Vec<_> = Kind::ALL
        .iter()
        .copied()
        .filter(|kind| deliverable(*kind))
        .map(|event| SemanticEventFilter::Event {
            event,
            scope: SemanticEventScope::All {},
            condition: SemanticEventMatch::Any {},
        })
        .collect();
    // Terminal output wakes every listener without a commit, so only a new commit is read.
    let mut read_commit = None;
    loop {
        let wake = signal.wake_revision();
        if stop.load(Ordering::Acquire) || service.strong_count() == 0 {
            return;
        }
        let commit = signal.commit_revision();
        if read_commit != Some(commit) {
            read_commit = Some(commit);
            let events = read(journal, &mut cursor, &filters);
            if !events.is_empty() {
                let Some(service) = service.upgrade() else {
                    return;
                };
                hooks.events(
                    &registry,
                    &events,
                    &mut WorkerJobs {
                        service: &service,
                        stop,
                    },
                );
            }
        }
        signal.wait(wake, Some(LIVENESS));
    }
}

/// Every retained event after the cursor. A lost range is reported, never reconstructed.
fn read(
    journal: &SessionEventJournal,
    cursor: &mut SemanticEventCursor,
    filters: &[SemanticEventFilter],
) -> Vec<SemanticEvent> {
    let mut events = Vec::new();
    loop {
        match journal.read(cursor, filters, MAXIMUM_EVENT_PAGE) {
            Ok(page) => {
                if page.gap.is_some() {
                    report("event", "history-lost", None, None);
                }
                *cursor = page.cursor;
                events.extend(page.events);
                if !page.more {
                    return events;
                }
            }
            Err(_) => {
                report("event", "journal-unavailable", None, None);
                if let Ok(head) = journal.head() {
                    *cursor = head;
                }
                return events;
            }
        }
    }
}

/// The Worker's answer to one hook submission.
#[derive(Debug, Eq, PartialEq)]
enum Admission {
    Started,
    /// The submission may have been delivered; it is tracked and never resubmitted.
    Unknown,
    Refused(&'static str),
}

/// Worker plugin-job admission as the dispatcher sees it.
trait Jobs {
    fn allocate(&mut self) -> Option<node::OperationId>;
    fn start(&mut self, id: node::OperationId, intent: node::PluginCommandIntent) -> Admission;
    /// Whether the job may still run; an unanswerable query counts as running.
    fn running(&mut self, id: node::OperationId) -> bool;
    fn stopping(&self) -> bool;
}

/// Matches plugins to triggers and keeps one hook run per plugin and event.
#[derive(Default)]
struct Dispatcher {
    /// The jobs started for each plugin and event, kept until a later occurrence finds them done.
    running: BTreeMap<(String, Kind), Vec<node::OperationId>>,
}

impl Dispatcher {
    fn startup(&mut self, registry: &PluginRegistry, jobs: &mut impl Jobs) {
        let Ok(catalog) = registry.list(None) else {
            report("startup", "registry-unavailable", None, None);
            return;
        };
        let context = PluginInvocationContext {
            invocation_source: Some(STARTUP.into()),
            ..PluginInvocationContext::default()
        };
        for entry in catalog.plugins.iter().filter(|entry| runnable(entry)) {
            for command in &entry.manifest.startup {
                if !entry.manifest.supports_windows(&command.platforms) {
                    continue;
                }
                if jobs.stopping() {
                    return;
                }
                submit(
                    registry,
                    entry,
                    STARTUP,
                    command.command.clone(),
                    None,
                    context.clone(),
                    jobs,
                );
            }
        }
    }

    fn events(
        &mut self,
        registry: &PluginRegistry,
        events: &[SemanticEvent],
        jobs: &mut impl Jobs,
    ) {
        let Ok(catalog) = registry.list(None) else {
            report("event", "registry-unavailable", None, None);
            return;
        };
        let plugins: Vec<_> = catalog
            .plugins
            .iter()
            .filter(|entry| runnable(entry))
            .collect();
        self.running
            .retain(|(id, _), _| plugins.iter().any(|entry| entry.manifest.id == *id));
        for event in events {
            let public = event.event.public_name();
            let hooked: Vec<(&PluginRegistryEntry, Vec<&[String]>)> = plugins
                .iter()
                .map(|entry| (*entry, hook_commands(entry, event.event, &public)))
                .filter(|(_, commands)| !commands.is_empty())
                .collect();
            if hooked.is_empty() {
                continue;
            }
            let payload = payload(event, &public);
            let context = context(event);
            for (entry, commands) in hooked {
                if jobs.stopping() {
                    return;
                }
                let key = (entry.manifest.id.clone(), event.event);
                if let Some(ids) = self.running.get(&key)
                    && ids.iter().any(|id| jobs.running(*id))
                {
                    report("event", "dropped", Some(&entry.manifest.id), Some(&public));
                    continue;
                }
                self.running.remove(&key);
                let Some(payload) = payload.as_deref() else {
                    report(
                        "event",
                        "payload-too-large",
                        Some(&entry.manifest.id),
                        Some(&public),
                    );
                    continue;
                };
                let started: Vec<_> = commands
                    .into_iter()
                    .filter_map(|command| {
                        submit(
                            registry,
                            entry,
                            &public,
                            command.to_vec(),
                            Some(payload),
                            context.clone(),
                            jobs,
                        )
                    })
                    .collect();
                if !started.is_empty() {
                    self.running.insert(key, started);
                }
            }
        }
    }
}

fn runnable(entry: &PluginRegistryEntry) -> bool {
    entry.enabled && entry.warning.is_none()
}

/// The entry's Windows hook commands for this event, named publicly (`panel.*`) or by the
/// historical protocol noun (`pane.*`).
fn hook_commands<'a>(
    entry: &'a PluginRegistryEntry,
    kind: Kind,
    public: &str,
) -> Vec<&'a [String]> {
    if !deliverable(kind) {
        return Vec::new();
    }
    entry
        .manifest
        .events
        .iter()
        .filter(|hook| hook.on == public || hook.on == kind.as_str())
        .filter(|hook| entry.manifest.supports_windows(&hook.platforms))
        .map(|hook| hook.command.as_slice())
        .collect()
}

/// The event's public shape, as `orchbee events` reports it.
fn payload(event: &SemanticEvent, public: &str) -> Option<String> {
    let mut value = serde_json::to_value(event).ok()?;
    value["event"] = serde_json::Value::String(public.into());
    serde_json::to_string(&value)
        .ok()
        .filter(|text| text.len() <= MAXIMUM_PAYLOAD)
}

/// Display context naming the event's own Workspace, Tab and Panel where it carries them.
fn context(event: &SemanticEvent) -> PluginInvocationContext {
    let mut context = PluginInvocationContext {
        invocation_source: Some("event".into()),
        workspace_id: event.workspace_id.as_ref().map(ToString::to_string),
        ..PluginInvocationContext::default()
    };
    match &event.data {
        Data::Workspace { workspace } => context.workspace_label = Some(workspace.label.clone()),
        Data::Tab { tab_id, label, .. } => {
            context.tab_id = Some(tab_id.to_string());
            context.tab_label = Some(label.clone());
        }
        Data::Layout { tab_id, .. } => context.tab_id = Some(tab_id.to_string()),
        Data::Pane { pane }
        | Data::Agent { pane, .. }
        | Data::Output { pane, .. }
        | Data::Exited { pane, .. }
        | Data::Scroll { pane, .. } => {
            context.tab_id = Some(pane.tab_id.to_string());
            context.focused_pane_id = Some(pane.panel_id.to_string());
        }
        Data::WorkspaceOrder { .. }
        | Data::WorkspaceMetadata { .. }
        | Data::Worktree { .. }
        | Data::Client { .. } => {}
    }
    context
}

/// Freezes one hook command and submits it once. Returns the job to track when it may run.
fn submit(
    registry: &PluginRegistry,
    entry: &PluginRegistryEntry,
    trigger: &str,
    argv: Vec<String>,
    payload: Option<&str>,
    mut context: PluginInvocationContext,
    jobs: &mut impl Jobs,
) -> Option<node::OperationId> {
    let plugin = Some(entry.manifest.id.as_str());
    let Some(id) = jobs.allocate() else {
        report("submit", "identity-unavailable", plugin, Some(trigger));
        return None;
    };
    context.correlation_id = Some(hex(id.as_bytes()));
    let Ok(mut environment) = command_environment(registry, entry, &context) else {
        report("submit", "source-unavailable", plugin, Some(trigger));
        return None;
    };
    environment.push(("ORCHBEE_PLUGIN_EVENT".into(), trigger.into()));
    if let Some(payload) = payload {
        environment.push(("ORCHBEE_PLUGIN_EVENT_JSON".into(), payload.into()));
    }
    let intent = node::PluginCommandIntent {
        plugin_id: entry.manifest.id.clone(),
        command_id: trigger.into(),
        directory: entry.plugin_root.clone(),
        argv,
        environment,
    };
    if intent.validate().is_err() {
        report("submit", "invalid-intent", plugin, Some(trigger));
        return None;
    }
    match jobs.start(id, intent) {
        Admission::Started => Some(id),
        Admission::Unknown => {
            report("submit", "outcome-unknown", plugin, Some(trigger));
            Some(id)
        }
        Admission::Refused(stage) => {
            report("submit", stage, plugin, Some(trigger));
            None
        }
    }
}

struct WorkerJobs<'a> {
    service: &'a LocalTerminalService,
    stop: &'a AtomicBool,
}

impl WorkerJobs<'_> {
    fn request(
        &mut self,
        id: node::OperationId,
        action: node::PluginJobAction,
    ) -> Result<node::PluginJobOutcome, JobRequestError> {
        let handle = self
            .service
            .workers
            .lock()
            .map_err(|_| JobRequestError::Unavailable)?
            .request_handle()
            .ok_or(JobRequestError::Unavailable)?;
        request_plugin_job(&handle, id, action)
    }
}

impl Jobs for WorkerJobs<'_> {
    fn allocate(&mut self) -> Option<node::OperationId> {
        let value = self
            .service
            .coordinator
            .lock()
            .ok()?
            .allocate_identity_block(1)?;
        node::OperationId::from_bytes(value.to_be_bytes()).ok()
    }

    fn start(&mut self, id: node::OperationId, intent: node::PluginCommandIntent) -> Admission {
        match self.request(id, node::PluginJobAction::Start(intent)) {
            Ok(node::PluginJobOutcome::Started { .. }) => Admission::Started,
            Ok(node::PluginJobOutcome::Refused(node::PluginJobRefusal::Busy))
            | Err(JobRequestError::Busy) => Admission::Refused("busy"),
            Ok(_) => Admission::Refused("refused"),
            Err(JobRequestError::Unknown) => Admission::Unknown,
            Err(JobRequestError::Unavailable) => Admission::Refused("worker-unavailable"),
        }
    }

    fn running(&mut self, id: node::OperationId) -> bool {
        let Some(query) = self.allocate() else {
            return true;
        };
        match self.request(query, node::PluginJobAction::Get { job_id: id }) {
            Ok(node::PluginJobOutcome::Logs { logs, .. }) => {
                logs.iter().any(|log| log.finished_ms.is_none())
            }
            _ => true,
        }
    }

    fn stopping(&self) -> bool {
        self.stop.load(Ordering::Acquire)
    }
}

/// One locale-neutral standard-error line from static stages, a validated plugin ID and an event
/// name. Managed launch discards Server standard error, as for other Server diagnostics.
fn report(operation: &'static str, stage: &'static str, plugin: Option<&str>, event: Option<&str>) {
    let mut line = format!(
        "orchbee-server diagnostic component=plugin-hooks operation={operation} stage={stage}"
    );
    // Writing into a String cannot fail.
    if let Some(plugin) = plugin {
        let _infallible = write!(line, " plugin={plugin}");
    }
    if let Some(event) = event {
        let _infallible = write!(line, " event={event}");
    }
    // A failed diagnostic write has no further owner.
    let _unobserved = writeln!(io::stderr().lock(), "{line}");
}

#[cfg(test)]
mod tests;
