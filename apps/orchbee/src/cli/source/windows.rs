//! Windows source admission: bounded files and inherited pipes with console
//! cancellation. A pipe's owned reader process is reaped before source handoff;
//! the caller's pipe mode is never changed and no blocking thread escapes.
mod pipe;
use super::input::{limit_for, usage};
use crate::cli::input_source::{InputSource, SourceFailure};
use crate::{
    cli::{Failure, FailureKind, SourceFinalization},
    invocation::SendSourceCause,
    windows_signal::ControlInterrupt,
};
use std::{
    fs::File,
    io::{self, IsTerminal, Read},
    os::windows::io::AsHandle,
    time::Instant,
};
pub(super) type ArmError = io::Error;
pub(super) type SignalError = io::Error;

pub(crate) trait SignalWatch {
    fn poll_fd(&self) -> Option<orchbee_local_ipc::LocalIo<'_>>;
    fn take(&mut self) -> io::Result<Option<i32>>;
}
pub(crate) enum Finish {
    Clear,
    Cancelled {
        signo: i32,
        finalization: SourceFinalization,
    },
}
pub(crate) struct ArmedSignals(ControlInterrupt);
impl ArmedSignals {
    pub(in crate::cli) fn requested(&self) -> bool {
        self.0.requested()
    }
    pub(in crate::cli) fn check_before(&mut self, deadline: Instant) -> Result<(), Failure> {
        if self
            .take()
            .map_err(|_| Failure::new(FailureKind::InternalFailure))?
            .is_some()
        {
            Err(Failure::new(FailureKind::Cancelled))
        } else if Instant::now() >= deadline {
            Err(Failure::new(FailureKind::Timeout))
        } else {
            Ok(())
        }
    }
    pub(crate) fn arm() -> io::Result<Self> {
        ControlInterrupt::arm().map(Self)
    }
    pub(crate) fn arm_interrupts() -> io::Result<Self> {
        Self::arm()
    }
    pub(crate) fn finish(self) -> io::Result<Finish> {
        if self.0.finish_clear() {
            Ok(Finish::Clear)
        } else {
            Ok(Finish::Cancelled {
                signo: 2,
                finalization: self.into_finalization(),
            })
        }
    }
    pub(crate) fn into_finalization(self) -> SourceFinalization {
        SourceFinalization::new(self.0)
    }
}
impl SignalWatch for ArmedSignals {
    fn poll_fd(&self) -> Option<orchbee_local_ipc::LocalIo<'_>> {
        Some(self.0.wake())
    }
    fn take(&mut self) -> io::Result<Option<i32>> {
        Ok(self.0.requested().then_some(2))
    }
}

/// Acquire a document through the same cancellable raw-byte boundary as terminal input.
pub(in crate::cli) fn acquire_document(
    path: Option<&str>,
    limit: usize,
    deadline: Instant,
    signals: &mut dyn SignalWatch,
) -> Result<Vec<u8>, SourceFailure> {
    match path {
        Some(path) => acquire_file_limit(path, limit, deadline, signals),
        None => acquire_stdin_limit(limit, deadline, signals),
    }
}

