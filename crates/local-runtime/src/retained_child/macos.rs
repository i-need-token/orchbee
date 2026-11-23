//! A retained child on macOS starts through the reviewed `posix_spawn` adapter, so it receives only
//! its standard streams however many threads of this process are creating descriptors meanwhile.
use std::{
    ffi::{OsStr, OsString},
    io,
    os::unix::process::ExitStatusExt,
    path::Path,
    process::ExitStatus,
};

/// One spawned child, reaped at most once; its status, once read, answers every later wait.
pub(super) struct Process {
    id: u32,
    status: Option<ExitStatus>,
}

impl Process {
    /// Starts `executable`, an absolute path, in `/` with null standard streams and this process's
    /// environment without the relaunch protocol's variable, `path` replacing `PATH` when given.
    pub(super) fn spawn(
        executable: &Path,
        arguments: &[&OsStr],
        path: Option<&OsStr>,
    ) -> io::Result<Self> {
        Self::start(executable, arguments, path, false)
    }

    /// As [`Self::spawn`], with the child stopped before its first instruction until it receives
    /// `SIGCONT`, so its creator can publish its identity first.
    pub(super) fn spawn_suspended(
        executable: &Path,
        arguments: &[&OsStr],
        path: Option<&OsStr>,
    ) -> io::Result<Self> {
        Self::start(executable, arguments, path, true)
    }

    fn start(
        executable: &Path,
        arguments: &[&OsStr],
        path: Option<&OsStr>,
        suspended: bool,
    ) -> io::Result<Self> {
        // Every caller names an exact resolved artifact. A relative path would be read against
        // `/`, after the directory change, and macOS can report a started child as failed then.
        if executable.is_relative() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "a retained child's executable must be an absolute path",
            ));
        }
        let relaunch = OsStr::new(crate::RELAUNCH_PROTOCOL_VARIABLE);
        let mut environment: Vec<(OsString, OsString)> = std::env::vars_os()
            .filter(|(name, _)| name != relaunch && (path.is_none() || name != "PATH"))
            .collect();
        if let Some(path) = path {
            environment.push(("PATH".into(), path.to_owned()));
        }
        let id = crate::retained_spawn_native::spawn(
            executable,
            arguments,
            &environment,
            Path::new("/"),
            None,
            false,
            suspended,
        )?;
        Ok(Self { id, status: None })
    }

    pub(super) fn id(&self) -> u32 {
        self.id
    }

    pub(super) fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.observe(false)
    }

    pub(super) fn wait(&mut self) -> io::Result<ExitStatus> {
        self.observe(true)?
            .ok_or_else(|| io::Error::other("a blocking wait returned no status"))
    }

    fn observe(&mut self, block: bool) -> io::Result<Option<ExitStatus>> {
        if self.status.is_none() {
            self.status =
                crate::retained_spawn_native::wait(self.id, block)?.map(ExitStatus::from_raw);
        }
        Ok(self.status)
    }
}
