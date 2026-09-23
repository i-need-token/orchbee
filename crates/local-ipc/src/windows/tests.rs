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
fn bounded_namespace_observation_never_connects_or_invents_absence() {
    use std::ffi::OsStr;
    let path = endpoint();
    let listener = LocalListener::bind(&path).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    assert_eq!(endpoint_exists_before(&path, deadline).unwrap(), Some(true));
    assert_eq!(listener.readiness().unwrap(), Readiness::Pending);
    let client = LocalStream::connect(&path).unwrap();
    let (server, ()) = listener.accept().unwrap();
    drop((client, server, listener));
    assert_eq!(
        endpoint_exists_before(&path, deadline).unwrap(),
        Some(false)
    );
    assert_eq!(endpoint_exists_before(&path, Instant::now()).unwrap(), None);
    let names = || [Ok("other".into()), Ok("TARGET".into())].into_iter();
    assert_eq!(
        observe_pipe_names(OsStr::new("target"), names(), deadline, 1).unwrap(),
        None
    );
    assert_eq!(
        observe_pipe_names(OsStr::new("target"), names(), deadline, 2).unwrap(),
        Some(true)
    );
    assert_eq!(
        observe_pipe_names(OsStr::new("missing"), names(), deadline, 3).unwrap(),
        Some(false)
    );
    assert!(
        observe_pipe_names(
            OsStr::new("target"),
            [Err(io::ErrorKind::PermissionDenied.into())].into_iter(),
            deadline,
            2
        )
        .is_err()
    );
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
