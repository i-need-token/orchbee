//! An invocation-owned process makes inherited synchronous pipe reads
//! cancellable without altering shared handle modes. Payload is opaque stdin;
//! only a fixed script and a numeric bound reach the system PowerShell process.

use super::{InputSource, SignalWatch, SourceFailure, usage};
use crate::{
    cli::{Failure, FailureKind},
    invocation::SendSourceCause,
};
use orchbee_local_ipc::ChildOutputReader;
use std::{
    fs::File,
    io::{self, Read},
    os::windows::process::CommandExt,
    path::PathBuf,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const CADENCE: Duration = Duration::from_millis(2);
const READER: &str = r#"$ErrorActionPreference='Stop'; try {
    $inputStream=[Console]::OpenStandardInput(); $outputStream=[Console]::OpenStandardOutput()
    $buffer=New-Object byte[] 8192; $remaining=__LIMIT__
    while ($remaining -gt 0) {
        $count=$inputStream.Read($buffer,0,[Math]::Min($buffer.Length,$remaining))
        if ($count -eq 0) { break }
        $outputStream.Write($buffer,0,$count); $remaining-=$count
    }
    $outputStream.Flush(); exit 0
} catch { exit 1 }"#;

struct ReaderChild(Child);

impl Drop for ReaderChild {
    fn drop(&mut self) {
        // Kill is harmless after exit; wait releases the process handle. No
        // user shell/profile or descendant is launched by the fixed script.
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn unreadable() -> SourceFailure {
    usage("--stdin", None, SendSourceCause::Unreadable)
}

fn check(deadline: Instant, signals: &mut dyn SignalWatch) -> Result<(), SourceFailure> {
    if let Some(signal) = signals
        .take()
        .map_err(|_| SourceFailure::Standard(Failure::new(FailureKind::InternalFailure)))?
    {
        return Err(SourceFailure::Cancelled {
            source: InputSource::Stdin,
            signal,
        });
    }
    if Instant::now() >= deadline {
        return Err(SourceFailure::Timeout {
            source: InputSource::Stdin,
        });
    }
    Ok(())
}

pub(super) fn acquire(
    file: File,
    limit: usize,
    deadline: Instant,
    signals: &mut dyn SignalWatch,
) -> Result<Vec<u8>, SourceFailure> {
    check(deadline, signals)?;
    let system = winsafe::GetSystemDirectory().map_err(|_| unreadable())?;
    let executable = PathBuf::from(system).join("WindowsPowerShell/v1.0/powershell.exe");
    let (mut output, writer) = ChildOutputReader::pipe().map_err(|_| unreadable())?;
    check(deadline, signals)?;
    let mut command = Command::new(executable);
    command
        .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command"])
        .arg(READER.replace("__LIMIT__", &(limit + 1).to_string()))
        .stdin(Stdio::from(file))
        .stdout(writer)
        .stderr(Stdio::null())
        .creation_flags(0x0800_0000);
    let mut child = ReaderChild(command.spawn().map_err(|_| unreadable())?);
    // Command can retain stdio handles after spawn; retire the parent writer
    // now so EOF reflects the child's lifetime, not this builder's lifetime.
    drop(command);
    collect(&mut output, &mut child, limit, deadline, signals)
}

fn collect(
    output: &mut ChildOutputReader,
    child: &mut ReaderChild,
    limit: usize,
    deadline: Instant,
    signals: &mut dyn SignalWatch,
) -> Result<Vec<u8>, SourceFailure> {
    let mut bytes = Vec::new();
    let mut chunk = [0; 8192];
    loop {
        check(deadline, signals)?;
        let capacity = (limit + 1 - bytes.len()).min(chunk.len());
        match output.read(&mut chunk[..capacity]) {
            Ok(0) => break,
            Ok(count) => {
                bytes.extend_from_slice(&chunk[..count]);
                if bytes.len() > limit {
                    return Err(usage("--stdin", None, SendSourceCause::Oversize { limit }));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(CADENCE.min(deadline.saturating_duration_since(Instant::now())));
            }
            Err(_) => return Err(unreadable()),
        }
    }
    loop {
        check(deadline, signals)?;
        match child.0.try_wait().map_err(|_| unreadable())? {
            Some(status) if status.success() => break,
            Some(_) => return Err(unreadable()),
            None => thread::sleep(CADENCE.min(deadline.saturating_duration_since(Instant::now()))),
        }
    }
    if bytes.is_empty() {
        return Err(usage("--stdin", None, SendSourceCause::Empty));
    }
    Ok(bytes)
}

#[cfg(test)]
#[path = "pipe/tests.rs"]
mod tests;
