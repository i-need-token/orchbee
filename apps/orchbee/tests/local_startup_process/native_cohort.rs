use nix::{
    errno::Errno,
    poll::{PollFd, PollFlags, PollTimeout, poll},
    unistd::{Pid, geteuid},
};
use orchbee_local_runtime::{
    Cohort, HOST_ENDPOINT_ID, ModeCheckedLocalSessionRendezvous, SessionScope,
};
use rustix::process::{Pid as RustixPid, PidfdFlags, Signal, pidfd_open, pidfd_send_signal};
use sha2::{Digest, Sha256};
use std::{
    ffi::{OsStr, OsString},
    fs, io,
    os::{
        fd::{AsFd, OwnedFd},
        unix::{
            ffi::OsStrExt,
            fs::{DirBuilderExt, MetadataExt, PermissionsExt},
        },
    },
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

const BINARIES: [&str; 4] = [
    "orchbee",
    "orchbee-server",
    "orchbee-worker",
    "orchbee-terminal-host",
];
static NEXT: AtomicU64 = AtomicU64::new(0);
const CLEANUP_TIMEOUT: Duration = Duration::from_secs(3);
const GRACEFUL_CLEANUP_TIMEOUT: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Eq, PartialEq)]
enum ManagedRole {
    Server,
    Worker,
    TerminalHost,
}

impl ManagedRole {
    const PARENT_FIRST: [Self; 3] = [Self::Server, Self::Worker, Self::TerminalHost];

    fn from_name(name: &str) -> Self {
        match name {
            "orchbee-server" => Self::Server,
            "orchbee-worker" => Self::Worker,
            "orchbee-terminal-host" => Self::TerminalHost,
            _ => panic!("unknown managed role {name:?}"),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Server => "orchbee-server",
            Self::Worker => "orchbee-worker",
            Self::TerminalHost => "orchbee-terminal-host",
        }
    }
}

pub(super) struct ManagedProcess {
    pid: Pid,
    pidfd: OwnedFd,
}

#[derive(Default)]
struct ManagedProcessScan {
    processes: Vec<ManagedProcess>,
    unsettled: bool,
}

enum ProcessVerification {
    Verified(ManagedProcess),
    Gone,
    Unsettled,
}

impl ManagedProcess {
    pub(super) fn pid(&self) -> Pid {
        self.pid
    }

    pub(super) fn terminate(&self) -> io::Result<()> {
        self.signal(Signal::TERM)
    }

    pub(super) fn exited(&self) -> io::Result<bool> {
        let mut descriptor = [PollFd::new(self.pidfd.as_fd(), PollFlags::POLLIN)];
        poll(&mut descriptor, PollTimeout::ZERO).map_err(io::Error::from)?;
        let events = descriptor[0].revents().unwrap_or(PollFlags::POLLNVAL);
        if events.contains(PollFlags::POLLNVAL) {
            return Err(io::Error::other("pidfd became invalid"));
        }
        Ok(events.intersects(PollFlags::POLLIN | PollFlags::POLLHUP | PollFlags::POLLERR))
    }

