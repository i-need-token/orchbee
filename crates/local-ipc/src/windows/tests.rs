use super::*;
use std::{
    process::{Command, Stdio},
    sync::atomic::AtomicU64,
};

static SERIAL: AtomicU64 = AtomicU64::new(1);

fn endpoint() -> std::path::PathBuf {
    format!(
        r"\\.\pipe\orchbee-ipc-test-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    )
    .into()
}

fn pair() -> (LocalStream, LocalStream) {
    let path = endpoint();
    let listener = LocalListener::bind(&path).unwrap();
    let client = LocalStream::connect(&path).unwrap();
    let (server, ()) = listener.accept().unwrap();
    (client, server)
}

#[test]
fn short_kernel_wait_limit_retains_a_real_same_user_connection() {
    let path = endpoint();
    let listener = LocalListener::bind(&path).unwrap();
    let mut client = LocalStream::connect_before_with_wait_limit(
        &path,
        Instant::now() + Duration::from_secs(5),
        Duration::from_millis(1),
    )
    .unwrap()
    .expect("available listener connects once");
    let (mut server, ()) = listener.accept().unwrap();
    server
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    client.write_all(b"authenticated").unwrap();
    let mut bytes = [0; 13];
    server.read_exact(&mut bytes).unwrap();
    assert_eq!(&bytes, b"authenticated");
}

#[test]
fn bounded_namespace_observation_never_connects_or_invents_absence() {
    let path = endpoint();
    let listener = LocalListener::bind(&path).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    assert!(endpoint_exists(&path).unwrap());
    assert_eq!(endpoint_exists_before(&path, deadline).unwrap(), Some(true));
    let different_case = path.to_str().unwrap().replace("orchbee", "ORCHBEE");
    assert!(endpoint_exists(Path::new(&different_case)).unwrap());
    for suffix in [".", "-other"] {
        let different_name = format!("{}{suffix}", path.display());
        assert!(!endpoint_exists(Path::new(&different_name)).unwrap());
    }
    assert_eq!(listener.readiness().unwrap(), Readiness::Pending);
    let client = LocalStream::connect(&path).unwrap();
    assert!(endpoint_exists(&path).unwrap());
    let (server, ()) = listener.accept().unwrap();
    drop((client, server, listener));
    assert!(!endpoint_exists(&path).unwrap());
    assert_eq!(
        endpoint_exists_before(&path, deadline).unwrap(),
        Some(false)
    );
    assert_eq!(endpoint_exists_before(&path, Instant::now()).unwrap(), None);
    for invalid in [
        r"\\.\pipe\*",
        r"\\.\pipe\orchbee?",
        r"\\remote\pipe\orchbee",
    ] {
        assert_eq!(
            endpoint_exists(Path::new(invalid)).unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
        assert_eq!(
            endpoint_exists_before(Path::new(invalid), Instant::now())
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidInput
        );
    }
}

#[test]
fn retained_listeners_remain_observable_during_unrelated_pipe_churn() {
    let stop = AtomicBool::new(false);
    let result = thread::scope(|scope| {
        let workers: Vec<_> = (0..4)
            .map(|_| {
                scope.spawn(|| -> io::Result<()> {
                    for _ in 0..1024 {
                        if stop.load(Ordering::Relaxed) {
                            break;
                        }
                        let listeners: io::Result<Vec<_>> =
                            (0..32).map(|_| LocalListener::bind(endpoint())).collect();
                        drop(listeners?);
                    }
                    Ok(())
                })
            })
            .collect();
        let result = (|| -> io::Result<()> {
            for _ in 0..512 {
                let path = endpoint();
                let listener = LocalListener::bind(&path)?;
                if !endpoint_exists(&path)?
                    || endpoint_exists_before(&path, Instant::now() + Duration::from_secs(5))?
                        != Some(true)
                {
                    return Err(io::Error::other(format!(
                        "retained listener was absent from namespace observation: {path:?}"
                    )));
                }
                if listener.readiness()? != Readiness::Pending {
                    return Err(io::Error::other(
                        "namespace observation opened the listener",
                    ));
                }
            }
            Ok(())
        })();
        stop.store(true, Ordering::Relaxed);
        for worker in workers {
            worker.join().unwrap()?;
        }
        result
    });
    result.unwrap();
}

#[test]
fn records_larger_than_pipe_capacity_progress_without_losing_bytes() {
    let (mut client, mut server) = pair();
    client
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    server
        .set_write_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let bytes: Vec<u8> = (0..=255).cycle().take(256 * 1024).collect();
    let expected = bytes.clone();
    let reader = thread::spawn(move || {
        let mut received = vec![0; expected.len()];
        client.read_exact(&mut received).unwrap();
        assert_eq!(received, expected);
    });
    server.write_all(&bytes).unwrap();
    reader.join().unwrap();
}

#[test]
fn probe_preserves_exact_order_and_eof() {
    let (mut client, mut server) = pair();
    assert_eq!(server.readiness().unwrap(), Readiness::Pending);
    let bytes: Vec<u8> = (0..=255).cycle().take(8192).collect();
    client.write_all(&bytes).unwrap();
    assert_eq!(server.readiness().unwrap(), Readiness::Readable);
    assert_eq!(server.readiness().unwrap(), Readiness::Readable);
    let mut received = vec![0; bytes.len()];
    server.read_exact(&mut received).unwrap();
    assert_eq!(received, bytes);
    drop(client);
    assert_eq!(server.readiness().unwrap(), Readiness::Closed);
    assert_eq!(server.read(&mut [0; 1]).unwrap(), 0);
}

#[test]
fn read_timeout_and_clone_shutdown_are_bounded() {
    let (client, mut server) = pair();
    server
        .set_read_timeout(Some(Duration::from_millis(25)))
        .unwrap();
    let start = Instant::now();
    assert_eq!(
        server.read(&mut [0]).unwrap_err().kind(),
        io::ErrorKind::TimedOut
    );
    assert!(start.elapsed() < Duration::from_secs(2));
    server.set_read_timeout(None).unwrap();
    let closer = server.try_clone().unwrap();
    let task = thread::spawn(move || server.read(&mut [0]));
    closer.shutdown(Shutdown::Both).unwrap();
    assert_eq!(task.join().unwrap().unwrap(), 0);
    assert_eq!(client.readiness().unwrap(), Readiness::Closed);
}

#[test]
fn full_buffer_write_does_not_block_shutdown() {
    let (mut client, _server) = pair();
    client.set_nonblocking(true).unwrap();
    let data = [7; 8192];
    let mut filled = false;
    for _ in 0..1024 {
        match client.write(&data) {
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                filled = true;
                break;
            }
            Err(error) => panic!("unexpected write failure: {error}"),
        }
    }
    assert!(filled, "test never reached the real pipe's capacity");
    client.set_nonblocking(false).unwrap();
    client
        .set_write_timeout(Some(Duration::from_millis(25)))
        .unwrap();
    let start = Instant::now();
    assert_eq!(
        client.write(&data).unwrap_err().kind(),
        io::ErrorKind::TimedOut
    );
    client.shutdown(Shutdown::Both).unwrap();
    assert!(start.elapsed() < Duration::from_secs(2));
}

#[test]
fn a_peer_byte_wakes_a_waiting_poll_and_stays_readable() {
    let (mut client, mut server) = pair();
    let writer = thread::spawn(move || {
        thread::sleep(Duration::from_millis(50));
        client.write_all(b"x").unwrap();
        client
    });
    let start = Instant::now();
    let mut sources = [crate::PollFd::new(
        crate::LocalIo::Stream(&server),
        crate::PollFlags::POLLIN,
    )];
    assert_eq!(
        crate::poll(
            &mut sources,
            crate::PollTimeout::try_from(30_000_u128).unwrap()
        )
        .unwrap(),
        1
    );
    assert!(start.elapsed() < Duration::from_secs(20));
    let mut byte = [0];
    server.read_exact(&mut byte).unwrap();
    assert_eq!(&byte, b"x");
    assert_eq!(server.readiness().unwrap(), Readiness::Pending);
    writer.join().unwrap();
}

#[test]
fn peer_close_and_clone_shutdown_each_wake_a_waiting_poll() {
    let (client, server) = pair();
    let closer = thread::spawn(move || {
        thread::sleep(Duration::from_millis(50));
        drop(client);
    });
    let timeout = crate::PollTimeout::try_from(30_000_u128).unwrap();
    let mut sources = [crate::PollFd::new(
        crate::LocalIo::Stream(&server),
        crate::PollFlags::POLLIN,
    )];
    assert_eq!(crate::poll(&mut sources, timeout).unwrap(), 1);
    assert!(
        sources[0]
            .revents()
            .unwrap()
            .contains(crate::PollFlags::POLLHUP)
    );
    closer.join().unwrap();

    let (_client, server) = pair();
    let clone = server.try_clone().unwrap();
    let closer = thread::spawn(move || {
        thread::sleep(Duration::from_millis(50));
        clone.shutdown(Shutdown::Both).unwrap();
    });
    let start = Instant::now();
    let mut sources = [crate::PollFd::new(
        crate::LocalIo::Stream(&server),
        crate::PollFlags::POLLIN,
    )];
    assert_eq!(crate::poll(&mut sources, timeout).unwrap(), 1);
    assert!(start.elapsed() < Duration::from_secs(20));
    closer.join().unwrap();
}

#[test]
fn closing_a_watched_stream_still_delivers_its_final_bytes_to_the_peer() {
    let (client, mut server) = pair();
    let mut sources = [crate::PollFd::new(
        crate::LocalIo::Stream(&client),
        crate::PollFlags::POLLIN,
    )];
    // Arm this end's watch, whose pipe handle must not outlive its owner.
    assert_eq!(
        crate::poll(&mut sources, crate::PollTimeout::try_from(5_u128).unwrap()).unwrap(),
        0
    );
    let mut writer = client.try_clone().unwrap();
    writer.write_all(b"final").unwrap();
    drop(writer);
    drop(client);
    server
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut bytes = Vec::new();
    server.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"final");
}