pub(super) fn acquire_file(
    path: &str,
    paste: bool,
    deadline: Instant,
    signals: &mut dyn SignalWatch,
) -> Result<Vec<u8>, SourceFailure> {
    acquire_file_limit(path, limit_for(paste), deadline, signals)
}
fn acquire_file_limit(
    path: &str,
    limit: usize,
    deadline: Instant,
    signals: &mut dyn SignalWatch,
) -> Result<Vec<u8>, SourceFailure> {
    let failure = |cause| usage("--file", Some(path.to_owned()), cause);
    if !std::fs::metadata(path).is_ok_and(|metadata| metadata.is_file()) {
        return Err(failure(SendSourceCause::NotRegularFile));
    }
    let file = File::open(path).map_err(|_| failure(SendSourceCause::Unreadable))?;
    read_regular(
        file,
        InputSource::File,
        Some(path),
        limit,
        deadline,
        signals,
    )
}
pub(super) fn acquire_stdin(
    paste: bool,
    deadline: Instant,
    signals: &mut dyn SignalWatch,
) -> Result<Vec<u8>, SourceFailure> {
    acquire_stdin_limit(limit_for(paste), deadline, signals)
}
fn acquire_stdin_limit(
    limit: usize,
    deadline: Instant,
    signals: &mut dyn SignalWatch,
) -> Result<Vec<u8>, SourceFailure> {
    let stdin = io::stdin();
    if stdin.is_terminal() {
        return Err(usage("--stdin", None, SendSourceCause::InteractiveTerminal));
    }
    let handle = stdin
        .as_handle()
        .try_clone_to_owned()
        .map_err(|_| usage("--stdin", None, SendSourceCause::Unreadable))?;
    let file = File::from(handle);
    let kind = winapi_util::file::typ(&file)
        .map_err(|_| usage("--stdin", None, SendSourceCause::UnsupportedStream))?;
    if kind.is_pipe() {
        return pipe::acquire(file, limit, deadline, signals);
    }
    read_regular(file, InputSource::Stdin, None, limit, deadline, signals)
}
fn read_regular(
    mut file: File,
    source: InputSource,
    path: Option<&str>,
    limit: usize,
    deadline: Instant,
    signals: &mut dyn SignalWatch,
) -> Result<Vec<u8>, SourceFailure> {
    let failure = |cause| usage(source.option(), path.map(str::to_owned), cause);
    if !file.metadata().is_ok_and(|metadata| metadata.is_file()) {
        return Err(failure(if source == InputSource::File {
            SendSourceCause::NotRegularFile
        } else {
            SendSourceCause::UnsupportedStream
        }));
    }
    let mut output = Vec::new();
    loop {
        if let Some(signal) = signals
            .take()
            .map_err(|_| SourceFailure::Standard(Failure::new(FailureKind::InternalFailure)))?
        {
            return Err(SourceFailure::Cancelled { source, signal });
        }
        if Instant::now() >= deadline {
            return Err(SourceFailure::Timeout { source });
        }
        let mut chunk = [0; 8192];
        let capacity = (limit + 1 - output.len()).min(chunk.len());
        let count = file
            .read(&mut chunk[..capacity])
            .map_err(|_| failure(SendSourceCause::Unreadable))?;
        if count == 0 {
            break;
        }
        output.extend_from_slice(&chunk[..count]);
        if output.len() > limit {
            return Err(failure(SendSourceCause::Oversize { limit }));
        }
    }
    if output.is_empty() {
        return Err(failure(SendSourceCause::Empty));
    }
    Ok(output)
}

/// Preserve the native finish result while the facade constructs the shared stage outcome.
pub(super) fn finish(
    signals: ArmedSignals,
) -> Result<Option<(i32, SourceFinalization)>, SignalError> {
    signals.finish().map(|finish| match finish {
        Finish::Clear => None,
        Finish::Cancelled {
            signo,
            finalization,
        } => Some((signo, finalization)),
    })
}

impl SignalWatch for super::ArmedSignals {
    fn poll_fd(&self) -> Option<orchbee_local_ipc::LocalIo<'_>> {
        SignalWatch::poll_fd(&self.0)
    }
    fn take(&mut self) -> io::Result<Option<i32>> {
        SignalWatch::take(&mut self.0)
    }
}

impl super::ArmedSignals {
    pub(in crate::cli) fn arm_interrupts() -> io::Result<Self> {
        ArmedSignals::arm_interrupts().map(Self)
    }

    pub(in crate::cli) fn requested(&self) -> bool {
        self.0.requested()
    }
    pub(in crate::cli) fn check_before(&mut self, deadline: Instant) -> Result<(), Failure> {
        self.0.check_before(deadline)
    }
}
