use super::*;
use crate::{
    WindowsNamedSiteCatalog as Catalog, WindowsSiteChild, WindowsSiteName,
    durable_sessions_windows::Directory,
    process_registration::{Creator, ProcessIdentity, ProcessRole},
};
use std::{ffi::OsStr, io};

struct Child {
    child: Option<crate::RetainedChild>,
    base: Option<Fixture>,
    local: Option<Fixture>,
}
impl Drop for Child {
    fn drop(&mut self) {
        if let Some(child) = &mut self.child
            && let Err(error) = child.terminate_before(Instant::now() + Duration::from_secs(5))
        {
            eprintln!(
                "Registered child cleanup unknown; retaining base {} and local {}: {error}",
                self.base.as_ref().unwrap().0.display(),
                self.local.as_ref().unwrap().0.display()
            );
            // Their exact process and catalog identities remain available for diagnosis.
            std::mem::forget(self.base.take());
            std::mem::forget(self.local.take());
        }
        self.child.take();
    }
}

#[test]
fn server_body_observes_its_durable_identity_before_execution_and_replay_cannot_spawn() {
    // Declared before all consumers so their pins close before this owner's cleanup.
    let mut child = Child {
        child: None,
        base: Some(Fixture::new()),
        local: Some(Fixture::new()),
    };
    let base = child.base.as_ref().unwrap().0.clone();
    let local = child.local.as_ref().unwrap().0.clone();
    let name = WindowsSiteName::parse("registered-body").unwrap();
    let scope = name.scope_for_user(&orchbee_local_ipc::current_user_sid().unwrap());
    let catalog = Catalog::try_lock(&base, &local).unwrap();
    let site = Rendezvous::prepare_mode_checked(&base, SiteScope::parse(&scope).unwrap()).unwrap();
    let LockAttempt::Acquired(activation) = site.try_acquire_activation_lock().unwrap() else {
        panic!("activation");
    };
    site.register_session_name(&activation, &name).unwrap();
    catalog.begin_activation(&site, [1; 16]).unwrap();
    site.commit_activation().unwrap();
    let marker = base.join("executed.json");
    let executable = std::env::current_exe().unwrap();
    let args = [
        OsStr::new("--exact"),
        OsStr::new("tests::child_registration::child_fixture"),
        OsStr::new("--ignored"),
        OsStr::new("--skip"),
        local.as_os_str(),
        OsStr::new("--skip"),
        site.runtime_directory().as_os_str(),
        OsStr::new("--skip"),
        marker.as_os_str(),
    ];
    child.child = Some(
        catalog
            .spawn_server(
                &site,
                &activation,
                &executable,
                &args,
                Instant::now() + Duration::from_secs(5),
            )
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        if let Some(status) = child.child.as_mut().unwrap().try_wait().unwrap() {
            break status;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    };
    let observed: ProcessIdentity = serde_json::from_slice(&fs::read(&marker).unwrap()).unwrap();
    let directory = Directory::open(&local, &base.canonicalize().unwrap())
        .unwrap()
        .unwrap();
    let record = directory.read(&scope).unwrap().unwrap();
    assert_eq!(
        record
            .processes
            .registration(ProcessRole::Server)
            .identity(),
        Some(observed)
    );
    assert_eq!(status.code(), Some(259));
    assert!(
        catalog
            .spawn_server(
                &site,
                &activation,
                &executable,
                &args,
                Instant::now() + Duration::from_secs(5)
            )
            .is_err()
    );
    assert_eq!(directory.read(&scope).unwrap().unwrap(), record);
}

#[test]
fn wrong_direct_parent_and_failed_initial_publication_have_no_process_effect() {
    let base = Fixture::new();
    let local = Fixture::new();
    let name = WindowsSiteName::parse("no-child").unwrap();
    let scope = name.scope_for_user(&orchbee_local_ipc::current_user_sid().unwrap());
    let catalog = Catalog::try_lock(&base.0, &local.0).unwrap();
    let site = base.open(&scope);
    let LockAttempt::Acquired(activation) = site.try_acquire_activation_lock().unwrap() else {
        panic!("activation");
    };
    site.register_session_name(&activation, &name).unwrap();
    catalog.begin_activation(&site, [1; 16]).unwrap();
    site.commit_activation().unwrap();
    let directory = Directory::open(&local.0, &base.0.canonicalize().unwrap())
        .unwrap()
        .unwrap();
    let original = directory.read(&scope).unwrap().unwrap();
    let executable = std::env::current_exe().unwrap();
    let record_path = local
        .0
        .join("Orchbee/session-catalog")
        .join(Rendezvous::host_endpoint_id(
            &base.0.canonicalize().unwrap(),
        ))
        .join(format!("{scope}.json"));
    let pin = permissions::open_file(&record_path, false, false).unwrap();
    assert!(
        catalog
            .spawn_server(
                &site,
                &activation,
                &executable,
                &[],
                Instant::now() + Duration::from_secs(5)
            )
            .is_err()
    );
    assert_eq!(directory.read(&scope).unwrap().unwrap(), original);
    drop(pin);
    let mut record = original;
    let processes = &mut record.processes;
    processes
        .reserve(ProcessRole::Server, Creator::Activator)
        .unwrap();
    processes
        .identify(ProcessRole::Server, ProcessIdentity::new(8, 1).unwrap())
        .unwrap();
    directory.publish(&scope, &record).unwrap();
    let error = crate::spawn_windows_site_child(
        &local.0,
        site.runtime_directory(),
        WindowsSiteChild::Worker,
        &executable,
        &[],
        Instant::now() + Duration::from_secs(5),
    );
    assert_eq!(error.err().unwrap().kind(), io::ErrorKind::InvalidData);
    assert_eq!(directory.read(&scope).unwrap().unwrap(), record);
}

#[test]
#[ignore = "suspended native child validates its own registration before executing"]
fn child_fixture() {
    let arguments: Vec<_> = std::env::args_os().collect();
    let local = PathBuf::from(&arguments[arguments.len() - 5]);
    let runtime = PathBuf::from(&arguments[arguments.len() - 3]);
    let marker = PathBuf::from(&arguments[arguments.len() - 1]);
    let directory = Directory::open(&local, runtime.parent().unwrap())
        .unwrap()
        .unwrap();
    let scope = runtime.file_name().unwrap().to_str().unwrap();
    let record = directory.read(scope).unwrap().unwrap();
    let identity = crate::process_observation_windows::current_identity().unwrap();
    assert_eq!(
        record
            .processes
            .registration(ProcessRole::Server)
            .identity(),
        Some(identity)
    );
    fs::write(marker, serde_json::to_vec(&identity).unwrap()).unwrap();
    std::process::exit(259);
}
