use crate::{
    client_view::ClientView,
    local_client::{LocalClient, LocalClientConnectError},
    localization::Locale,
    startup_wait::{
        self, InheritedWaitInput, InheritedWaitViewport, StartupWait, WaitPrompt,
        standalone_presentable_area,
    },
    terminal_device::InheritedTerminal,
    terminal_lifecycle::{InheritedTerminalModes, TerminalModeGuard},
    terminal_session::{TerminalIntent, TerminalSession, TerminalStatus},
    termination_signal::TerminationSignal,
    tui_shell,
};
use orchbee_local_client_protocol::{ConnectClientError, RejectionReason, unix_socket_path_fits};
use orchbee_local_site_runtime::{
    LockAttempt, ModeCheckedLocalSiteRendezvous, NativeArtifact, NativeCohort, RetainedChild,
    SiteScope,
};
use std::{
    fs::File,
    io::{self, Write},
    path::PathBuf,
    process::{Command, Stdio},
    sync::mpsc::{self, Sender},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StartupFailure {
    Terminal,
    Cohort,
    Runtime,
    SocketPathTooLong,
    RecoveryRequired,
    Incompatible,
    Admission,
    TimedOut,
    TerminalUnavailable,
    OutcomeUnknown,
    Interface,
}

pub(crate) fn run(locale: Locale, mouse: bool) -> Result<(), StartupFailure> {
    let deadline = Instant::now() + Duration::from_secs(20);
    let inherited = InheritedTerminal::admit().map_err(|_| StartupFailure::Terminal)?;
    // A temporary zero-cell report is not drawable yet but is not a fault
    // either; the bounded wait keeps polling instead of failing admission.
    let reported = match inherited.window_dimensions() {
        Ok(dimensions) => Some(dimensions),
        Err(error) if error.kind() == io::ErrorKind::InvalidData => None,
        Err(_) => return Err(StartupFailure::Terminal),
    };
    let dimensions = match reported {
        Some(dimensions) if standalone_presentable_area(dimensions).is_some() => dimensions,
        _ => match await_geometry(&inherited, locale, deadline) {
            StartupWait::Available(dimensions) => dimensions,
            StartupWait::Cancelled => return Ok(()),
            StartupWait::TimedOut => return Err(StartupFailure::TimedOut),
            StartupWait::Fault(error) => return Err(failure(error, StartupFailure::Terminal)),
        },
    };
    let geometry =
        tui_shell::terminal_geometry(dimensions, &ClientView::new(dimensions.columns), None)
            .ok()
            .flatten()
            .ok_or(StartupFailure::Terminal)?;
    let progress = DelayedProgress::start(locale).map_err(|_| StartupFailure::Runtime)?;
    let cohort =
        NativeCohort::current(deadline).map_err(|error| failure(error, StartupFailure::Cohort))?;
    let base = std::env::var_os("ORCHBEE_RUNTIME_BASE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    let scope = SiteScope::parse(&format!("orchbee-native-{}", nix::unistd::geteuid()))
        .map_err(|_| StartupFailure::Runtime)?;
    let site = ModeCheckedLocalSiteRendezvous::prepare_mode_checked(&base, scope)
        .map_err(|_| StartupFailure::Runtime)?;
    // Every endpoint a Site derives that cannot fit this platform's socket
    // address ABI can never bind or connect; report the derived-path budget
    // before any admission attempt collapses it into a generic refusal.
    if !ModeCheckedLocalSiteRendezvous::socket_budget_paths(site.runtime_directory())
        .iter()
        .all(|path| unix_socket_path_fits(path))
    {
        return Err(StartupFailure::SocketPathTooLong);
    }
    let mut client = connect_or_activate(&site, &cohort, deadline)?;
    client.set_submission_deadline(Some(deadline));
    let mut session = TerminalSession::start(client, TerminalIntent::Standalone, geometry)
        .map_err(|_| StartupFailure::TerminalUnavailable)?;
    loop {
        check_deadline(deadline)?;
        if session.startup_draw_ready() {
            check_deadline(deadline)?;
            break;
        }
        match session.status() {
            TerminalStatus::Unavailable => return Err(StartupFailure::TerminalUnavailable),
            TerminalStatus::OutcomeUnknown => return Err(StartupFailure::OutcomeUnknown),
            _ => pause(deadline)?,
        }
    }
    drop(progress);
    tui_shell::run_prepared(locale, inherited, dimensions, session, mouse)
        .map_err(|_| StartupFailure::Interface)
}

fn connect_or_activate(
    site: &ModeCheckedLocalSiteRendezvous,
    cohort: &NativeCohort,
    deadline: Instant,
) -> Result<LocalClient, StartupFailure> {
    loop {
        check_deadline(deadline)?;
        if let Some(client) = connect(site, cohort, deadline)? {
            return Ok(client);
        }
        let _activation = match site
            .try_acquire_activation_lock()
            .map_err(|_| StartupFailure::Runtime)?
        {
            LockAttempt::Acquired(lock) => lock,
            LockAttempt::Contended => {
                pause(deadline)?;
                continue;
            }
        };
        if let Some(client) = connect(site, cohort, deadline)? {
            return Ok(client);
        }
        let marker = site
            .activation_started()
            .map_err(|_| StartupFailure::Runtime)?;
        let mut child = match site
            .try_acquire_site_lifetime_lock()
            .map_err(|_| StartupFailure::Runtime)?
        {
            LockAttempt::Acquired(lifetime) => {
                if marker
                    || site
                        .client_socket_exists()
                        .map_err(|_| StartupFailure::Runtime)?
                    || site
                        .worker_socket_exists()
                        .map_err(|_| StartupFailure::Runtime)?
                {
                    return Err(StartupFailure::RecoveryRequired);
                }
                drop(lifetime);
                check_deadline(deadline)?;
                site.commit_activation()
                    .map_err(|_| StartupFailure::Runtime)?;
                check_deadline(deadline)?;
                Some(RetainedChild::new(
                    Command::new(cohort.executable(NativeArtifact::Server))
                        .arg("--local-site")
                        .arg(site.runtime_directory())
                        .stdin(Stdio::null())
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .spawn()
                        .map_err(|_| StartupFailure::RecoveryRequired)?,
                ))
            }
            LockAttempt::Contended if marker => None,
            LockAttempt::Contended => return Err(StartupFailure::RecoveryRequired),
        };
        loop {
            check_deadline(deadline)?;
            if let Some(client) = connect(site, cohort, deadline)? {
                return Ok(client);
            }
            if let Some(child) = child.as_mut() {
                if child
                    .try_wait()
                    .map_err(|_| StartupFailure::RecoveryRequired)?
                    .is_some()
                {
                    return Err(StartupFailure::RecoveryRequired);
                }
            } else if matches!(
                site.try_acquire_site_lifetime_lock()
                    .map_err(|_| StartupFailure::Runtime)?,
                LockAttempt::Acquired(_)
            ) {
                return Err(StartupFailure::RecoveryRequired);
            }
            pause(deadline)?;
        }
    }
}

fn connect(
    site: &ModeCheckedLocalSiteRendezvous,
    cohort: &NativeCohort,
    deadline: Instant,
) -> Result<Option<LocalClient>, StartupFailure> {
    site.client_socket_exists()
        .map_err(|_| StartupFailure::Runtime)?;
    match LocalClient::connect_before(site.client_socket_path(), cohort.identity(), deadline) {
        Ok(client) => Ok(Some(client)),
        Err(LocalClientConnectError::Admission(ConnectClientError::ConnectFailed(error)))
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
            ) =>
        {
            Ok(None)
        }
        Err(LocalClientConnectError::Admission(ConnectClientError::Rejected(
            RejectionReason::IncompatibleCohort | RejectionReason::IncompatibleProtocol,
        ))) => Err(StartupFailure::Incompatible),
        Err(LocalClientConnectError::Admission(
            ConnectClientError::ConnectFailed(error) | ConnectClientError::Io(error),
        )) => Err(failure(error, StartupFailure::Admission)),
        Err(_) => Err(StartupFailure::Admission),
    }
}
fn await_geometry(inherited: &InheritedTerminal, locale: Locale, deadline: Instant) -> StartupWait {
    let (termination, input, output) = match (
        TerminationSignal::install(),
        InheritedWaitInput::open(inherited),
        inherited.duplicate_output(),
    ) {
        (Ok(termination), Ok(input), Ok(output)) => (termination, input, File::from(output)),
        _ => {
            return StartupWait::Fault(io::Error::other(
                "the inherited terminal could not arm the startup wait",
            ));
        }
    };
    let mut input = input;
    let mut output = output;
    let mut viewport = InheritedWaitViewport::new(inherited);
    let mut presentable = standalone_presentable_area;
    startup_wait::await_presentable_geometry(
        || TerminalModeGuard::enter(InheritedTerminalModes::new(inherited.clone())),
        startup_wait::StartupWaitDeps {
            viewport: &mut viewport,
            input: &mut input,
            output: &mut output,
            termination: &termination,
            clock: &Instant::now,
            deadline,
            prompt: WaitPrompt {
                notice: locale.startup_wait_notice(),
                exit_hint: locale.startup_wait_hint(),
            },
            presentable: &mut presentable,
        },
    )
}

fn check_deadline(deadline: Instant) -> Result<(), StartupFailure> {
    if Instant::now() >= deadline {
        return Err(StartupFailure::TimedOut);
    }
    Ok(())
}
fn pause(deadline: Instant) -> Result<(), StartupFailure> {
    check_deadline(deadline)?;
    thread::sleep(
        Duration::from_millis(10).min(deadline.saturating_duration_since(Instant::now())),
    );
    Ok(())
}
fn failure(error: io::Error, fallback: StartupFailure) -> StartupFailure {
    if error.kind() == io::ErrorKind::TimedOut {
        StartupFailure::TimedOut
    } else {
        fallback
    }
}
struct DelayedProgress {
    cancel: Option<Sender<()>>,
    task: Option<JoinHandle<()>>,
}
impl DelayedProgress {
    fn start(locale: Locale) -> io::Result<Self> {
        let (cancel, receiver) = mpsc::channel();
        let task = thread::Builder::new()
            .name("orchbee-startup-progress".into())
            .spawn(move || {
                if matches!(
                    receiver.recv_timeout(Duration::from_millis(500)),
                    Err(mpsc::RecvTimeoutError::Timeout)
                ) {
                    let _written = io::stderr()
                        .lock()
                        .write_all(locale.startup_progress().as_bytes());
                }
            })?;
        Ok(Self {
            cancel: Some(cancel),
            task: Some(task),
        })
    }
}
impl Drop for DelayedProgress {
    fn drop(&mut self) {
        self.cancel.take();
        if let Some(task) = self.task.take() {
            let _joined = task.join();
        }
    }
}
