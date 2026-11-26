//! The system curl as one bounded process group per attempt, owned by the local runtime's Unix
//! command child: the group is stopped and has left before any result, so no descendant or output
//! writer outlives the attempt. curl lives only as long as its own fifteen-second limit, so the
//! standard library's spawn starts it, as it starts other short-lived commands.
use super::super::{CatalogUrl, Failure, LIMIT, cancelled};
use orchbee_local_ipc::ChildOutputReader;
use orchbee_local_runtime::CommandChild;
use std::{
    io::{ErrorKind, Read},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::AtomicBool,
    thread,
    time::{Duration, Instant},
};

/// Part of every macOS installation and where Linux distributions install theirs, so no search path
/// or user-installed copy is consulted; a system without it fails the attempt as a process failure.
const CURL: &str = "/usr/bin/curl";

pub(super) fn curl() -> Result<PathBuf, Failure> {
    Ok(PathBuf::from(CURL))
}

/// Stops the group at cancellation, the deadline, a failed read, or the first byte over the limit,
/// as the Windows adapter stops its Job, and reports that cause once the group has left.
pub(super) fn once(
    program: &Path,
    source: &CatalogUrl,
    stop: &AtomicBool,
    deadline: Instant,
) -> Result<String, Failure> {
    let (mut output, writer) = ChildOutputReader::pipe().map_err(|_| Failure::Process)?;
    let mut command = Command::new(program);
    // No variable reaches curl, so no proxy, configuration or home-folder setting steers it.
    command
        .env_clear()
        .args(super::CURL_ARGUMENTS)
        .arg(&source.0)
        .stdin(Stdio::null())
        .stdout(writer)
        .stderr(Stdio::null());
    // The spawn releases this process's copy of the output writer.
    let mut child = CommandChild::spawn(command).map_err(|_| Failure::Process)?;
    let mut bytes = Vec::new();
    let mut open = true;
    loop {
        let failure = cancelled(stop, deadline)
            .and_then(|()| read_available(&mut output, &mut bytes, &mut open, false))
            .err();
        if let Some(failure) = failure {
            // An unsettled group stays owned by the child's drop, which kills and reaps it.
            return Err(child.settle().map_or(Failure::Process, |_| failure));
        }
        match child.try_wait() {
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Ok(Some(status)) => {
                // The group has left, so every writer it held is closed.
                read_available(&mut output, &mut bytes, &mut open, true)?;
                return match status.code() {
                    Some(0) => String::from_utf8(bytes).map_err(|_| Failure::InvalidUtf8),
                    // curl's own refusal of a body over `--max-filesize`.
                    Some(63) => Err(Failure::TooLarge),
                    _ => Err(Failure::Fetch),
                };
            }
            Err(_) => {
                let _unknown = child.settle();
                return Err(Failure::Process);
            }
        }
    }
}

/// Reads what the pipe holds, a few reads per poll while curl runs. After settlement it reads to
/// the end, which the limit bounds; an empty pipe is then the end too, since a writer that left
/// the group is no longer this attempt's.
fn read_available(
    output: &mut ChildOutputReader,
    bytes: &mut Vec<u8>,
    open: &mut bool,
    settled: bool,
) -> Result<(), Failure> {
    let mut buffer = [0; 8192];
    let mut reads = 0;
    while *open && (settled || reads < 8) {
        reads += 1;
        match output.read(&mut buffer) {
            Ok(0) => *open = false,
            Ok(count) => {
                if bytes.len() + count > LIMIT {
                    return Err(Failure::TooLarge);
                }
                bytes.extend_from_slice(&buffer[..count]);
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                if settled {
                    *open = false;
                }
                return Ok(());
            }
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            Err(_) => return Err(Failure::Fetch),
        }
    }
    Ok(())
}