#[test]
fn a_peers_large_pending_read_does_not_refuse_a_write() {
    // A plain handle's pending read larger than the pipe buffer reduces the counted quota to zero
    // while the pipe is empty, so only a real attempt can tell the writer there is room.
    let path = endpoint();
    let listener = LocalListener::bind(&path).unwrap();
    let mut peer = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    let (mut server, ()) = listener.accept().unwrap();
    server
        .set_write_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let reader = thread::spawn(move || {
        let mut received = Vec::new();
        let mut buffer = vec![0; 1024 * 1024];
        while received.len() < 256 * 1024 {
            let read = peer.read(&mut buffer).unwrap();
            assert_ne!(read, 0);
            received.extend_from_slice(&buffer[..read]);
        }
        received
    });
    thread::sleep(Duration::from_millis(50));
    let bytes: Vec<u8> = (0..=255).cycle().take(256 * 1024).collect();
    server.write_all(&bytes).unwrap();
    assert_eq!(reader.join().unwrap(), bytes);
}

#[test]
fn names_are_local_and_listeners_cannot_be_squatted_twice() {
    assert!(LocalListener::bind(r"\\remote\pipe\orchbee-test").is_err());
    assert!(LocalStream::connect(r"\\remote\pipe\orchbee-test").is_err());
    let path = endpoint();
    let listener = LocalListener::bind(&path).unwrap();
    assert!(LocalListener::bind(&path).is_err());
    drop(listener);
    assert!(LocalListener::bind(&path).is_ok());
}

