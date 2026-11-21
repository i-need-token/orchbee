//! Bounded acknowledged records and cleanup over one native stdout relay.
use crate::cli::{Failure, FailureKind};
use std::{
    io, thread,
    time::{Duration, Instant},
};
#[cfg(windows)]
#[path = "relay/windows.rs"]
mod platform;
#[cfg(unix)]
#[path = "relay/unix.rs"]
mod platform;
trait NativeRelay {
    /// Waits until the relay can take records; a relay that writes in place can at once.
    fn ready(&mut self, _cancelled: &dyn Fn() -> bool) -> io::Result<()> {
        Ok(())
    }
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize>;
    fn read_ack(&mut self, bytes: &mut [u8]) -> io::Result<usize>;
    fn stop(&mut self) -> io::Result<()>;
}
pub(in crate::cli) struct Relay {
    native: Box<dyn NativeRelay>,
    healthy: bool,
}
impl Relay {
    /// Starts the relay and returns once it can take records. Its start belongs to no record's
    /// budget, so callers sample their deadlines afterwards.
    pub(in crate::cli) fn start(cancelled: &dyn Fn() -> bool) -> io::Result<Self> {
        Self::once_ready(platform::start()?, cancelled)
    }
    #[cfg(all(test, windows))]
    pub(super) fn with_output(output: std::process::Stdio) -> io::Result<Self> {
        Self::once_ready(platform::with_output(output)?, &|| false)
    }
    fn once_ready(native: Box<dyn NativeRelay>, cancelled: &dyn Fn() -> bool) -> io::Result<Self> {
        // A relay that never becomes ready is still stopped and reaped on drop.
        let mut relay = Self {
            native,
            healthy: true,
        };
        relay.native.ready(cancelled)?;
        Ok(relay)
    }
    pub(in crate::cli) fn healthy(&self) -> bool {
        self.healthy
    }
    pub(in crate::cli) fn record(
        &mut self,
        bytes: &[u8],
        deadline: Instant,
        cancelled: &dyn Fn() -> bool,
    ) -> io::Result<()> {
        if !self.healthy || bytes.is_empty() || bytes.len() > super::super::output::LIMIT {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        // Any interrupted record may have a written prefix. Never append a terminal record to it.
        self.healthy = false;
        let header = u32::try_from(bytes.len())
            .map_err(io::Error::other)?
            .to_le_bytes();
        for part in [header.as_slice(), bytes] {
            let mut remaining = part;
            while !remaining.is_empty() {
                check(deadline, cancelled)?;
                match self.native.write(remaining) {
                    Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
                    Ok(n) => remaining = &remaining[n..],
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => pause(),
                    Err(e) => return Err(e),
                }
            }
        }
        loop {
            check(deadline, cancelled)?;
            let mut ack = [0];
            match self.native.read_ack(&mut ack) {
                Ok(1) if ack[0] == 1 => {
                    self.healthy = true;
                    return Ok(());
                }
                Ok(_) => return Err(io::ErrorKind::BrokenPipe.into()),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => pause(),
                Err(e) => return Err(e),
            }
        }
    }
}
impl Drop for Relay {
    fn drop(&mut self) {
        let _ = self.native.stop();
    }
}
/// The command failure that a relay or stdin reader error stands for: an interruption is a
/// cancellation and an exhausted budget a timeout; every other fault is internal.
pub(in crate::cli) fn failure(error: &io::Error) -> Failure {
    Failure::new(match error.kind() {
        io::ErrorKind::Interrupted => FailureKind::Cancelled,
        io::ErrorKind::TimedOut => FailureKind::Timeout,
        _ => FailureKind::InternalFailure,
    })
}
fn check(deadline: Instant, cancelled: &dyn Fn() -> bool) -> io::Result<()> {
    if cancelled() {
        return Err(io::ErrorKind::Interrupted.into());
    }
    if Instant::now() >= deadline {
        return Err(io::ErrorKind::TimedOut.into());
    }
    Ok(())
}
fn pause() {
    thread::sleep(Duration::from_millis(2));
}

#[cfg(test)]
mod tests;
