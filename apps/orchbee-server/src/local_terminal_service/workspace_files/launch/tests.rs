//! The endpoint admits only the connection that presents its ticket: any process of this user can
//! reach the socket folder, so a wrong ticket must be refused without ending the wait, and the
//! endpoint must be gone once the service is admitted. No Worker is needed to show it, so the
//! service is played by a thread here.
use super::Endpoint;
use orchbee_capability_node_protocol::WorkspaceFilesTicket;
use orchbee_local_ipc::LocalStream;
use orchbee_local_runtime::ModeCheckedLocalSessionRendezvous;
use std::io::Write;
use std::time::{Duration, Instant};

fn runtime() -> std::path::PathBuf {
    let runtime = std::env::temp_dir().join(format!(
        "orchbee-workspace-files-launch-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&runtime).unwrap();
    #[cfg(unix)]
    {
        // The endpoint binds in this environment's socket folder, which activation creates.
        use std::os::unix::fs::DirBuilderExt;
        let _created = std::fs::DirBuilder::new()
            .mode(0o700)
            .create(ModeCheckedLocalSessionRendezvous::socket_root_path());
    }
    runtime
}

#[test]
fn only_the_ticket_holder_is_admitted_and_the_endpoint_is_removed() {
    let runtime = runtime();
    let ticket = WorkspaceFilesTicket::from_bytes([0x3c; 16]).unwrap();
    let path =
        ModeCheckedLocalSessionRendezvous::workspace_files_socket_path(&runtime, ticket.as_bytes());
    let endpoint = Endpoint::bind(&runtime, ticket).unwrap();
    let service = std::thread::spawn(move || {
        let mut stranger = LocalStream::connect(&path).unwrap();
        stranger
            .write_all(&orchbee_workspace_files::hello([0x3d; 16]))
            .unwrap();
        let mut holder = LocalStream::connect(&path).unwrap();
        holder
            .write_all(&orchbee_workspace_files::hello([0x3c; 16]))
            .unwrap();
        holder.write_all(b"after the hello\n").unwrap();
        (stranger, holder, path)
    });
    let connection = endpoint
        .admit(ticket, Instant::now() + Duration::from_secs(5))
        .unwrap();
    let (_stranger, _holder, path) = service.join().unwrap();
    let mut line = String::new();
    std::io::BufRead::read_line(
        &mut std::io::BufReader::new(connection.reader().unwrap()),
        &mut line,
    )
    .unwrap();
    assert_eq!(line, "after the hello\n");
    drop(endpoint);
    #[cfg(unix)]
    assert!(!path.exists());
    #[cfg(not(unix))]
    let _path = path;
    let _removed = std::fs::remove_dir_all(&runtime);
}
