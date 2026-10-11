use crate::local_client::LocalClient;
use orchbee_terminal_protocol::{
    CURRENT_PROTOCOL_VERSION, ClientId, ConnectionControl, CorrelationId, Envelope, Hello,
    HostEpoch, InventoryControl, ListTerminals, MAXIMUM_INPUT_BYTES, MAXIMUM_ROW_WINDOW_BYTES,
    MAXIMUM_TERMINAL_FRAME_BYTES, MessageRole, PayloadEncoding, TerminalSummary,
    decode_connection_control, decode_inventory_control, decode_transport_envelope,
    encode_connection_control, encode_inventory_control, encode_transport_envelope,
};
use std::{
    fs,
    io::{self, Read, Write},
    os::unix::{fs::DirBuilderExt, net::UnixStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};
static NEXT: AtomicU64 = AtomicU64::new(1);
const ENDPOINT_ID: &str = "1234567890abcdef1234567890abcdef";
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
pub(super) struct Runtime {
    pub(super) directory: PathBuf,
    pub(super) client: PathBuf,
    worker: Process,
    _server: Process,
    _host: Process,
    _directory: Directory,
}
pub(super) struct Process(pub(super) Child);
impl Process {
    fn ready(&mut self) -> Result<()> {
        use nix::{
            fcntl::{FcntlArg, OFlag, fcntl},
            poll::{PollFd, PollFlags, poll},
        };
        use std::os::fd::AsFd;
        let mut output = self.0.stdout.take().ok_or("child stdout")?;
        fcntl(output.as_fd(), FcntlArg::F_SETFL(OFlag::O_NONBLOCK))?;
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut marker = Vec::new();
        while marker.len() < 6 {
            if Instant::now() >= deadline {
                return Err("readiness deadline".into());
            }
            let mut descriptors = [PollFd::new(output.as_fd(), PollFlags::POLLIN)];
            poll(&mut descriptors, 50_u16)?;
            let mut bytes = [0; 6];
            match output.read(&mut bytes[..6 - marker.len()]) {
                Ok(0) => return Err("child exited before readiness".into()),
                Ok(count) => marker.extend_from_slice(&bytes[..count]),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                Err(e) => return Err(e.into()),
            }
        }
        if marker != b"ready\n" {
            return Err("unexpected readiness".into());
        }
        Ok(())
    }
    pub(super) fn spawn(command: &mut Command) -> io::Result<Self> {
        Ok(Self(
            command
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?,
        ))
    }
}
impl Drop for Process {
    fn drop(&mut self) {
        let _killed = self.0.kill();
        let _waited = self.0.wait();
    }
}
impl Runtime {
    pub(super) fn start() -> Result<Self> {
        let directory = PathBuf::from(format!(
            "/tmp/ob-tui-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&directory)?;
        let directory_guard = Directory(directory.clone());
        let host_path = directory.join(format!("{ENDPOINT_ID}.s"));
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let binary_root = std::env::current_exe()?
            .parent()
            .ok_or("test binary directory")?
            .parent()
            .ok_or("Cargo binary directory")?
            .to_path_buf();
        let host_binary = root.join("apps/terminal-host/zig-out/bin/orchbee-terminal-host");
        for path in [
            &host_binary,
            &binary_root.join("orchbee-server"),
            &binary_root.join("orchbee-worker"),
        ] {
            assert!(
                path.is_file(),
                "build process prerequisite: {}",
                path.display()
            );
        }
        let host = Process::spawn(
            Command::new(host_binary)
                .args(["serve", "--runtime-directory"])
                .arg(&directory)
                .args(["--endpoint-id", ENDPOINT_ID]),
        )?;
        wait_for(|| host_path.exists())?;
        let worker_path = directory.join("worker.sock");
        let client_path = directory.join("client.sock");
        let server = Process::spawn(
            Command::new(binary_root.join("orchbee-server"))
                .arg("--local-endpoint")
                .arg(&worker_path)
                .arg("--client-endpoint")
                .arg(&client_path),
        )?;
        wait_for(|| worker_path.exists() && client_path.exists())?;
        let mut worker = Process::spawn(
            Command::new(binary_root.join("orchbee-worker"))
                .env("SHELL", "/bin/sh")
                .arg("--local-endpoint")
                .arg(&worker_path)
                .arg("--terminal-host-runtime-directory")
                .arg(&directory)
                .args(["--terminal-host-endpoint-id", ENDPOINT_ID])
                .stdout(Stdio::piped()),
        )?;
        worker.ready()?;
        Ok(Self {
            directory,
            client: client_path,
            worker,
            _server: server,
            _host: host,
            _directory: directory_guard,
        })
    }
    pub(super) fn connect(&self) -> Result<LocalClient> {
        Ok(LocalClient::connect(
            &self.client,
            env!("CARGO_PKG_VERSION"),
        )?)
    }
    pub(super) fn retained_and_stop(&mut self) -> Result<usize> {
        // Host admits one physical broker. Release this fixture's Worker before the inventory observer.
        let _stopped = self.worker.0.kill();
        let _waited = self.worker.0.wait();
        let mut stream = UnixStream::connect(self.directory.join(format!("{ENDPOINT_ID}.s")))?;
        stream.set_read_timeout(Some(Duration::from_secs(3)))?;
        stream.set_write_timeout(Some(Duration::from_secs(3)))?;
        let hello = encode_connection_control(&ConnectionControl::Hello(Hello {
            client_id: ClientId::from_bytes([77; 16])?,
            protocol: CURRENT_PROTOCOL_VERSION,
            frame_bytes: MAXIMUM_TERMINAL_FRAME_BYTES as u32,
            input_bytes: MAXIMUM_INPUT_BYTES as u32,
            row_window_bytes: MAXIMUM_ROW_WINDOW_BYTES as u32,
            supported_optional_features: Vec::new(),
        }))?;
        let response = exchange(&mut stream, None, hello.kind, &hello.payload)?;
        let envelope = decode_transport_envelope(&response)?;
        let ConnectionControl::HelloAccepted(hello) =
            decode_connection_control(envelope.kind, envelope.role, envelope.payload)?
        else {
            return Err("Hello response".into());
        };
        let list = encode_inventory_control(&InventoryControl::ListTerminals(ListTerminals {
            cursor: None,
            maximum_terminals: 8,
        }))?;
        let response = exchange(
            &mut stream,
            Some(hello.host_epoch),
            list.kind,
            &list.payload,
        )?;
        let envelope = decode_transport_envelope(&response)?;
        let InventoryControl::TerminalList(list) =
            decode_inventory_control(envelope.kind, envelope.role, envelope.payload)?
        else {
            return Err("inventory response".into());
        };
        let mut running = 0;
        for terminal in list.terminals {
            if let TerminalSummary::Running { core, .. } = terminal {
                running += 1;
                let stop = orchbee_terminal_protocol::encode_lifecycle_control(
                    &orchbee_terminal_protocol::LifecycleControl::ForceTerminateTerminal(
                        orchbee_terminal_protocol::TerminalTarget {
                            terminal_id: core.identity.terminal_id,
                        },
                    ),
                )?;
                exchange(
                    &mut stream,
                    Some(hello.host_epoch),
                    stop.kind,
                    &stop.payload,
                )?;
            }
        }
        Ok(running)
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        let _stopped = self.retained_and_stop();
    }
}
fn exchange(
    stream: &mut UnixStream,
    epoch: Option<HostEpoch>,
    kind: u16,
    payload: &[u8],
) -> Result<Vec<u8>> {
    stream.write_all(&encode_transport_envelope(&Envelope {
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Request,
        kind,
        payload_encoding: PayloadEncoding::ControlJson,
        host_epoch: epoch,
        correlation_id: Some(CorrelationId::from_bytes(
            u128::from(NEXT.fetch_add(1, Ordering::Relaxed)).to_be_bytes(),
        )?),
        optional_message: false,
        payload,
    })?)?;
    let mut header = [0; 64];
    stream.read_exact(&mut header)?;
    let length = u32::from_le_bytes(header[8..12].try_into()?) as usize;
    if !(64..=65536).contains(&length) {
        return Err("observer record bound".into());
    }
    let mut record = vec![0; length];
    record[..64].copy_from_slice(&header);
    stream.read_exact(&mut record[64..])?;
    Ok(record)
}
pub(super) fn wait_for(mut predicate: impl FnMut() -> bool) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !predicate() {
        if Instant::now() >= deadline {
            return Err("process witness deadline".into());
        }
        thread::sleep(Duration::from_millis(5));
    }
    Ok(())
}

struct Directory(PathBuf);
impl Drop for Directory {
    fn drop(&mut self) {
        let _removed = fs::remove_dir_all(&self.0);
    }
}