    fn signal(&self, signal: Signal) -> io::Result<()> {
        match pidfd_send_signal(&self.pidfd, signal) {
            Ok(()) | Err(rustix::io::Errno::SRCH) => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

pub(super) struct Fixture {
    pub(super) cohort: PathBuf,
    pub(super) base: PathBuf,
    cleanup_armed: bool,
    authorized_runtimes: Mutex<Vec<PathBuf>>,
}

impl Fixture {
    pub(super) fn new() -> Self {
        let id = format!(
            "{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        Self::create(
            PathBuf::from(format!("/tmp/obc-{id}")),
            PathBuf::from(format!("/tmp/obs-{id}")),
        )
    }

    /// An explicit real Workspace is a prerequisite for terminal interaction witnesses.
    /// Empty-startup/admission witnesses keep using `new` and never seed membership.
    pub(super) fn with_workspace() -> Self {
        use orchbee_workbench_protocol::{
            LaunchOptions, WorkbenchAction, WorkbenchCommandOutcome, WorkbenchDirectoryPath,
            WorkbenchLabel, WorkbenchTerminalGeometry, WorkbenchTerminalOutcome,
            WorkspacePlacement,
        };
        let fixture = Self::new();
        let child = crate::support::orchbee_command(fixture.cohort.join("orchbee"))
            .args(["server", "start"])
            .env("ORCHBEE_RUNTIME_BASE", &fixture.base)
            .env("SHELL", "/bin/sh")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let output =
            super::support::bounded_wait_with_output(child, super::STARTUP_OR_CREATE_TIMEOUT)
                .unwrap_or_else(|error| panic!("Workspace fixture startup: {error}"));
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let deadline = Instant::now() + super::STARTUP_OR_CREATE_TIMEOUT;
        let mut peer = super::workbench_peer::WorkbenchPeer::connect(&fixture, deadline).unwrap();
        peer.reserve_fixture_request_range();
        let outcome = peer
            .prepare(
                WorkbenchAction::WorkspaceLaunch {
                    placement: WorkspacePlacement::Ungrouped {},
                    label: WorkbenchLabel::parse("Workspace").unwrap(),
                    automatic_label: false,
                    geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
                    launch: None,
                    profile: None,
                    options: Box::new(LaunchOptions {
                        cwd: Some(
                            WorkbenchDirectoryPath::parse(fixture.base.to_str().unwrap()).unwrap(),
                        ),
                        ..LaunchOptions::default()
                    }),
                },
                deadline,
            )
            .unwrap();
        assert!(
            matches!(
                outcome,
                WorkbenchCommandOutcome::WorkspaceLaunched {
                    terminal_outcome: WorkbenchTerminalOutcome::Started { .. },
                    ..
                }
            ),
            "{outcome:?}"
        );
        fixture
    }

    fn create(cohort: PathBuf, base: PathBuf) -> Self {
        reserve_fixture_directories(&cohort, &base).unwrap_or_else(|error| {
            panic!(
                "reserve fixture directories {} and {}: {error}",
                cohort.display(),
                base.display()
            )
        });
        let runtime = runtime_for_base(&base);
        let fixture = Self {
            cohort,
            base,
            cleanup_armed: true,
            authorized_runtimes: Mutex::new(vec![runtime]),
        };
        let binaries = Path::new(env!("CARGO_BIN_EXE_orchbee")).parent().unwrap();
        let staged = std::env::var_os("ORCHBEE_TEST_NATIVE_COHORT").map(PathBuf::from);
        if let Some(staged) = &staged {
            Cohort::load(staged, Instant::now() + Duration::from_secs(20))
                .expect("valid staged test cohort");
        }
        for name in BINARIES {
            let source = if let Some(staged) = &staged {
                staged.join(name)
            } else if name == "orchbee-terminal-host" {
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../terminal-host/zig-out/bin")
                    .join(name)
            } else {
                binaries.join(name)
            };
            fs::copy(&source, fixture.cohort.join(name)).unwrap_or_else(|error| {
                panic!("build native fixture {}: {error}", source.display())
            });
            fs::set_permissions(fixture.cohort.join(name), fs::Permissions::from_mode(0o700))
                .unwrap();
        }
        fixture.manifest();
        fixture
    }

    pub(super) fn manifest(&self) {
        let mut artifacts = serde_json::Map::new();
        for name in BINARIES {
            artifacts.insert(
                name.into(),
                serde_json::Value::String(format!(
                    "{:x}",
                    Sha256::digest(fs::read(self.cohort.join(name)).unwrap())
                )),
            );
        }
        // A controlled dummy digest stands in for the adapters payload — the
        // fixture stages no sidecar, so cohort admission stays lazy. A real
        // staged cohort under ORCHBEE_TEST_NATIVE_COHORT is only validated by
        // the load above; its manifest is never copied into the fixture.
        artifacts.insert(
            "agent-observation-adapters".into(),
            serde_json::Value::String(format!(
                "{:x}",
                Sha256::digest(b"staged stand-in adapters\n")
            )),
        );
        let manifest = self.cohort.join("cohort.json");
        fs::write(
            &manifest,
            serde_json::to_vec(&serde_json::json!({"schemaVersion":1,"artifacts":artifacts}))
                .unwrap(),
        )
        .unwrap();
        // Cohort rejects group/other-writable manifests; pin the
        // permissions independently of the inherited process umask.
        fs::set_permissions(&manifest, fs::Permissions::from_mode(0o600)).unwrap();
    }

    pub(super) fn runtime(&self) -> PathBuf {
        runtime_for_base(&self.base)
    }

    pub(super) fn register_runtime_base(&self, base: &Path) {
        let runtime = runtime_for_base(base);
        let mut authorized = self.authorized_runtimes.lock().unwrap();
        if !authorized.contains(&runtime) {
            authorized.push(runtime);
        }
    }

    pub(super) fn session(&self) -> ModeCheckedLocalSessionRendezvous {
        ModeCheckedLocalSessionRendezvous::prepare_mode_checked(
            &self.base,
            SessionScope::parse(&format!("orchbee-native-{}", geteuid())).unwrap(),
        )
        .unwrap()
    }

    pub(super) fn processes(&self, name: &str) -> Vec<Pid> {
        self.process_handles(name)
            .expect("scan managed fixture processes")
            .into_iter()
            .map(|process| process.pid())
            .collect()
    }

    pub(super) fn process_handles(&self, name: &str) -> io::Result<Vec<ManagedProcess>> {
        let scan = self.managed_processes(ManagedRole::from_name(name))?;
        if scan.unsettled {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        Ok(scan.processes)
    }

    fn managed_processes(&self, role: ManagedRole) -> io::Result<ManagedProcessScan> {
        let executable = self.cohort.join(role.name());
        let commands = self.expected_commands(role);
        let mut scan = ManagedProcessScan::default();
        for entry in fs::read_dir("/proc")? {
            let entry = entry?;
            let Some(pid) = entry
                .file_name()
                .to_str()
                .and_then(|value| value.parse::<i32>().ok())
            else {
                continue;
            };
            let initial_command = fs::read(entry.path().join("cmdline")).ok();
            let initial_executable = fs::read_link(entry.path().join("exe")).ok();
            if initial_command
                .as_deref()
                .and_then(|command| command_argument(command, 0))
                != Some(executable.as_os_str().as_bytes())
                && initial_executable.as_deref() != Some(executable.as_path())
            {
                continue;
            }
            match open_verified_process(pid, &executable, &commands)? {
                ProcessVerification::Verified(process) => scan.processes.push(process),
                ProcessVerification::Gone => {}
                ProcessVerification::Unsettled => scan.unsettled = true,
            }
        }
        Ok(scan)
    }

    fn expected_commands(&self, role: ManagedRole) -> Vec<Vec<OsString>> {
        let executable = self.cohort.join(role.name()).into_os_string();
        self.authorized_runtimes
            .lock()
            .unwrap()
            .iter()
            .map(|runtime| match role {
                ManagedRole::Server | ManagedRole::Worker => vec![
                    executable.clone(),
                    OsString::from("--runtime-directory"),
                    runtime.as_os_str().to_owned(),
                ],
                ManagedRole::TerminalHost => vec![
                    executable.clone(),
                    OsString::from("serve"),
                    OsString::from("--runtime-directory"),
                    ModeCheckedLocalSessionRendezvous::host_runtime_directory(runtime)
                        .into_os_string(),
                    OsString::from("--endpoint-id"),
                    OsString::from(HOST_ENDPOINT_ID),
                ],
            })
            .collect()
    }

    fn stop_managed_processes(&self) -> bool {
        let cleanup_deadline = Instant::now() + CLEANUP_TIMEOUT;
        let mut scan = |role| self.managed_processes(role);
        stop_roles_before(&mut scan, cleanup_deadline)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if self.cleanup_armed && self.stop_managed_processes() {
            let _result = fs::remove_dir_all(&self.cohort);
            let _result = fs::remove_dir_all(&self.base);
        }
    }
}

fn create_private_directory(path: &Path) -> io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.mode(0o700).create(path)
}

fn reserve_fixture_directories(cohort: &Path, base: &Path) -> io::Result<()> {
    create_private_directory(cohort)?;
    if let Err(error) = create_private_directory(base) {
        let _result = fs::remove_dir_all(cohort);
        return Err(error);
    }
    Ok(())
}

pub(super) fn runtime_for_base(base: &Path) -> PathBuf {
    base.join(format!("orchbee-native-{}", geteuid()))
}

fn command_argument(command: &[u8], index: usize) -> Option<&[u8]> {
    command
        .strip_suffix(&[0])?
        .split(|byte| *byte == 0)
        .nth(index)
}

fn command_matches(command: &[u8], expected: &[OsString]) -> bool {
    let Some(command) = command.strip_suffix(&[0]) else {
        return false;
    };
    command
        .split(|byte| *byte == 0)
        .eq(expected.iter().map(|value| value.as_os_str().as_bytes()))
}

fn open_verified_process(
    raw_pid: i32,
    executable: &Path,
    expected_commands: &[Vec<OsString>],
) -> io::Result<ProcessVerification> {
    let Some(rustix_pid) = RustixPid::from_raw(raw_pid) else {
        return Ok(ProcessVerification::Gone);
    };
    let pidfd = match pidfd_open(rustix_pid, PidfdFlags::empty()) {
        Ok(pidfd) => pidfd,
        Err(rustix::io::Errno::SRCH) => return Ok(ProcessVerification::Gone),
        Err(error) => return Err(error.into()),
    };
    let process = ManagedProcess {
        pid: Pid::from_raw(raw_pid),
        pidfd,
    };
    if process.exited()? {
        return Ok(ProcessVerification::Gone);
    }
    let proc = PathBuf::from(format!("/proc/{raw_pid}"));
    let observed_executable = match fs::read_link(proc.join("exe")) {
        Ok(executable) => executable,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return if process.exited()? {
                Ok(ProcessVerification::Gone)
            } else {
                Ok(ProcessVerification::Unsettled)
            };
        }
        Err(error) => return Err(error),
    };
    let observed_command = match fs::read(proc.join("cmdline")) {
        Ok(command) => command,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return if process.exited()? {
                Ok(ProcessVerification::Gone)
            } else {
                Ok(ProcessVerification::Unsettled)
            };
        }
        Err(error) => return Err(error),
    };
    let observed_metadata = match fs::metadata(proc.join("exe")) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return if process.exited()? {
                Ok(ProcessVerification::Gone)
            } else {
                Ok(ProcessVerification::Unsettled)
            };
        }
        Err(error) => return Err(error),
    };
    let expected_metadata = fs::metadata(executable)?;
    if process.exited()?
        || observed_executable != executable
        || observed_metadata.dev() != expected_metadata.dev()
        || observed_metadata.ino() != expected_metadata.ino()
        || !expected_commands
            .iter()
            .any(|expected| command_matches(&observed_command, expected))
    {
        return if process.exited()? {
            Ok(ProcessVerification::Gone)
        } else {
            Ok(ProcessVerification::Unsettled)
        };
    }
    Ok(ProcessVerification::Verified(process))
}

