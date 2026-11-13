//! `uninstall` and the uninstaller helper `--installer-uninstall-v1`: the inverse of install. One
//! run removes this environment's installed release completely, in the contract's order:
//!
//! a. list what goes and what stays, with each running Session's open terminals and running
//!    Agents, then confirm;
//! b. stop every Session of the environment, including one another installed version runs, then
//!    end the machine hub of its data root;
//! c. disable every Agent integration through the writer's own disable path;
//! d. remove the user `Path` entry the PATH step wrote and the installer entries;
//! e. remove from the data root what the data-root contract lists, or the whole root with
//!    `--delete-data`;
//! f. remove the store root, last.
//!
//! Every step is safe to repeat. A failure reports what was done and what remains and changes
//! nothing further, so a second run continues. The removal plan is computed and fenced before
//! anything changes. The helper form asks nothing and ends no terminal: with one open in any
//! Session it changes nothing. The platform uninstaller that started it and waits for it stays in
//! the store root with its own files, which the uninstaller removes after the helper exits; a
//! person's run removes those files too, by the names the installer's contract gives them, so
//! nothing of the installer outlives its registration.
//!
//! The Agent integration writer runs here, in this process, rather than as a Worker job: every
//! Session, and so every Worker, has stopped by step c.
mod plan;
#[cfg(windows)]
#[path = "uninstall/windows.rs"]
mod platform;
#[cfg(not(windows))]
#[path = "uninstall/unix.rs"]
mod platform;
mod removal;
mod report;
#[cfg(all(test, windows))]
mod tests;