#[test]
fn child_stdout_is_nonblocking_and_preserves_final_bytes() {
    let (mut reader, output) = ChildOutputReader::pipe().unwrap();
    let mut child = Command::new(std::env::var_os("COMSPEC").unwrap())
        .args(["/d", "/c", "echo child-output"])
        .stdin(Stdio::null())
        .stdout(output)
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut output = Vec::new();
    loop {
        let mut bytes = [0; 256];
        match reader.read(&mut bytes) {
            Ok(0) => break,
            Ok(count) => output.extend_from_slice(&bytes[..count]),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!("child pipe stuck");
                }
                thread::sleep(POLL_INTERVAL);
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("child pipe failed: {error}");
            }
        }
    }
    assert!(child.wait().unwrap().success());
    assert_eq!(output, b"child-output\r\n");
}

#[test]
fn real_process_peer() {
    let Some(path) = std::env::var_os("ORCHBEE_IPC_TEST_PIPE") else {
        return;
    };
    let mut stream = LocalStream::connect(Path::new(&path)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let mut input = [0; 4];
    stream.read_exact(&mut input).unwrap();
    assert_eq!(&input, b"ping");
    stream.write_all(b"pong").unwrap();
    let mut ack = [0];
    stream.read_exact(&mut ack).unwrap();
    assert_eq!(ack, [1]);
}

#[test]
fn verifies_a_real_second_process_before_round_trip() {
    let path = endpoint();
    let listener = LocalListener::bind(&path).unwrap();
    listener.set_nonblocking(true).unwrap();
    let child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "windows::tests::real_process_peer",
            "--nocapture",
        ])
        .env("ORCHBEE_IPC_TEST_PIPE", &path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    struct ChildOwner(std::process::Child);
    impl Drop for ChildOwner {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let mut child = ChildOwner(child);
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut server = loop {
        match listener.accept() {
            Ok((stream, ())) => break stream,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                assert!(Instant::now() < deadline, "child did not connect");
                thread::sleep(POLL_INTERVAL);
            }
            Err(error) => panic!("accept failed: {error}"),
        }
    };
    server
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    server.write_all(b"ping").unwrap();
    let mut reply = [0; 4];
    server.read_exact(&mut reply).unwrap();
    assert_eq!(&reply, b"pong");
    server.write_all(&[1]).unwrap();
    loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        assert!(Instant::now() < deadline, "child did not exit");
        thread::sleep(POLL_INTERVAL);
    }
}
