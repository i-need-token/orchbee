use super::super::*;
use crate::{
    DataRoot,
    durable_sessions::Directory,
    private_filesystem as permissions,
    process_observation::ProcessSnapshot,
    process_registration::{
        Creator, ProcessIdentity, ProcessRole, RecoveryVerdict, SessionProcesses,
    },
};
use std::{
    fs, thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

struct Fixture {
    root: std::path::PathBuf,
    child: Option<RetainedChild>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(child) = &mut self.child
            && let Err(error) = child.terminate_before(Instant::now() + Duration::from_secs(5))
        {
            eprintln!(
                "Registration fixture retained at {}: {error}",
                self.root.display()
            );
            return;
        }
        self.child.take();
        if self.root.parent() != Some(std::env::temp_dir().canonicalize().unwrap().as_path()) {
            return;
        }
        // The marker the child wrote and the record lock's Session folder.
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn suspended_child_identity_precedes_execution_and_exit_259_is_gone() {
    let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
        "orchbee-registration-test-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    permissions::protect_new(&mut permissions::open_directory(&root, true).unwrap()).unwrap();
    let mut fixture = Fixture { root, child: None };
    // A verdict is taken under the record writer lock, as cold recovery takes it.
    let directory = Directory::create(&DataRoot::at(fixture.root.clone())).unwrap();
    let writer = directory.try_writer("registration").unwrap();
    let executable = std::env::current_exe().unwrap();
    let marker = fixture.root.join("executed.json");
    let args = [
        OsStr::new("--exact"),
        OsStr::new("retained_child::windows::tests::registration::registered_fixture"),
        OsStr::new("--ignored"),
        OsStr::new("--skip"),
        marker.as_os_str(),
    ];
    fixture.child = Some(RetainedChild::spawn_suspended(&executable, &args).unwrap());
    let child = fixture.child.as_mut().unwrap();
    let identity = child.identity().unwrap();
    assert_ne!(identity.pid(), std::process::id());
    let mut processes = SessionProcesses::default();
    processes
        .reserve(ProcessRole::Server, Creator::Activator)
        .unwrap();
    processes.identify(ProcessRole::Server, identity).unwrap();
    let snapshot = ProcessSnapshot::observe(&processes);
    assert_eq!(
        snapshot.verdict(&processes, &writer),
        RecoveryVerdict::Live(ProcessRole::Server)
    );
    thread::sleep(Duration::from_millis(100));
    assert!(!marker.exists(), "suspended program must not have executed");
    child.resume().unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(10));
    };
    let observed: ProcessIdentity = serde_json::from_slice(&fs::read(marker).unwrap()).unwrap();
    assert_eq!(observed, identity);
    assert_eq!(status.code(), Some(259));
    assert_eq!(
        ProcessSnapshot::observe(&processes).verdict(&processes, &writer),
        RecoveryVerdict::Gone
    );
}

#[test]
#[ignore = "exact suspended-child fixture owned by its parent"]
fn registered_fixture() {
    let marker = std::env::args_os().last().unwrap();
    let identity = crate::process_observation::current_identity().unwrap();
    fs::write(marker, serde_json::to_vec(&identity).unwrap()).unwrap();
    std::process::exit(259);
}
