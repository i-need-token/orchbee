//! A retained child on Linux starts through [`crate::retained_spawn_linux`], so it receives only
//! its standard streams, whatever descriptors this process inherited.
use std::{
    ffi::{OsStr, OsString},
    fs::File,
    io::{self, PipeWriter, Write as _},
    os::fd::AsFd,
    path::Path,
    process::ExitStatus,
};

/// One spawned child, reaped at most once; its status, once read, answers every later wait.
pub(super) struct Process {
    id: u32,
    status: Option<ExitStatus>,
    /// The write end of a held child's gate, which only this process holds.
    gate: Option<PipeWriter>,
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

    /// As [`Self::spawn`], with the child held at its gate before it starts `executable`. Linux
    /// cannot create a stopped process, so the child waits as `/bin/sh` for a line on a pipe whose
    /// write end only this process holds. [`Self::resume`] writes that line and the child replaces
    /// itself with `executable`, which keeps its process ID and start, with `/dev/null` as standard
    /// input; the end of that input, as when this process ends first, makes it exit before it starts
    /// `executable`.
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
        gated: bool,
    ) -> io::Result<Self> {
        let relaunch = OsStr::new(crate::RELAUNCH_PROTOCOL_VARIABLE);
        let mut environment: Vec<(OsString, OsString)> = std::env::vars_os()
            .filter(|(name, _)| name != relaunch && (path.is_none() || name != "PATH"))
            .collect();
        if let Some(path) = path {
            environment.push(("PATH".into(), path.to_owned()));
        }
        let null = File::options().read(true).write(true).open("/dev/null")?;
        let (input, gate) = if gated {
            let (reader, writer) = io::pipe()?;
            (Some(reader), Some(writer))
        } else {
            (None, None)
        };
        let id = crate::retained_spawn_linux::spawn(&crate::retained_spawn_linux::Launch {
            executable,
            arguments,
            environment: &environment,
            directory: Path::new("/"),
            streams: [
                input.as_ref().map_or(null.as_fd(), AsFd::as_fd),
                null.as_fd(),
                null.as_fd(),
            ],
            process_group: false,
            gated,
        })?;
        Ok(Self {
            id,
            status: None,
            gate,
        })
    }

    pub(super) fn id(&self) -> u32 {
        self.id
    }

    /// Lets a child [`Self::spawn_suspended`] started run.
    pub(super) fn resume(&mut self) -> io::Result<()> {
        self.gate
            .take()
            .ok_or(io::ErrorKind::NotFound)?
            .write_all(b"\n")
    }

    /// Closes a held child's gate, so it exits before it starts its program.
    pub(super) fn close_gate(&mut self) {
        self.gate = None;
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
            self.status = crate::retained_spawn_linux::wait(self.id, block)?;
        }
        Ok(self.status)
    }
}
