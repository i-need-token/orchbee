#![cfg(unix)]

use std::{
    fs,
    io::{self, Read, Write},
    num::NonZeroU64,
    os::unix::net::UnixListener,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

use orchbee_local_client_protocol::{
    ClientSessionConnection, LocalClientMessage, read_message, write_message,
};
use orchbee_local_peer_credentials::require_same_effective_user;

static NEXT_FIXTURE_ID: AtomicU64 = AtomicU64::new(1);

#[cfg(target_os = "linux")]
#[path = "connection/admission_deadline.rs"]
mod admission_deadline;

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

#[test]
fn accepted_connection_preserves_coalesced_next_phase_bytes() -> io::Result<()> {
    const SERVER_PHASE_BYTES: &[u8] = b"OBCS\0\x01next-phase";
    const CLIENT_PHASE_BYTES: &[u8] = b"client-next-phase";

    let fixture = TestDirectory::new()?;
    let endpoint = fixture.path.join("client.sock");
    let listener = UnixListener::bind(&endpoint)?;
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

        let generation = NonZeroU64::new(11).ok_or_else(|| "invalid generation".to_owned())?;
        let mut coalesced_response = Vec::new();
        write_message(
            &mut coalesced_response,
            &LocalClientMessage::ClientAccepted { generation },
        )
        .map_err(|error| error.to_string())?;
        coalesced_response.extend_from_slice(SERVER_PHASE_BYTES);
        let written = stream
            .write(&coalesced_response)
            .map_err(|error| error.to_string())?;
        if written != coalesced_response.len() {
            return Err(format!(
                "coalesced response was short-written: {written}/{} bytes",
                coalesced_response.len()
            ));
        }

        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .map_err(|error| error.to_string())?;
        let mut client_phase_bytes = [0_u8; CLIENT_PHASE_BYTES.len()];
        stream
            .read_exact(&mut client_phase_bytes)
            .map_err(|error| error.to_string())?;
        if client_phase_bytes != CLIENT_PHASE_BYTES {
            return Err("unexpected Client next-phase bytes".to_owned());
        }
        Ok(())
    });

    let mut connection =
        ClientSessionConnection::connect(&endpoint, "test-cohort").map_err(io::Error::other)?;
    assert_eq!(connection.generation().get(), 11);

    let mut server_phase_bytes = [0_u8; SERVER_PHASE_BYTES.len()];
    connection.read_exact(&mut server_phase_bytes)?;
    assert_eq!(server_phase_bytes, SERVER_PHASE_BYTES);
    connection.write_all(CLIENT_PHASE_BYTES)?;

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
