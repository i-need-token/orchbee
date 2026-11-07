use super::*;
use crate::{
    test_support::WorkbenchFixture, worker_obcn_carrier::create_worker_carrier,
    worker_registry::WorkerAdmission,
};
use orchbee_capability_node_protocol::{CapabilityNodeMessage, LaunchPreferences, read_message};
use orchbee_local_ipc::LocalStream as UnixStream;
use orchbee_local_runtime::UserPreferencesScope;
use orchbee_local_worker_protocol::WorkerId as LocalWorkerId;
use orchbee_workbench_protocol::{PresentationBinding, SessionId, TerminalId};
use std::{error::Error, path::PathBuf, thread, time::Duration};

struct PrivateRoot(PathBuf);
impl Drop for PrivateRoot {
    fn drop(&mut self) {
        let _removed = std::fs::remove_dir_all(&self.0);
    }
}

/// Windows restoration launches before the process observer starts. Restoration therefore
/// freezes the preference scope itself; an unfrozen store would capture `Unavailable`, which the
/// Worker refuses for every restored terminal.
#[test]
fn restored_terminals_capture_the_session_preferences() -> Result<(), Box<dyn Error + Send + Sync>>
{
    let registry = Arc::new(Mutex::new(WorkerRegistry::new()));
    let local_worker = LocalWorkerId::from_bytes([9; 16]);
    let WorkerAdmission::Accepted { generation } = registry
        .lock()
        .map_err(|_poison| "Worker registry poisoned")?
        .admit(local_worker, None)
    else {
        return Err("test Worker was not admitted".into());
    };
    let owner = WorkerRegistry::register_carrier(
        &registry,
        generation,
        create_worker_carrier(local_worker, generation),
    )
    .ok_or("test Worker carrier was not registered")?;
    let (server, mut worker_peer) = UnixStream::pair()?;
    worker_peer.set_read_timeout(Some(Duration::from_secs(5)))?;
    let owner_thread = thread::spawn(move || owner.run(server));

    // One started Tab of this Worker, captured the way a Session checkpoint is.
    let worker = WorkerId::parse(&"09".repeat(16))?;
    let mut authority = WorkbenchAuthority::workspace_fixture(SessionId::parse(&"11".repeat(16))?)?;
    authority
        .append_fixture_tab(PresentationBinding {
            terminal_id: TerminalId::parse(&"22".repeat(16))?,
            worker_id: worker.clone(),
        })
        .map_err(|_result| "fixture Tab was not created")?;
    let root = PrivateRoot(std::env::temp_dir().join(format!(
        "orchbee-restore-preferences-{}",
        std::process::id()
    )));
    std::fs::create_dir(&root.0)?;
    // Restore launches only in a directory that exists, so the Workspace names this one.
    let checkpoint = String::from_utf8(authority.checkpoint(&worker)?)?.replace(
        "\"/workspace\"",
        &serde_json::to_string(&root.0.to_string_lossy())?,
    );
    let checkpoint = WorkbenchCheckpoint::parse(checkpoint.as_bytes())?;
    let scope = UserPreferencesScope::for_configuration(&root.0.join("config.toml"))
        .map_err(|_failure| "preference scope")?;
    let (service, restore) =
        LocalTerminalService::from_checkpoint(Arc::clone(&registry), Some(checkpoint), scope)?;
    let service = Arc::new(service);
    let restoring = {
        let service = Arc::clone(&service);
        thread::spawn(move || {
            service.launch_checkpoint_terminals(restore, Instant::now() + Duration::from_secs(10))
        })
    };
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(launch) = read_message(&mut worker_peer)?
    else {
        return Err("restoration did not dispatch a terminal Launch".into());
    };
    assert!(matches!(
        launch.preferences(),
        LaunchPreferences::Available(_)
    ));
    drop(worker_peer);
    restoring
        .join()
        .map_err(|_panic| "restoration panicked")??;
    let _exit = owner_thread
        .join()
        .map_err(|_panic| "Worker owner panicked")?;
    Ok(())
}
