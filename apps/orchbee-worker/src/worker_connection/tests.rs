use super::*;
use orchbee_local_ipc::LocalListener;
use std::{thread, time::Instant};

#[test]
fn separate_windows_connections_retain_their_own_admitted_worker_identity()
-> Result<(), Box<dyn Error>> {
    let mut identities = Vec::new();
    for index in 0..2 {
        let endpoint = std::path::PathBuf::from(format!(
            r"\\.\pipe\orchbee-worker-context-test-{}-{index}",
            std::process::id()
        ));
        let listener = LocalListener::bind(&endpoint)?;
        listener.set_nonblocking(true)?;
        let peer = thread::spawn(move || -> io::Result<(WorkerId, Option<String>)> {
            let deadline = Instant::now() + Duration::from_secs(3);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, ())) => break stream,
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        if Instant::now() >= deadline {
                            return Err(io::ErrorKind::TimedOut.into());
                        }
                        thread::yield_now();
                    }
                    Err(error) => return Err(error),
                }
            };
            stream.set_read_timeout(Some(HANDSHAKE_TIMEOUT))?;
            stream.set_write_timeout(Some(HANDSHAKE_TIMEOUT))?;
            let LocalWorkerMessage::WorkerHello(hello) =
                read_message(&mut stream).map_err(io::Error::other)?
            else {
                return Err(io::Error::other("expected WorkerHello"));
            };
            assert_eq!(hello.cohort(), "caller-context-test");
            write_message(
                &mut stream,
                &LocalWorkerMessage::WorkerAccepted {
                    generation: NonZeroU64::new(7).expect("generation"),
                },
            )
            .map_err(io::Error::other)?;
            Ok((
                hello.worker_id(),
                hello.system_shell().map(|name| name.as_str().to_owned()),
            ))
        });
        // The hello names the same pinned resolution every default launch of the generation uses.
        let system_shell = SystemShell::pinned(orchbee_local_runtime::DefaultInteractiveLaunch {
            executable: r"C:\Program Files\PowerShell\7\pwsh.exe".into(),
            arguments: Vec::new(),
            environment: crate::default_interactive_launch::default_launch_environment(),
        });
        let connection = connect_to_server(&endpoint, "caller-context-test", &system_shell);
        let (admitted, reported_shell) = peer.join().expect("peer thread")?;
        let connection = connection?;
        assert_eq!(reported_shell.as_deref(), Some("pwsh.exe"));
        assert_eq!(connection.worker_id, admitted);
        assert_eq!(connection.generation().get(), 7);
        assert_ne!(admitted.as_bytes(), [0; 16]);
        assert_ne!(admitted.as_bytes(), *b"orchbee-local-v1");
        identities.push(admitted);
    }
    assert_ne!(identities[0], identities[1]);
    Ok(())
}
