//! Actual binary helper process and PowerShell wrapper; no configured SSH endpoint is touched.
#![cfg(windows)]
#[path = "../crates/workbench-client/src/machine_tunnel/program.rs"]
mod program;
use crate::peer_support as support;
use orchbee_local_client_protocol::{ClientHello, LocalClientMessage, read_message, write_message};
use orchbee_local_ipc::{ChildInputWriter, ChildOutputReader, LocalListener};
use orchbee_local_runtime::{Cohort, CommandChild};
use std::{
    io::{self, Read, Write},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use support::Fixture;
struct Owned(CommandChild);
impl Drop for Owned {
    fn drop(&mut self) {
        self.0.settle().unwrap();
    }
}
struct Peer(Option<thread::JoinHandle<()>>);
impl Drop for Peer {
    fn drop(&mut self) {
        if let Some(task) = self.0.take() {
            let _joined = task.join();
        }
    }
}
#[test]
fn tunnel_helper_preserves_admission_full_duplex_binary_and_final_eof_bytes() {
    let fixture = Fixture::new();
    let cohort = Cohort::load(
        fixture.cohort_directory(),
        Instant::now() + Duration::from_secs(15),
    )
    .unwrap();
    let session = fixture.rendezvous();
    for shell in [false, true] {
        let listener = LocalListener::bind(session.client_socket_path()).unwrap();
        listener.set_nonblocking(true).unwrap();
        let identity = cohort.identity().to_owned();
        let data: Vec<u8> = (0..512 * 1024).map(|n| (n % 256) as u8).collect();
        let remote_data = data.clone();
        let mut peer = Peer(Some(thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(35);
            let (mut stream, _) = loop {
                match listener.accept() {
                    Ok(stream) => break stream,
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline);
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(e) => panic!("accept: {e}"),
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(20)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(20)))
                .unwrap();
            assert_eq!(
                read_message(&mut stream).unwrap(),
                LocalClientMessage::ClientHello(ClientHello::new(&identity).unwrap())
            );
            write_message(
                &mut stream,
                &LocalClientMessage::ClientAccepted {
                    generation: std::num::NonZeroU64::new(7).unwrap(),
                },
            )
            .unwrap();
            // Output precedes request draining, forcing simultaneous progress in both directions.
            stream.write_all(&remote_data).unwrap();
            let mut received = vec![0; remote_data.len()];
            stream.read_exact(&mut received).unwrap();
            assert_eq!(received, remote_data);
            stream.write_all(&received).unwrap();
            // Closing the remote pipe must drain the stdio writer before its Job is settled.
        })));
        let exe = fixture.executable();
        let mut command = if shell {
            let remote = program::command(exe.to_str().unwrap(), "default", cohort.identity());
            let mut c = Command::new(
                std::path::PathBuf::from(winsafe::GetSystemDirectory().unwrap())
                    .join("WindowsPowerShell/v1.0/powershell.exe"),
            );
            c.args(remote.split_ascii_whitespace().skip(1));
            c
        } else {
            let mut c = Command::new(exe);
            c.args(["--machine-tunnel-v1", "default", cohort.identity()]);
            c
        };
        let (mut input, stdin) = ChildInputWriter::pipe().unwrap();
        let (mut output, stdout) = ChildOutputReader::pipe().unwrap();
        command
            .stdin(stdin)
            .stdout(stdout)
            .stderr(Stdio::null())
            .env("ORCHBEE_RUNTIME_BASE", fixture.0.join("runtime"))
            .env("APPDATA", fixture.0.join("appdata"))
            .env("USERPROFILE", fixture.0.join("appdata"))
            .env_remove("ORCHBEE_CONFIG_FILE");
        let mut child = Owned(CommandChild::spawn(command).map_err(|e| e.error).unwrap());
        let mut request = Vec::new();
        write_message(
            &mut request,
            &LocalClientMessage::ClientHello(ClientHello::new(cohort.identity()).unwrap()),
        )
        .unwrap();
        request.extend_from_slice(&data);
        let mut expected = Vec::new();
        write_message(
            &mut expected,
            &LocalClientMessage::ClientAccepted {
                generation: std::num::NonZeroU64::new(7).unwrap(),
            },
        )
        .unwrap();
        expected.extend_from_slice(&data);
        expected.extend_from_slice(&data);
        let deadline = Instant::now() + Duration::from_secs(35);
        let mut sent = 0;
        let mut received = Vec::new();
        let mut bytes = [0; 16384];
        loop {
            assert!(
                Instant::now() < deadline,
                "helper stalled: sent {sent}, received {} (shell={shell})",
                received.len()
            );
            if sent < request.len() {
                match input.write(&request[sent..]) {
                    Ok(n) => sent += n,
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                    Err(e) => panic!("stdin: {e}"),
                }
            }
            match output.read(&mut bytes) {
                Ok(0) => break,
                Ok(n) => received.extend_from_slice(&bytes[..n]),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                Err(e) => panic!("stdout: {e}"),
            }
            assert!(received.len() <= expected.len());
            thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(
            received.len(),
            expected.len(),
            "truncated helper output (shell={shell})"
        );
        assert_eq!(received, expected);
        drop(input);
        let status = loop {
            if let Some(status) = child.0.try_wait().unwrap() {
                break status;
            }
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(5));
        };
        assert!(status.success(), "helper status: {status}");
        peer.0.take().unwrap().join().unwrap();
    }
}
#[test]
fn tunnel_helper_refuses_wrong_cohort_without_activating_a_session() {
    let fixture = Fixture::new();
    let output = fixture.run(&["--machine-tunnel-v1", "default", &"0".repeat(64)]);
    assert_eq!(output.status.code(), Some(5));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    assert!(!fixture.0.join("runtime").exists());
}
