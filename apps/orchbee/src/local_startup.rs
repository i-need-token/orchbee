use crate::{
    client_view::ClientView,
    local_activation,
    localization::Locale,
    startup_wait::{
        self, InheritedWaitInput, InheritedWaitViewport, StartupWait, WaitPrompt,
        startup_presentable_area,
    },
    terminal_device::InheritedTerminal,
    terminal_lifecycle::{InheritedTerminalModes, TerminalModeGuard},
    termination_signal::TerminationSignal,
    tui_shell,
    workbench_client::{TerminalStatus, WorkbenchClient},
};
use std::{
    io::{self, Write},
    sync::mpsc::{self, Sender},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

// The verdict enum is owned by the shared activation core so the core has
// no dependency back on this adapter; the re-export keeps the TUI call
// sites and the localization routing on their existing path.
pub(crate) use crate::local_activation::StartupFailure;

pub(crate) fn run(locale: Locale, mouse: bool) -> Result<(), StartupFailure> {
    #[cfg(windows)]
    if local_activation::inside_selected_session().map_err(|_| StartupFailure::Runtime)? {
        return Err(StartupFailure::AlreadyInSession);
    }
    let deadline = Instant::now() + Duration::from_secs(20);
    let startup_cwd = std::env::current_dir().ok();
    let inherited = InheritedTerminal::admit().map_err(|_| StartupFailure::Terminal)?;
    // A temporary zero-cell report is not drawable yet but is not a fault
    // either; the bounded wait keeps polling instead of failing admission.
    let reported = match inherited.window_dimensions() {
        Ok(dimensions) => Some(dimensions),
        Err(error) if error.kind() == io::ErrorKind::InvalidData => None,
        Err(_) => return Err(StartupFailure::Terminal),
    };
    let dimensions = match reported {
        Some(dimensions) if startup_presentable_area(dimensions).is_some() => dimensions,
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
    // The shared activation core owns cohort validation, rendezvous, the
    // socket-path budget, and the connect-or-activate loop; the TUI verdict
    // is exactly the `StartupFailure` it always derived.
    let started =
        local_activation::activate_local_site(deadline).map_err(|failure| failure.failure)?;
    #[cfg(windows)]
    let allow_startup_directory = !matches!(
        started.restore_notice,
        Some(orchbee_local_site_runtime::WindowsCheckpointRestoreNotice::Empty)
    );
    #[cfg(not(windows))]
    let allow_startup_directory = false;
    let directory = crate::workbench_client::StartupDirectory::new(
        startup_cwd.as_deref(),
        locale,
        allow_startup_directory && crate::tui_capabilities::Capability::DirectoryInput.available(),
    );
    #[cfg(windows)]
    let notice = started.restore_notice.map(|notice| match notice {
        orchbee_local_site_runtime::WindowsCheckpointRestoreNotice::Recovered => {
            crate::client_view::Notice::CheckpointRecovered
        }
        orchbee_local_site_runtime::WindowsCheckpointRestoreNotice::Empty => {
            crate::client_view::Notice::CheckpointEmpty
        }
    });
    #[cfg(not(windows))]
    let notice = None;
    let mut client = started.client;
    client.set_submission_deadline(Some(deadline));
    let mut workbench_client = WorkbenchClient::start_local(
        client,
        geometry,
        &started.runtime_directory,
        directory.clone(),
    )
    .map_err(|_| StartupFailure::TerminalUnavailable)?;
    let ready = (|| {
        loop {
            local_activation::check_deadline(deadline).map_err(|failure| failure.failure)?;
            if workbench_client.startup_draw_ready() {
                local_activation::check_deadline(deadline).map_err(|failure| failure.failure)?;
                break;
            }
            match workbench_client.status() {
                TerminalStatus::Unavailable => return Err(StartupFailure::TerminalUnavailable),
                TerminalStatus::OutcomeUnknown => return Err(StartupFailure::OutcomeUnknown),
                _ => local_activation::pause(deadline).map_err(|failure| failure.failure)?,
            }
        }
        Ok(())
    })();
    if ready.is_err()
        && let Some(note) = directory
            .failure_note(locale)
            .map_err(|_| StartupFailure::Interface)?
    {
        io::stderr()
            .lock()
            .write_all(note.as_bytes())
            .map_err(|_| StartupFailure::Interface)?;
    }
    ready?;
    workbench_client.finish_local_startup();
    drop(progress);
    tui_shell::run_prepared(
        locale,
        inherited,
        dimensions,
        workbench_client,
        mouse,
        notice,
    )
    .map_err(|_| StartupFailure::Interface)
}

fn await_geometry(inherited: &InheritedTerminal, locale: Locale, deadline: Instant) -> StartupWait {
    let (termination, input, output) = match (
        TerminationSignal::install(),
        InheritedWaitInput::open(inherited),
        inherited.output(),
    ) {
        (Ok(termination), Ok(input), Ok(output)) => (termination, input, output),
        _ => {
            return StartupWait::Fault(io::Error::other(
                "the inherited terminal could not arm the startup wait",
            ));
        }
    };
    let mut input = input;
    let mut output = output;
    let mut viewport = InheritedWaitViewport::new(inherited);
    let mut presentable = startup_presentable_area;
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
