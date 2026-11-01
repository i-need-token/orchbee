//! `server connect`: relays one admitted Client connection over standard input and output, for a
//! process that cannot own local IPC itself, such as the Desktop shell. Activation, peer
//! verification, and OBLC admission happen here. The bytes after admission belong to the parent
//! and stay opaque; this command neither decodes them nor retries a lost connection.

#[cfg(any(target_os = "linux", target_os = "macos", windows))]
use std::io::{self, IsTerminal, Read, Write};

use super::{Failure, FailureKind, Output, failure_output};
use crate::invocation::OutputFormat;
use crate::localization::Locale;

/// The same activation budget `server start` and the TUI use.
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
const ACTIVATION_BUDGET: std::time::Duration = std::time::Duration::from_secs(20);
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
const RELAY_CHUNK_BYTES: usize = 64 * 1024;

pub(crate) fn run(locale: Locale) -> Output {
    match connect() {
        Ok(()) => Output {
            status: 0,
            stdout: Vec::new(),
            stderr: String::new(),
            finalization: None,
        },
        Err(failure) => failure_output(
            failure.source(crate::cli_outcome::local_runtime_source()),
            Some(OutputFormat::Text),
            locale,
        ),
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn connect() -> Result<(), Failure> {
    Err(
        Failure::new(FailureKind::ServiceUnavailable).details(super::stage_reason_details(
            "platform",
            "unsupported-platform",
        )),
    )
}

#[cfg(any(target_os = "linux", target_os = "macos", windows))]
fn connect() -> Result<(), Failure> {
    // Protocol bytes on an interactive console are unusable and would corrupt its state.
    if io::stdin().is_terminal() || io::stdout().is_terminal() {
        return Err(Failure::new(FailureKind::InvalidUsage)
            .details(super::stage_reason_details("connect", "requires-pipes")));
    }
    let started = super::server::activate_for_relay(std::time::Instant::now() + ACTIVATION_BUDGET)?;
    let stream = started
        .client
        .into_admitted_stream()
        .map_err(|_| internal())?;
    let upstream = stream.try_clone().map_err(|_| internal())?;
    let sender = stream.try_clone().map_err(|_| internal())?;
    std::thread::Builder::new()
        .name("server-connect-stdin".to_owned())
        .spawn(move || {
            let mut writer = upstream;
            let _ended = pump(&mut io::stdin().lock(), &mut writer);
            // A closed parent ends the Client connection and with it the output relay.
            let _closed = sender.shutdown(std::net::Shutdown::Both);
        })
        .map_err(|_| internal())?;
    let mut reader = stream;
    // Either side ending is the normal end of this relay; the parent observes the closed pipe.
    let _ended = pump(&mut reader, &mut io::stdout().lock());
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos", windows))]
fn internal() -> Failure {
    Failure::new(FailureKind::InternalFailure).details(super::stage_details("activation"))
}

/// Copies until end of input, flushing each chunk so a small record is never held back.
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
fn pump(reader: &mut impl Read, writer: &mut impl Write) -> io::Result<()> {
    let mut chunk = vec![0_u8; RELAY_CHUNK_BYTES];
    loop {
        let read = match reader.read(&mut chunk) {
            Ok(0) => return Ok(()),
            Ok(read) => read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        writer.write_all(&chunk[..read])?;
        writer.flush()?;
    }
}