use super::super::{
    Failure, FailureKind, Output,
    console_interrupt::{ArmedSignals, Finish},
    destructive_request, execution, failure_output, output,
};
use crate::release_store::read_pointer;
use crate::{
    agent_integrations::{IntegrationId, Refusal, State, Verdict, model::Availability},
    invocation::OutputFormat,
    localization::Locale,
};
use orchbee_connection_handshake::{CohortIdentity, ProcessState};
use orchbee_local_runtime::{
    Cohort, DataRoot, HOME_VARIABLE, ModeCheckedLocalSessionRendezvous as Rendezvous,
    SessionCatalog,
};
use orchbee_release_records::Pointer;
use plan::{Presence, Roots, Target};
use report::{
    IntegrationLine, IntegrationOutcome, Outcome, Report, SessionLine, SessionOutcome, Stop,
};
use std::{
    io,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

/// The bound for reading the Session catalog and asking each Session about itself.
const INVENTORY_BUDGET: Duration = Duration::from_secs(10);
/// The bound for one Session's stop, as `session stop` uses by default.
const STOP_BUDGET: Duration = Duration::from_secs(65);
/// How long the processes of stopped Sessions and their Clients may take to leave the store.
const EXIT_BUDGET: Duration = Duration::from_secs(10);
/// The bound for ending the machine hub: its drain, its exit and the checks around them.
const HUB_BUDGET: Duration = Duration::from_secs(20);
/// The variable that chooses the store root instead of the default, as install reads it.
const STORE_VARIABLE: &str = "ORCHBEE_INSTALL_DIR";

/// The kinds of background process of this environment that run from the store root, in the order
/// step b ends them. Each kind has one inventory arm in [`background`] and one stop arm in
/// [`end_background`]; a new kind of background process joins here.
#[derive(Clone, Copy)]
enum Background {
    Sessions,
    /// The machine hub of the data root, ended after the Sessions. An interface of theirs still
    /// running then starts no hub in reaction to that ending, and the store wait that follows
    /// covers the interfaces and any hub a client started otherwise.
    Hub,
}

const BACKGROUND: [Background; 2] = [Background::Sessions, Background::Hub];

#[derive(Clone, Copy)]
enum Caller {
    Person { confirm: bool, format: OutputFormat },
    Installer,
}

/// `orchbee uninstall [--delete-data]`.
pub(crate) fn run(
    delete_data: bool,
    confirm: bool,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    execute(delete_data, Caller::Person { confirm, format }, locale)
}

/// `orchbee --installer-uninstall-v1 [--delete-data]`, which a platform uninstaller runs after its
/// own confirmation; it answers in text.
pub(crate) fn installer(delete_data: bool, locale: Locale) -> Output {
    execute(delete_data, Caller::Installer, locale)
}

/// What the run found before changing anything.
struct Inventory {
    roots: Roots,
    data: DataRoot,
    locations: platform::Locations,
    plan: plan::Plan,
    /// This process's parent when its program file is inside the store root: the launcher.
    launcher: Option<platform::StoreProcess>,
    /// The helper form's platform uninstaller, which started this run and waits for it: see
    /// [`waiting_uninstaller`].
    uninstaller: Vec<u32>,
}

fn execute(delete_data: bool, caller: Caller, locale: Locale) -> Output {
    let format = match caller {
        Caller::Person { format, .. } => format,
        Caller::Installer => OutputFormat::Text,
    };
    if let Err(failure) = execution::admit(execution::Capability::ReleaseUpdate) {
        return failure_output(failure, Some(format), locale);
    }
    let mut report = Report::new(delete_data, matches!(caller, Caller::Installer));
    let inventory = match inventory(&mut report, delete_data, caller) {
        Some(inventory) => inventory,
        None => return answer(&report, format, None, locale),
    };
    if let Some(blocked) = blocked(&report, &inventory, caller) {
        let (stop, subject, detail) = blocked;
        report.stop(stop, subject, detail);
        return answer(&report, format, None, locale);
    }
    if let Caller::Person { confirm, format } = caller {
        let listing = report.listing(locale);
        if !orchbee_local_runtime::is_release() && format == OutputFormat::Text {
            // A build that takes --confirm asks nothing; the listing still comes first.
            eprint!("{listing}");
        }
        if let Err(refusal) =
            destructive_request::admit_uninstall(confirm, &listing, format, locale)
        {
            return *refusal;
        }
    }
    let Ok(mut signals) = ArmedSignals::arm_interrupts() else {
        return failure_output(
            Failure::new(FailureKind::InternalFailure),
            Some(format),
            locale,
        );
    };
    steps(&mut report, &inventory, &mut signals);
    // Once the `Path` entry is gone a new terminal no longer finds `orchbee`, so a stop names the
    // program to run again by its full path.
    report.again = inventory
        .launcher
        .as_ref()
        .map(|launcher| launcher.image.clone())
        .or_else(|| std::env::current_exe().ok());
    let finalization = match signals.finish() {
        Ok(Finish::Clear) => None,
        Ok(Finish::Cancelled { finalization }) => {
            report.interrupted = true;
            if report.stop.is_none() {
                report.stop(Stop::Interrupted, "", "");
            }
            Some(finalization)
        }
        Err(_) => None,
    };
    answer(&report, format, finalization, locale)
}

/// Resolves the roots and the entries outside them, fences the plan and reads the Sessions and the
/// Agent integrations, recording each in `report`. `None` when the run stops here.
fn inventory(report: &mut Report, delete_data: bool, caller: Caller) -> Option<Inventory> {
    let deadline = Instant::now() + INVENTORY_BUDGET;
    let store = platform::store_root();
    let data = DataRoot::resolve(|name| std::env::var_os(name));
    let locations = platform::locations();
    let (Ok(store), Ok(data), Ok(locations)) = (store, data, locations) else {
        report.stop(Stop::Location, "", "");
        return None;
    };
    report.store_root = Some(store.clone());
    report.data_root = Some(data.path().to_owned());
    report.path_key = Some(locations.path_key());
    report.shortcut = locations.shortcut().map(Path::to_owned);
    report.registration = Some(locations.registration_key());
    if inside_session() {
        report.stop(Stop::InsideSession, "", "");
        return None;
    }
    let roots = Roots {
        store: store.clone(),
        data: data.path().to_owned(),
        protected: platform::protected_folders(),
    };
    let store_contents = contents(&store, || {
        read_pointer(&store)
            .ok()
            .flatten()
            .is_some_and(|bytes| Pointer::parse(&bytes).is_some())
    });
    let data_contents = contents(data.path(), || data.carries_layout_record());
    let installer = plan::Installer {
        files: platform::INSTALLER_FILES,
        remove: matches!(caller, Caller::Person { .. }),
    };
    let plan = match plan::plan(
        &roots,
        delete_data,
        installer,
        &store_contents,
        &data_contents,
        removal::presence,
    ) {
        Ok(plan) => plan,
        Err(refusal) => {
            let subject = refusal.path().map(report::display).unwrap_or_default();
            // The variable that chose an unmarked root, when one did, is the likely mistake.
            let variable = match refusal {
                plan::Refusal::UnmarkedStore(_) => Some(STORE_VARIABLE),
                plan::Refusal::UnmarkedData(_) => Some(HOME_VARIABLE),
                _ => None,
            }
            .filter(|variable| std::env::var_os(variable).is_some());
            report.stop(
                Stop::Plan(refusal.reason()),
                subject,
                variable.unwrap_or_default(),
            );
            return None;
        }
    };
    report.data = plan
        .data
        .targets
        .iter()
        .map(|target| (target.path.clone(), initial(target)))
        .collect();
    report.store = if plan
        .store
        .targets
        .iter()
        .chain(&plan.store.mark)
        .all(|target| target.presence == Presence::Missing)
    {
        Outcome::Absent
    } else {
        Outcome::NotReached
    };
    report.kept = [&plan.store, &plan.data]
        .into_iter()
        .filter(|part| part.kept > 0)
        .map(|part| (part.root.clone(), part.kept))
        .collect();
    report.path_entry = match platform::path_entry_present(&locations, &store) {
        Ok(false) => Outcome::Absent,
        _ => Outcome::NotReached,
    };
    report.shortcut_outcome = match locations.shortcut().map(removal::presence) {
        None | Some(Presence::Missing) => Outcome::Absent,
        Some(_) => Outcome::NotReached,
    };
    report.registration_outcome = match platform::registration_present(&locations) {
        Ok(false) => Outcome::Absent,
        _ => Outcome::NotReached,
    };
    if background(report, &data, deadline).is_err() || integrations(report, deadline).is_err() {
        report.stop(Stop::Inventory, "", "");
        return None;
    }
    let mut ancestors = platform::ancestors().into_iter().peekable();
    report.npm_carrier = platform::npm_carrier(ancestors.peek());
    let launcher = ancestors.next_if(|parent| platform::inside(&parent.image, &store));
    let uninstaller = match caller {
        Caller::Installer => ancestors
            .filter(|ancestor| waiting_uninstaller(&ancestor.image, &plan.store))
            .map(|ancestor| ancestor.pid)
            .collect(),
        Caller::Person { .. } => Vec::new(),
    };
    Some(Inventory {
        roots,
        data,
        locations,
        plan,
        launcher,
        uninstaller,
    })
}

/// Whether a process on the helper's chain of parents, past the launcher, is the platform
/// uninstaller that started the helper and waits for it: its program lies in the store root but in
/// none of the entries the removal removes, as the installer's own uninstaller does. It holds no
/// file the removal touches, so it never counts as Orchbee running from the store; Orchbee's own
/// programs on that chain, such as the Desktop application, still do.
fn waiting_uninstaller(image: &Path, store: &plan::RootPlan) -> bool {
    platform::inside(image, &store.root)
        && !store.targets.iter().chain(&store.mark).any(|target| {
            platform::same(image, &target.path) || platform::inside(image, &target.path)
        })
}

/// What `root` holds for the plan: its names and whether `marked` finds its mark, read only when
/// it is a real folder.
fn contents(root: &Path, marked: impl FnOnce() -> bool) -> plan::Contents {
    if removal::presence(root) == Presence::Directory {
        plan::Contents {
            names: removal::names(root),
            marked: marked(),
        }
    } else {
        plan::Contents {
            names: Some(Vec::new()),
            marked: false,
        }
    }
}

fn initial(target: &Target) -> Outcome {
    if target.presence == Presence::Missing {
        Outcome::Absent
    } else {
        Outcome::NotReached
    }
}

/// Whether this process runs in a terminal of one of this environment's Sessions, which step b
/// would end with it.
fn inside_session() -> bool {
    let environment =
        orchbee_local_runtime::session_route_environment(|name| std::env::var_os(name));
    environment("ORCHBEE_RUNTIME_SCOPE").is_some() && environment("ORCHBEE_TERMINAL_ID").is_some()
}

/// Every running background process of the environment, kind by kind.
fn background(report: &mut Report, data: &DataRoot, deadline: Instant) -> io::Result<()> {
    for kind in BACKGROUND {
        match kind {
            Background::Sessions => sessions(report, data, deadline)?,
            Background::Hub => report.hub = platform::machine_hub(data, deadline),
        }
    }
    Ok(())
}

/// Every running Session of the environment with its open terminals and, for a Session this build
/// can read, its running Agents.
fn sessions(report: &mut Report, data: &DataRoot, deadline: Instant) -> io::Result<()> {
    if crate::local_activation::runtime_base_unstarted()? {
        return Ok(());
    }
    let (base, _) = crate::local_activation::runtime_location()?;
    let cohort = Cohort::current(deadline).ok();
    let own_cohort = cohort
        .as_ref()
        .and_then(|cohort| CohortIdentity::parse(cohort.identity()).ok());
    for entry in SessionCatalog::list(&base, data, deadline)? {
        let Ok(session) = Rendezvous::from_runtime_directory(&entry.runtime) else {
            continue;
        };
        let Ok(query) =
            super::super::server_query::read(session.client_socket_path(), deadline, None)
        else {
            // No Server answers: nothing of this Session runs, so there is nothing to stop.
            continue;
        };
        let Some(server) = query.processes().first() else {
            continue;
        };
        let same_build = server.product_version.as_str() == env!("CARGO_PKG_VERSION")
            && own_cohort.is_some()
            && server.build.cohort == own_cohort;
        let agents = match (
            &cohort,
            same_build && server.state != ProcessState::Stopping,
        ) {
            (Some(cohort), true) => running_agents(&session, cohort, deadline),
            _ => None,
        };
        report.sessions.push(SessionLine {
            name: entry.name.as_str().to_owned(),
            runtime: entry.runtime.clone(),
            terminals: Some(server.terminals),
            agents,
            other_version: (!same_build).then(|| server.product_version.as_str().to_owned()),
            pids: query
                .processes()
                .iter()
                .map(|process| process.pid.get())
                .collect(),
            outcome: SessionOutcome::NotReached,
        });
    }
    Ok(())
}

/// The Agents running in one Session of this build, read as `agent list` reads them.
fn running_agents(session: &Rendezvous, cohort: &Cohort, deadline: Instant) -> Option<u32> {
    let mut client = crate::local_client::LocalClient::connect_before(
        session.client_socket_path(),
        &crate::local_client::client_offer(Some(cohort.identity())),
        deadline,
    )
    .ok()?;
    let outcome =
        crate::agent_runtime::startup::snapshot::read(&mut client, deadline, None).ok()?;
    let orchbee_workbench_protocol::WorkbenchSnapshotOutcome::Available { snapshot } = outcome
    else {
        return None;
    };
    let checkpoint =
        super::super::agent_projection::read(&mut client, &snapshot, false, deadline, None).ok()?;
    u32::try_from(checkpoint.rows.len()).ok()
}

/// Every provider, marked when its files show an entry of this environment or only its own
/// program can tell; another environment's entry, which disable leaves, marks nothing. This
/// starts no Agent program.
fn integrations(report: &mut Report, deadline: Instant) -> Result<(), ()> {
    let context =
        orchbee_agent_integration_writer::context_before(None, None, deadline).map_err(|_| ())?;
    let rows = orchbee_agent_integration_writer::list_owned(&context);
    report.integrations = IntegrationId::ALL
        .iter()
        .map(|id| IntegrationLine {
            provider: id.provider(),
            listed: rows.iter().any(|(item, own)| {
                item.integration_id == *id
                    && match own {
                        State::Enabled | State::UpdateRequired | State::Conflict => true,
                        State::Unknown => item.availability == Availability::Available,
                        State::Disabled | State::NotInstalled => false,
                    }
            }),
            outcome: IntegrationOutcome::NotReached,
        })
        .collect();
    Ok(())
}

/// What a person must do before anything changes: the helper ends no terminal; no Session may be
/// one this build cannot stop; and no other program may run from the store root, the helper's
/// waiting uninstaller aside.
fn blocked(
    report: &Report,
    inventory: &Inventory,
    caller: Caller,
) -> Option<(Stop, String, String)> {
    let escape = crate::cli_text::escape::escape;
    if matches!(caller, Caller::Installer)
        && let Some(session) = report
            .sessions
            .iter()
            .find(|session| session.terminals != Some(0))
    {
        return Some((
            Stop::TerminalsOpen,
            escape(&session.name),
            session
                .terminals
                .map(|count| count.to_string())
                .unwrap_or_default(),
        ));
    }
    if let Some(session) = report
        .sessions
        .iter()
        .find(|session| session.other_version.is_some() && session.terminals != Some(0))
    {
        return Some((
            Stop::SessionUnstoppable,
            escape(&session.name),
            session.other_version.clone().unwrap_or_default(),
        ));
    }
    // Clients end with their Session, and the Sessions' own processes stop in step b.
    let client = std::env::current_exe()
        .ok()
        .and_then(|path| path.file_name().map(ToOwned::to_owned));
    let mut except: Vec<u32> = report
        .sessions
        .iter()
        .flat_map(|session| session.pids.iter().copied())
        .collect();
    except.extend(inventory.launcher.as_ref().map(|launcher| launcher.pid));
    except.extend(&inventory.uninstaller);
    platform::store_processes(&inventory.roots.store, &except)
        .into_iter()
        .find(|process| process.image.file_name() != client.as_deref())
        .map(|process| {
            (
                Stop::ProgramRunning,
                report::display(&process.image),
                process.pid.to_string(),
            )
        })
}

/// Steps b to f; each records its outcome and the first failure ends the run.
fn steps(report: &mut Report, inventory: &Inventory, signals: &mut ArmedSignals) {
    let _ = end_background(report, inventory, signals)
        .and_then(|()| disable_integrations(report, signals))
        .and_then(|()| remove_entries(report, inventory, signals))
        .and_then(|()| remove_data(report, inventory, signals))
        .and_then(|()| remove_store(report, inventory, signals));
}

/// Ends the run when a console interrupt arrived.
fn check(report: &mut Report, signals: &mut ArmedSignals) -> Result<(), ()> {
    if signals.take().is_ok_and(|signal| signal.is_some()) {
        report.interrupted = true;
        report.stop(Stop::Interrupted, "", "");
        return Err(());
    }
    Ok(())
}

/// Step b: ends every background process of the environment, kind by kind, then waits until nothing
/// but this process, its launcher and the helper's waiting uninstaller runs from the store root. A
/// program that still does, of a kind this build cannot end, stops the run here, before anything
/// else changes, and is named.
fn end_background(
    report: &mut Report,
    inventory: &Inventory,
    signals: &mut ArmedSignals,
) -> Result<(), ()> {
    for kind in BACKGROUND {
        match kind {
            Background::Sessions => stop_sessions(report, signals)?,
            Background::Hub => end_hub(report, inventory, signals)?,
        }
    }
    wait_for_store(report, inventory, signals)
}

fn stop_sessions(report: &mut Report, signals: &mut ArmedSignals) -> Result<(), ()> {
    for index in 0..report.sessions.len() {
        check(report, signals)?;
        let runtime = report.sessions[index].runtime.clone();
        match super::super::server_stop::stop_session(
            &runtime,
            Instant::now() + STOP_BUDGET,
            signals,
        ) {
            super::super::server_stop::SessionEnd::Stopped => {
                report.sessions[index].outcome = SessionOutcome::Stopped;
                report.changed = true;
            }
            super::super::server_stop::SessionEnd::Failed(reason) => {
                report.sessions[index].outcome = SessionOutcome::Failed;
                let name = crate::cli_text::escape::escape(&report.sessions[index].name);
                report.stop(Stop::Sessions, name, reason);
                return Err(());
            }
        }
    }
    Ok(())
}

/// Ends the machine hub, whether or not the inventory found one, since a Client may have started
/// one since. A hub that will not end is left for the store wait, which names it.
fn end_hub(
    report: &mut Report,
    inventory: &Inventory,
    signals: &mut ArmedSignals,
) -> Result<(), ()> {
    check(report, signals)?;
    if platform::end_machine_hub(&inventory.data, Instant::now() + HUB_BUDGET) {
        report.changed = true;
    }
    Ok(())
}

/// Waits for the stopped processes and their Clients to leave the store root.
fn wait_for_store(
    report: &mut Report,
    inventory: &Inventory,
    signals: &mut ArmedSignals,
) -> Result<(), ()> {
    let deadline = Instant::now() + EXIT_BUDGET;
    loop {
        check(report, signals)?;
        let Some(process) = still_running(inventory) else {
            return Ok(());
        };
        if Instant::now() >= deadline {
            report.stop(
                Stop::StillRunning,
                report::display(&process.image),
                process.pid.to_string(),
            );
            return Err(());
        }
        thread::sleep(Duration::from_millis(100));
    }
}

/// A process other than this one, its launcher and the uninstaller waiting for it whose program
/// file is inside the store root.
fn still_running(inventory: &Inventory) -> Option<platform::StoreProcess> {
    let except: Vec<u32> = inventory
        .launcher
        .iter()
        .map(|launcher| launcher.pid)
        .chain(inventory.uninstaller.iter().copied())
        .collect();
    platform::store_processes(&inventory.roots.store, &except)
        .into_iter()
        .next()
}

/// Step c: the writer's disable for every provider, whether or not anything shows it enabled; the
/// writer leaves every other part of an Agent's configuration as it was.
fn disable_integrations(report: &mut Report, signals: &mut ArmedSignals) -> Result<(), ()> {
    for index in 0..report.integrations.len() {
        check(report, signals)?;
        let Some(id) = IntegrationId::parse_provider(report.integrations[index].provider) else {
            continue;
        };
        let Ok(context) = orchbee_agent_integration_writer::context(None, None) else {
            report.integrations[index].outcome = IntegrationOutcome::Failed;
            report.stop(Stop::Integrations, id.provider(), "home-unavailable");
            return Err(());
        };
        let mutation = orchbee_agent_integration_writer::mutate(
            &context,
            id,
            orchbee_agent_integration_writer::Direction::Disable,
        );
        if mutation.effect_begun {
            report.changed = true;
        }
        let program_missing = mutation
            .observed
            .as_ref()
            .is_some_and(|item| item.availability == Availability::NotFound);
        let (outcome, detail) = match mutation.verdict {
            Verdict::Completed if mutation.attempted.is_empty() => {
                (IntegrationOutcome::NotEnabled, "")
            }
            Verdict::Completed => (IntegrationOutcome::Disabled, ""),
            Verdict::Refused(Refusal::ManualActionRequired) => (IntegrationOutcome::NotManaged, ""),
            Verdict::Refused(Refusal::InspectionFailed) if program_missing => {
                (IntegrationOutcome::ProgramNotFound, "")
            }
            Verdict::Refused(refusal) => (IntegrationOutcome::Failed, refusal.as_str()),
            Verdict::Failed(code) => (IntegrationOutcome::Failed, code.as_str()),
            Verdict::Unknown => (IntegrationOutcome::Failed, "unknown"),
            Verdict::Cancelled => (IntegrationOutcome::Failed, "cancelled"),
        };
        report.integrations[index].outcome = outcome;
        if outcome == IntegrationOutcome::Failed {
            report.stop(Stop::Integrations, id.provider(), detail);
            return Err(());
        }
    }
    Ok(())
}

/// Step d: the user `Path` entry, then the Start menu shortcut and the uninstall registration.
fn remove_entries(
    report: &mut Report,
    inventory: &Inventory,
    signals: &mut ArmedSignals,
) -> Result<(), ()> {
    check(report, signals)?;
    if report.path_entry != Outcome::Absent {
        if remove_path_entry(&inventory.locations, &inventory.roots.store).is_err() {
            report.path_entry = Outcome::Failed;
            let key = report.path_key.clone().unwrap_or_default();
            report.stop(Stop::PathEntry, key, "");
            return Err(());
        }
        report.path_entry = Outcome::Removed;
        report.changed = true;
    }
    if report.shortcut_outcome != Outcome::Absent
        && let Some(shortcut) = inventory.locations.shortcut()
    {
        // The shortcut is one file or link; a folder by that name is not the installer's.
        let removed = match removal::presence(shortcut) {
            Presence::Missing => Ok(()),
            Presence::File | Presence::Link => std::fs::remove_file(shortcut),
            Presence::Directory | Presence::Unreadable => Err(io::ErrorKind::InvalidData.into()),
        };
        if removed.is_err() {
            report.shortcut_outcome = Outcome::Failed;
            report.stop(Stop::InstallerEntries, report::display(shortcut), "");
            return Err(());
        }
        report.shortcut_outcome = Outcome::Removed;
        report.changed = true;
    }
    if report.registration_outcome != Outcome::Absent {
        if platform::remove_registration(&inventory.locations).is_err() {
            report.registration_outcome = Outcome::Failed;
            let key = report.registration.clone().unwrap_or_default();
            report.stop(Stop::InstallerEntries, key, "");
            return Err(());
        }
        report.registration_outcome = Outcome::Removed;
        report.changed = true;
    }
    Ok(())
}

/// Removes the entries the PATH step wrote by the exact inverse of its edit, keeping every other
/// entry byte for byte and the value's registry type. Nothing is broadcast.
fn remove_path_entry(locations: &platform::Locations, root: &Path) -> io::Result<()> {
    let root = root.to_str().ok_or(io::ErrorKind::InvalidInput)?;
    super::platform::edit_user_path(locations.environment(), &|current| {
        super::path_value::removal(current, root)
    })
}

/// Step e: the plan's data entries in order, the layout record last with `--delete-data`, then
/// the data root when nothing else is left in it.
fn remove_data(
    report: &mut Report,
    inventory: &Inventory,
    signals: &mut ArmedSignals,
) -> Result<(), ()> {
    let part = &inventory.plan.data;
    for (index, target) in part.targets.iter().enumerate() {
        check(report, signals)?;
        // Read again: admission's destructive record may have created `logs` since the plan.
        if removal::presence(&target.path) == Presence::Missing {
            report.data[index].1 = Outcome::Absent;
            continue;
        }
        let leftovers = fenced_removal(report, inventory, target)?;
        report.changed = true;
        if leftovers.is_empty() {
            report.data[index].1 = Outcome::Removed;
        } else {
            report.data[index].1 = Outcome::Failed;
            report.remaining = leftovers
                .files
                .into_iter()
                .chain(leftovers.folders)
                .collect();
            report.stop(Stop::Data, report::display(&target.path), "");
            return Err(());
        }
    }
    if part.remove_root {
        remove_root(report, &part.root, Stop::Data)?;
    }
    Ok(())
}

/// Step f: the store's entries go now except what this process and its launcher hold, the pointer
/// after them, and the root when nothing else is left in it; what the two hold, the pointer and the
/// root go once both have exited. While another program runs from the store nothing in it is
/// touched, so the installation stays whole and a second run finishes; a removal that still leaves
/// files another program holds names that program and never reports success.
fn remove_store(
    report: &mut Report,
    inventory: &Inventory,
    signals: &mut ArmedSignals,
) -> Result<(), ()> {
    check(report, signals)?;
    let part = &inventory.plan.store;
    let present = |target: &&Target| removal::presence(&target.path) != Presence::Missing;
    if !part
        .targets
        .iter()
        .chain(&part.mark)
        .any(|target| present(&target))
    {
        // Nothing of the store's layout is left: an earlier run finished, or the root held none.
        report.store = Outcome::Absent;
        return Ok(());
    }
    if let Some(process) = still_running(inventory) {
        report.stop(
            Stop::StillRunning,
            report::display(&process.image),
            process.pid.to_string(),
        );
        return Err(());
    }
    let mut leftovers = removal::Leftovers::default();
    for target in part.targets.iter().filter(present) {
        check(report, signals)?;
        leftovers.extend(fenced_removal(report, inventory, target)?);
        report.changed = true;
    }
    let mark = part.mark.as_ref().filter(present);
    if leftovers.is_empty() {
        if let Some(mark) = mark {
            leftovers.extend(fenced_removal(report, inventory, mark)?);
        }
        if leftovers.is_empty() {
            if part.remove_root && remove_root(report, &part.root, Stop::Store).is_err() {
                report.store = Outcome::Failed;
                return Err(());
            }
            report.store = Outcome::Removed;
            return Ok(());
        }
    }
    // Only the folders of the running Client and of the launcher may still hold anything.
    let held: Vec<PathBuf> = std::env::current_exe()
        .ok()
        .into_iter()
        .chain(
            inventory
                .launcher
                .iter()
                .map(|launcher| launcher.image.clone()),
        )
        .filter_map(|image| image.parent().map(Path::to_owned))
        .filter(|folder| platform::inside(folder, &part.root))
        .collect();
    // The running program's path may spell letter case differently from the folder walk.
    let same = platform::same;
    let explained = leftovers.files.iter().all(|file| {
        file.parent()
            .is_some_and(|parent| held.iter().any(|folder| same(parent, folder)))
    }) && leftovers.folders.iter().all(|folder| {
        held.iter()
            .any(|held| same(held, folder) || platform::inside(held, folder))
    });
    // A program started since the check may hold a file the removal would wait for in vain.
    let running = still_running(inventory);
    let marks: Vec<PathBuf> = mark.iter().map(|mark| mark.path.clone()).collect();
    if explained
        && running.is_none()
        && platform::schedule_removal(
            &leftovers.files,
            &leftovers.folders,
            &marks,
            part.remove_root.then_some(part.root.as_path()),
        )
        .is_ok()
    {
        report.store = Outcome::Scheduled;
        return Ok(());
    }
    report.store = Outcome::Failed;
    report.remaining = leftovers
        .files
        .into_iter()
        .chain(leftovers.folders)
        .collect();
    match running {
        Some(process) => report.stop(
            Stop::StillRunning,
            report::display(&process.image),
            process.pid.to_string(),
        ),
        None => report.stop(Stop::Store, report::display(&part.root), ""),
    }
    Err(())
}

/// Removes a root that carried its mark, once nothing is left in it; whatever else it holds stays,
/// and the plan counted it.
fn remove_root(report: &mut Report, root: &Path, stop: Stop) -> Result<(), ()> {
    if removal::remove_empty_folder(root).is_err() {
        report.remaining = vec![root.to_owned()];
        report.stop(stop, report::display(root), "");
        return Err(());
    }
    Ok(())
}

/// Fences `target`, and the entry it removes last, again against what exists now, then removes it
/// without following a link.
fn fenced_removal(
    report: &mut Report,
    inventory: &Inventory,
    target: &Target,
) -> Result<removal::Leftovers, ()> {
    let current = |path: PathBuf| Target {
        presence: removal::presence(&path),
        path,
        last: None,
    };
    let checked = [
        Some(current(target.path.clone())),
        target.last.map(|last| current(target.path.join(last))),
    ];
    if let Err(refusal) = plan::fence(
        &inventory.roots,
        checked.iter().flatten(),
        &removal::presence,
    ) {
        let subject = refusal.path().map(report::display).unwrap_or_default();
        report.stop(Stop::Plan(refusal.reason()), subject, "");
        return Err(());
    }
    Ok(match target.last {
        Some(last) => removal::remove_tree_last(&target.path, last),
        None => removal::remove_tree(&target.path),
    })
}

/// The run's output: the document under JSON, the final message under text, and the diagnostic on
/// standard error after a stop.
fn answer(
    report: &Report,
    format: OutputFormat,
    finalization: Option<super::super::SourceFinalization>,
    locale: Locale,
) -> Output {
    let status = report.status();
    let failed = status != 0;
    let encoded = match format {
        OutputFormat::Json => output::encode_document(&report.document()),
        // A failure writes nothing to standard output under text.
        OutputFormat::Text if failed => output::encode_text(""),
        OutputFormat::Text => output::encode_text(&report.done(locale)),
    };
    let mut answer = match encoded {
        Ok(stdout) => Output {
            status,
            stdout,
            stderr: if failed {
                report.diagnostic(locale)
            } else {
                String::new()
            },
            finalization: None,
        },
        Err(_) => failure_output(
            Failure::new(FailureKind::OutputTooLarge),
            Some(format),
            locale,
        ),
    };
    answer.finalization = finalization;
    answer
}