fn stop_until_quiescent_before(
    scan: &mut impl FnMut() -> io::Result<ManagedProcessScan>,
    signal: Signal,
    deadline: Instant,
) -> bool {
    loop {
        if Instant::now() >= deadline {
            return false;
        }
        let scan = match scan() {
            Ok(scan) => scan,
            Err(_) => return false,
        };
        if Instant::now() >= deadline {
            return false;
        }
        if scan.processes.is_empty() && !scan.unsettled {
            return true;
        }
        if scan.processes.is_empty() {
            thread::yield_now();
            continue;
        }
        for process in &scan.processes {
            if Instant::now() >= deadline {
                return false;
            }
            if process.signal(signal).is_err() {
                return false;
            }
        }
        if !wait_for_any_exit_before(&scan.processes, deadline) {
            return false;
        }
    }
}

fn stop_roles_before(
    scan: &mut impl FnMut(ManagedRole) -> io::Result<ManagedProcessScan>,
    deadline: Instant,
) -> bool {
    loop {
        if Instant::now() >= deadline {
            return false;
        }
        for role in ManagedRole::PARENT_FIRST {
            if Instant::now() >= deadline {
                return false;
            }
            let graceful_deadline = (Instant::now() + GRACEFUL_CLEANUP_TIMEOUT).min(deadline);
            let mut role_scan = || scan(role);
            if !stop_until_quiescent_before(&mut role_scan, Signal::TERM, graceful_deadline)
                && !stop_until_quiescent_before(&mut role_scan, Signal::KILL, deadline)
            {
                return false;
            }
        }
        if Instant::now() >= deadline {
            return false;
        }
        let mut found = false;
        for role in ManagedRole::PARENT_FIRST {
            if Instant::now() >= deadline {
                return false;
            }
            match scan(role) {
                Ok(scan) if Instant::now() < deadline => {
                    found |= scan.unsettled || !scan.processes.is_empty();
                }
                Ok(_) => return false,
                Err(_) => return false,
            }
        }
        if !found {
            return true;
        }
    }
}

