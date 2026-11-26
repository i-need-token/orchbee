//! A Workspace files service started from this executable. The Server names the endpoint from the
//! Session's runtime folder and the ticket, and the service names it the same way in another
//! process; nothing but a real process run shows that the two agree, that the service presents its
//! ticket first, serves the right bar's requests on that connection, and ends when the Server
//! closes it, so this one case is kept as that evidence.
use super::{
    TestDirectory, TestResult, accept_with_deadline, bind_listener, configure_peer_stream,
    wait_for_exit,
};
use orchbee_local_peer_credentials::require_same_effective_user;
use orchbee_local_runtime::ModeCheckedLocalSessionRendezvous;
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    net::Shutdown,
    os::unix::fs::DirBuilderExt,
    path::PathBuf,
    process::{Command, Stdio},
};

/// The endpoint's socket file, removed with the test.
struct Endpoint(PathBuf);

impl Drop for Endpoint {
    fn drop(&mut self) {
        let _removed = fs::remove_file(&self.0);
    }
}

#[test]
fn workspace_files_service_presents_its_ticket_serves_and_ends_with_its_connection() -> TestResult {
    let directory = TestDirectory::new()?;
    let runtime = directory.path().join("session");
    fs::create_dir(&runtime)?;
    fs::write(runtime.join("notes.txt"), "kept")?;
    // The endpoint binds in this environment's socket folder, which activation creates.
    let _created = fs::DirBuilder::new()
        .mode(0o700)
        .create(ModeCheckedLocalSessionRendezvous::socket_root_path());
    let mut ticket = [0x5a; 16];
    ticket[..4].copy_from_slice(&std::process::id().to_be_bytes());
    let endpoint =
        Endpoint(ModeCheckedLocalSessionRendezvous::workspace_files_socket_path(&runtime, ticket));
    let listener = bind_listener(&endpoint.0)?;
    let hex: String = ticket.iter().map(|byte| format!("{byte:02x}")).collect();
    let mut service = Command::new(env!("CARGO_BIN_EXE_orchbee-worker"))
        .arg("--workspace-files")
        .arg("operations")
        .arg("--runtime-directory")
        .arg(&runtime)
        .arg("--ticket")
        .arg(&hex)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    let mut stream = accept_with_deadline(&listener)?;
    configure_peer_stream(&stream)?;
    require_same_effective_user(&stream)?;
    assert_eq!(orchbee_workspace_files::read_hello(&mut stream)?, ticket);

    let root = runtime.to_str().ok_or("the runtime folder is not UTF-8")?;
    writeln!(
        stream,
        r#"{{"id":1,"root":"{root}","operation":{{"kind":"list-directory","path":""}}}}"#
    )?;
    let mut answer = String::new();
    BufReader::new(stream.try_clone()?).read_line(&mut answer)?;
    assert!(answer.starts_with(r#"{"id":1,"#), "{answer}");
    assert!(answer.contains("notes.txt"), "{answer}");

    stream.shutdown(Shutdown::Both)?;
    let status = wait_for_exit(&mut service)?;
    assert!(status.success(), "{status}");
    Ok(())
}
