#![cfg(unix)]

use std::{
    fs, io,
    num::NonZeroU64,
    os::unix::net::UnixListener,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread,
};

use orchbee_local_client_protocol::{
    ClientSessionConnection, LocalClientMessage, read_message, write_message,
};
use orchbee_local_peer_credentials::require_same_effective_user;

static NEXT_FIXTURE_ID: AtomicU64 = AtomicU64::new(1);

#[test]
fn connecting_adapter_authenticates_handshakes_and_observes_server_loss() -> io::Result<()> {
    let fixture = TestDirectory::new()?;
    let endpoint = fixture.path.join("client.sock");
    let listener = UnixListener::bind(&endpoint)?;
    let (release_sender, release_receiver) = mpsc::channel();
    let server = thread::spawn(move || -> Result<(), String> {
        let (mut stream, _address) = listener.accept().map_err(|error| error.to_string())?;
        require_same_effective_user(&stream).map_err(|error| error.to_string())?;
        let message = read_message(&mut stream).map_err(|error| error.to_string())?;
        let LocalClientMessage::ClientHello(hello) = message else {
            return Err("expected ClientHello".to_owned());
        };
        if hello.cohort() != "test-cohort" {
            return Err(format!("unexpected cohort: {}", hello.cohort()));
        }
        let generation = NonZeroU64::new(7).ok_or_else(|| "invalid generation".to_owned())?;
        write_message(
            &mut stream,
            &LocalClientMessage::ClientAccepted { generation },
        )
        .map_err(|error| error.to_string())?;
        release_receiver.recv().map_err(|error| error.to_string())?;
        Ok(())
    });

    let mut connection =
        ClientSessionConnection::connect(&endpoint, "test-cohort").map_err(io::Error::other)?;
    assert_eq!(connection.generation().get(), 7);
    release_sender
        .send(())
        .map_err(|error| io::Error::other(error.to_string()))?;
    server
        .join()
        .map_err(|_panic| io::Error::other("fake Server panicked"))?
        .map_err(io::Error::other)?;
    connection.wait_for_server_loss()?;
    Ok(())
}

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new() -> io::Result<Self> {
        for _attempt in 0..100 {
            let fixture_id = NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "orchbee-local-client-protocol-{}-{fixture_id}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not allocate a unique local Client protocol test directory",
        ))
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _result = fs::remove_dir_all(&self.path);
    }
}
