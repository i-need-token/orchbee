//! Retains exact child exit observation and transfers a dropped live child to a background
//! reaper while the parent remains alive. It never terminates that child on drop; after parent
//! exit, orphan reaping belongs to the operating system.

#[cfg(not(target_os = "macos"))]
use super::linux::Process;
#[cfg(target_os = "macos")]
use super::macos::Process;
use std::{io, process::ExitStatus, thread};

/// Observes startup exit, then reaps while its parent lives without terminating the child.
/// Parent exit transfers orphan ownership to the operating system.
pub struct RetainedChild(Option<Process>);
impl RetainedChild {
    pub(crate) fn spawn(
        executable: &std::path::Path,
        arguments: &[&std::ffi::OsStr],
    ) -> io::Result<Self> {
        Self::spawn_with_path(executable, arguments, None)
    }
    /// As [`Self::spawn`], with `path` as the child's `PATH` when given.
    pub(crate) fn spawn_with_path(
        executable: &std::path::Path,
        arguments: &[&std::ffi::OsStr],
        path: Option<&std::ffi::OsStr>,
    ) -> io::Result<Self> {
        // The child outlives the activating Client, so it keeps none of that Client's directories
        // busy or reachable, such as a Worktree the person later removes or unmounts. Nor does it
        // receive the relaunch protocol's variable, which only a launcher's own Client may read,
        // nor any descriptor beyond its standard streams (see `macos` and `linux`).
        Process::spawn(executable, arguments, path).map(|child| Self(Some(child)))
    }
    /// As [`Self::spawn`], with the child stopped before it starts `executable` until
    /// [`Self::resume`], so its creator can publish its identity first.
    pub(crate) fn spawn_suspended(
        executable: &std::path::Path,
        arguments: &[&std::ffi::OsStr],
    ) -> io::Result<Self> {
        Self::spawn_suspended_with_path(executable, arguments, None)
    }
    /// As [`Self::spawn_suspended`], with `path` as the child's `PATH` when given. macOS stops the
    /// child before its first instruction; Linux holds it at a gate (see `linux`).
    pub(crate) fn spawn_suspended_with_path(
        executable: &std::path::Path,
        arguments: &[&std::ffi::OsStr],
        path: Option<&std::ffi::OsStr>,
    ) -> io::Result<Self> {
        Process::spawn_suspended(executable, arguments, path).map(|child| Self(Some(child)))
    }
    /// The held child's registered identity: its process ID and start time. It is not reaped
    /// while held, so its ID names no other process.
    pub(crate) fn identity(&self) -> io::Result<crate::process_registration::ProcessIdentity> {
        let id = self.id().ok_or(io::ErrorKind::NotFound)?;
        crate::process_observation::identity(id)
    }
    /// Lets a child [`Self::spawn_suspended`] started run.
    #[cfg(not(target_os = "macos"))]
    pub(crate) fn resume(&mut self) -> io::Result<()> {
        self.0.as_mut().ok_or(io::ErrorKind::NotFound)?.resume()
    }
    /// Lets a child [`Self::spawn_suspended`] started run.
    #[cfg(target_os = "macos")]
    pub(crate) fn resume(&mut self) -> io::Result<()> {
        let id = self.id().ok_or(io::ErrorKind::NotFound)?;
        let pid = nix::unistd::Pid::from_raw(
            i32::try_from(id).map_err(|_| io::Error::from(io::ErrorKind::InvalidData))?,
        );
        nix::sys::signal::kill(pid, nix::sys::signal::Signal::SIGCONT).map_err(io::Error::from)
    }
    /// The held child's exact identity: its process ID and its start, which a peer that checks
    /// processes by both, such as the Keeper, compares.
    ///
    /// # Errors
    ///
    /// Returns an error when the start time cannot be read.
    pub fn exact_identity(&self) -> io::Result<(u32, u64)> {
        self.identity()
            .map(|identity| (identity.pid(), identity.creation_time()))
    }
    /// The operating-system process identifier of the held child.
    pub fn id(&self) -> Option<u32> {
        self.0.as_ref().map(Process::id)
    }
    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        match self.0.as_mut() {
            Some(child) => child.try_wait(),
            None => Ok(None),
        }
    }
    /// Terminates this exact process and waits for its exit until `deadline`; descendants remain
    /// the caller's. The child is still unreaped while held, so its identifier cannot name another
    /// process.
    pub fn terminate_before(&mut self, deadline: std::time::Instant) -> io::Result<ExitStatus> {
        if let Some(status) = self.try_wait()? {
            return Ok(status);
        }
        let id = self.id().ok_or(io::ErrorKind::NotFound)?;
        let pid = nix::unistd::Pid::from_raw(
            i32::try_from(id).map_err(|_| io::Error::from(io::ErrorKind::InvalidData))?,
        );
        if let Err(error) = nix::sys::signal::kill(pid, nix::sys::signal::Signal::SIGKILL) {
            // The child can exit between the first wait and the signal.
            return self.try_wait()?.ok_or_else(|| io::Error::from(error));
        }
        loop {
            if let Some(status) = self.try_wait()? {
                return Ok(status);
            }
            if std::time::Instant::now() >= deadline {
                return Err(io::ErrorKind::TimedOut.into());
            }
            thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}
impl Drop for RetainedChild {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            // A child still held at its gate leaves it once the gate closes, before it starts.
            #[cfg(not(target_os = "macos"))]
            child.close_gate();
            // Failure to allocate a reaper must never kill an already-started service.
            // In that case the OS reaps it when this parent exits.
            let _reaper = thread::Builder::new()
                .name("orchbee-child-reaper".into())
                .spawn(move || {
                    let _status = child.wait();
                });
        }
    }
}