fn wait_for_any_exit_before(processes: &[ManagedProcess], deadline: Instant) -> bool {
    loop {
        if Instant::now() >= deadline {
            return false;
        }
        let mut descriptors = processes
            .iter()
            .map(|process| PollFd::new(process.pidfd.as_fd(), PollFlags::POLLIN))
            .collect::<Vec<_>>();
        let remaining = deadline.saturating_duration_since(Instant::now());
        let timeout = PollTimeout::try_from(remaining).unwrap_or(PollTimeout::MAX);
        match poll(&mut descriptors, timeout) {
            Ok(0) => {}
            Ok(_) => {
                return Instant::now() < deadline
                    && descriptors.iter().any(|descriptor| {
                        descriptor.revents().is_some_and(|events| {
                            events.intersects(
                                PollFlags::POLLIN | PollFlags::POLLHUP | PollFlags::POLLERR,
                            )
                        })
                    });
            }
            Err(Errno::EINTR) => {}
            Err(_) => return false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Child, Command, Stdio};

    #[test]
    fn constructor_never_arms_cleanup_for_unowned_colliding_paths() {
        let root = test_directory("collision");
        fs::create_dir(&root).unwrap();

        let cohort_collision = root.join("existing-cohort");
        let unused_base = root.join("unused-base");
        fs::create_dir(&cohort_collision).unwrap();
        fs::write(cohort_collision.join("sentinel"), b"owned elsewhere").unwrap();
        let result = reserve_fixture_directories(&cohort_collision, &unused_base);
        assert!(result.is_err());
        assert_eq!(
            fs::read(cohort_collision.join("sentinel")).unwrap(),
            b"owned elsewhere"
        );
        assert!(!unused_base.exists());

        let owned_cohort = root.join("owned-cohort");
        let base_collision = root.join("existing-base");
        fs::create_dir(&base_collision).unwrap();
        fs::write(base_collision.join("sentinel"), b"owned elsewhere").unwrap();
        let result = reserve_fixture_directories(&owned_cohort, &base_collision);
        assert!(result.is_err());
        assert!(!owned_cohort.exists());
        assert_eq!(
            fs::read(base_collision.join("sentinel")).unwrap(),
            b"owned elsewhere"
        );

        let private_cohort = root.join("private-cohort");
        let private_base = root.join("private-base");
        reserve_fixture_directories(&private_cohort, &private_base).unwrap();
        for directory in [&private_cohort, &private_base] {
            assert_eq!(fs::metadata(directory).unwrap().mode() & 0o777, 0o700);
        }

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn alternate_runtime_registration_keeps_its_exact_server_visible() {
        let root = test_directory("alternate-runtime");
        let fixture = Fixture {
            cohort: root.join("cohort"),
            base: root.join("own-base"),
            cleanup_armed: false,
            authorized_runtimes: Mutex::new(vec![runtime_for_base(&root.join("own-base"))]),
        };
        let alternate_base = root.join("other-base");
        fixture.register_runtime_base(&alternate_base);
        let alternate_runtime = runtime_for_base(&alternate_base);
        let executable = fixture.cohort.join("orchbee-server");
        let observed = encode_command(&[
            executable.as_os_str(),
            OsStr::new("--runtime-directory"),
            alternate_runtime.as_os_str(),
        ]);
        let commands = fixture.expected_commands(ManagedRole::Server);
        assert!(
            commands
                .iter()
                .any(|expected| command_matches(&observed, expected))
        );

        let unregistered = encode_command(&[
            executable.as_os_str(),
            OsStr::new("--runtime-directory"),
            root.join("unregistered").as_os_str(),
        ]);
        assert!(
            commands
                .iter()
                .all(|expected| !command_matches(&unregistered, expected))
        );
    }

    #[test]
    fn pidfd_requires_exact_executable_and_command_before_signalling() {
        let root = test_directory("identity");
        fs::create_dir(&root).unwrap();
        let executable = copy_sleep(&root, "managed");
        let mut child = spawn_sleep(&executable);
        let raw_pid = i32::try_from(child.id()).unwrap();
        let exact = sleep_command(&executable);
        let mut wrong = exact.clone();
        wrong[1] = OsString::from("31");

        let evidence = (|| -> io::Result<_> {
            let rejected = matches!(
                open_verified_process(raw_pid, &executable, std::slice::from_ref(&wrong))?,
                ProcessVerification::Unsettled
            );
            let remained_alive = child.try_wait()?.is_none();
            let proc = PathBuf::from(format!("/proc/{raw_pid}"));
            assert_eq!(fs::read_link(proc.join("exe"))?, executable);
            assert!(command_matches(&fs::read(proc.join("cmdline"))?, &exact));
            let ProcessVerification::Verified(process) =
                open_verified_process(raw_pid, &executable, std::slice::from_ref(&exact))?
            else {
                return Err(io::Error::other("exact process identity was not admitted"));
            };
            let stable_pid = process.pid() == Pid::from_raw(raw_pid);
            let mut process = Some(process);
            let mut scan = || {
                Ok(ManagedProcessScan {
                    processes: process.take().into_iter().collect(),
                    unsettled: false,
                })
            };
            let expired_refused = !stop_until_quiescent_before(
                &mut scan,
                Signal::TERM,
                Instant::now() - Duration::from_secs(1),
            );
            let remained_alive_after_expiry = child.try_wait()?.is_none();
            let process = process
                .take()
                .ok_or_else(|| io::Error::other("expired cleanup consumed its scan"))?;
            process.terminate()?;
            let exited = wait_for_any_exit_before(
                std::slice::from_ref(&process),
                Instant::now() + Duration::from_secs(2),
            );
            Ok((
                rejected,
                remained_alive,
                stable_pid,
                expired_refused,
                remained_alive_after_expiry,
                exited,
            ))
        })();
        reap(&mut child);
        fs::remove_dir_all(root).unwrap();

        let (
            rejected,
            remained_alive,
            stable_pid,
            expired_refused,
            remained_alive_after_expiry,
            exited,
        ) = evidence.unwrap();
        assert!(rejected);
        assert!(remained_alive);
        assert!(stable_pid);
        assert!(expired_refused);
        assert!(remained_alive_after_expiry);
        assert!(exited);
    }

    #[test]
    fn bounded_cleanup_stops_each_late_child_after_its_parent_is_gone() {
        let root = test_directory("rescan");
        fs::create_dir(&root).unwrap();
        let server_executable = copy_sleep(&root, "server");
        let worker_executable = copy_sleep(&root, "worker");
        let host_executable = copy_sleep(&root, "host");
        let server_command = sleep_command(&server_executable);
        let worker_command = sleep_command(&worker_executable);
        let host_command = sleep_command(&host_executable);
        let mut server = spawn_sleep(&server_executable);
        let server_pid = i32::try_from(server.id()).unwrap();
        let mut worker: Option<Child> = None;
        let mut host: Option<Child> = None;
        let mut scan_counts = [0_u8; 3];
        let mut scan = |role| -> io::Result<ManagedProcessScan> {
            let scan = match role {
                ManagedRole::Server => {
                    scan_counts[0] += 1;
                    let scan = test_process_scan(server_pid, &server_executable, &server_command)?;
                    if scan.processes.is_empty() && !scan.unsettled && worker.is_none() {
                        worker = Some(spawn_sleep(&worker_executable));
                    }
                    scan
                }
                ManagedRole::Worker => {
                    scan_counts[1] += 1;
                    let scan = if let Some(child) = &worker {
                        test_process_scan(
                            i32::try_from(child.id()).unwrap(),
                            &worker_executable,
                            &worker_command,
                        )?
                    } else {
                        ManagedProcessScan::default()
                    };
                    if worker.is_some()
                        && scan.processes.is_empty()
                        && !scan.unsettled
                        && host.is_none()
                    {
                        host = Some(spawn_sleep(&host_executable));
                    }
                    scan
                }
                ManagedRole::TerminalHost => {
                    scan_counts[2] += 1;
                    if let Some(child) = &host {
                        test_process_scan(
                            i32::try_from(child.id()).unwrap(),
                            &host_executable,
                            &host_command,
                        )?
                    } else {
                        ManagedProcessScan::default()
                    }
                }
            };
            Ok(scan)
        };
        let stopped = stop_roles_before(&mut scan, Instant::now() + Duration::from_secs(2));
        reap(&mut server);
        let worker_spawned = worker.is_some();
        let host_spawned = host.is_some();
        if let Some(worker) = &mut worker {
            reap(worker);
        }
        if let Some(host) = &mut host {
            reap(host);
        }
        fs::remove_dir_all(root).unwrap();

        assert!(stopped);
        assert!(worker_spawned);
        assert!(host_spawned);
        assert!(scan_counts.into_iter().all(|count| count >= 2));
    }

    fn test_directory(label: &str) -> PathBuf {
        PathBuf::from(format!(
            "/tmp/orchbee-native-cohort-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn copy_sleep(directory: &Path, name: &str) -> PathBuf {
        let executable = directory.join(name);
        fs::copy("/bin/sleep", &executable).unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        executable
    }

    fn sleep_command(executable: &Path) -> Vec<OsString> {
        vec![executable.as_os_str().to_owned(), OsString::from("30")]
    }

    fn test_process_scan(
        raw_pid: i32,
        executable: &Path,
        command: &[OsString],
    ) -> io::Result<ManagedProcessScan> {
        let mut scan = ManagedProcessScan::default();
        match open_verified_process(raw_pid, executable, &[command.to_vec()])? {
            ProcessVerification::Verified(process) => scan.processes.push(process),
            ProcessVerification::Gone => {}
            ProcessVerification::Unsettled => scan.unsettled = true,
        }
        Ok(scan)
    }

    fn encode_command(arguments: &[&OsStr]) -> Vec<u8> {
        let mut command = Vec::new();
        for argument in arguments {
            command.extend_from_slice(argument.as_bytes());
            command.push(0);
        }
        command
    }

    fn spawn_sleep(executable: &Path) -> Child {
        let mut child = Command::new(executable)
            .arg("30")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let proc_executable = PathBuf::from(format!("/proc/{}/exe", child.id()));
        let proc_command = PathBuf::from(format!("/proc/{}/cmdline", child.id()));
        let expected_command = sleep_command(executable);
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            if fs::read_link(&proc_executable).is_ok_and(|observed| observed == executable)
                && fs::read(&proc_command)
                    .is_ok_and(|observed| command_matches(&observed, &expected_command))
            {
                return child;
            }
            assert!(child.try_wait().unwrap().is_none(), "sleep helper exited");
            assert!(Instant::now() < deadline, "sleep helper did not exec");
            std::thread::yield_now();
        }
    }

    fn reap(child: &mut Child) {
        if child.try_wait().unwrap().is_none() {
            child.kill().unwrap();
        }
        child.wait().unwrap();
    }
}
