use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Mutex},
};

use crate::client_session_registry::{
    ClientSessionAdmission, ClientSessionRegistry, MAXIMUM_ACTIVE_CLIENT_SESSIONS,
};

use super::{ClientSessionRegistration, ConnectionSlots, MAXIMUM_CLIENT_CONNECTIONS};

mod agent;
pub(crate) mod agent_harness;
mod workbench;

#[test]
fn bounds_connection_threads_and_reuses_a_released_slot() {
    let slots = Arc::new(ConnectionSlots::new());
    let mut reservations = Vec::new();
    for _connection in 0..MAXIMUM_CLIENT_CONNECTIONS {
        let Some(reservation) = slots.acquire() else {
            panic!("connection slot was unavailable within the bound");
        };
        reservations.push(reservation);
    }

    assert!(slots.acquire().is_none());
    let released = reservations.pop();
    assert!(released.is_some());
    drop(released);
    assert!(slots.acquire().is_some());
}

#[test]
fn unwind_releases_the_exact_session_and_handler_capacity() {
    let connection_slots = Arc::new(ConnectionSlots::new());
    let mut retained_connection_slots = Vec::new();
    for _connection in 1..MAXIMUM_CLIENT_CONNECTIONS {
        let Some(connection_slot) = connection_slots.acquire() else {
            panic!("connection slot was unavailable below the bound");
        };
        retained_connection_slots.push(connection_slot);
    }

    let registry = Arc::new(Mutex::new(ClientSessionRegistry::new()));
    let mut retained_session_generations = Vec::new();
    {
        let mut registry = registry
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        for _session in 1..MAXIMUM_ACTIVE_CLIENT_SESSIONS {
            let ClientSessionAdmission::Accepted { generation } = registry.admit() else {
                panic!("Client Session was unavailable below the bound");
            };
            retained_session_generations.push(generation);
        }
    }

    let mut unwound_generation = None;
    let unwind = catch_unwind(AssertUnwindSafe(|| {
        let _connection_slot = connection_slots
            .acquire()
            .unwrap_or_else(|| panic!("last connection slot was unavailable"));
        let generation = {
            let mut registry = registry
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let ClientSessionAdmission::Accepted { generation } = registry.admit() else {
                panic!("last Client Session was not admitted");
            };
            generation
        };
        unwound_generation = Some(generation);
        let _session_registration =
            ClientSessionRegistration::new(Arc::clone(&registry), generation);
        panic!("simulated post-admission panic");
    }));

    assert!(unwind.is_err());
    let recovered_connection_slot = connection_slots
        .acquire()
        .unwrap_or_else(|| panic!("unwound connection slot was not released"));
    assert!(connection_slots.acquire().is_none());
    drop(recovered_connection_slot);

    let unwound_generation =
        unwound_generation.unwrap_or_else(|| panic!("unwind did not retain its generation"));
    let mut registry = registry
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let ClientSessionAdmission::Accepted {
        generation: replacement_generation,
    } = registry.admit()
    else {
        panic!("unwound Client Session capacity was not released");
    };
    assert!(!registry.disconnect(unwound_generation));
    for retained_generation in retained_session_generations {
        assert!(registry.disconnect(retained_generation));
    }
    assert!(registry.disconnect(replacement_generation));
}

#[cfg(target_os = "linux")]
#[test]
fn managed_socket_publication_has_final_permissions_and_never_replaces_an_entry()
-> std::io::Result<()> {
    use std::{
        fs,
        os::unix::{fs::PermissionsExt, net::UnixStream},
    };
    let directory = std::env::temp_dir().join(format!("ob-publish-{}", std::process::id()));
    fs::create_dir(&directory)?;
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))?;
    use orchbee_local_site_runtime::CLIENT_PENDING_SOCKET_NAME;
    let endpoint = directory.join("client.sock");
    let listener = super::bind_managed(&endpoint)?;
    assert_eq!(
        fs::metadata(&endpoint)?.permissions().mode() & 0o7777,
        0o600
    );
    assert!(!directory.join(CLIENT_PENDING_SOCKET_NAME).exists());
    let client = UnixStream::connect(&endpoint)?;
    drop(client);
    drop(listener);
    assert_eq!(
        super::bind_managed(&endpoint).unwrap_err().kind(),
        std::io::ErrorKind::AlreadyExists
    );
    assert!(endpoint.exists());
    assert!(directory.join(CLIENT_PENDING_SOCKET_NAME).exists());
    fs::remove_dir_all(directory)?;
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn managed_socket_path_has_final_permissions_and_never_replaces_an_entry() -> std::io::Result<()> {
    use std::{
        fs,
        os::unix::{fs::PermissionsExt, net::UnixStream},
    };
    let directory = std::env::temp_dir().join(format!("ob-managed-{}", std::process::id()));
    fs::create_dir(&directory)?;
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))?;
    let endpoint = directory.join("client.sock");
    let listener = super::bind_managed(&endpoint)?;
    assert_eq!(
        fs::symlink_metadata(&endpoint)?.permissions().mode() & 0o7777,
        0o600
    );
    let client = UnixStream::connect(&endpoint)?;
    drop(client);
    drop(listener);
    assert_eq!(
        super::bind_managed(&endpoint).unwrap_err().kind(),
        std::io::ErrorKind::AddrInUse
    );
    assert_eq!(
        fs::symlink_metadata(&endpoint)?.permissions().mode() & 0o7777,
        0o600
    );
    fs::remove_dir_all(directory)?;
    Ok(())
}
