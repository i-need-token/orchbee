//! Starts a child on Linux that receives only its three standard streams, as the macOS adapter
//! starts one with `POSIX_SPAWN_CLOEXEC_DEFAULT`, through nix's safe `posix_spawn` wrapper.
//!
//! This process creates its own descriptors close-on-exec, but it can inherit others that are not,
//! such as the sockets, shared memory and cache files of an Electron window that started the
//! Client, and the standard library's spawn passes every one of those on. A Session that outlives
//! that window would then hold them for its whole life. So the launch closes every descriptor
//! above the standard streams that `/proc/self/fd` lists. A descriptor this process creates after
//! the listing is close-on-exec, and closing one that ended after it fails harmlessly in the child.
//!
//! `posix_spawn` cannot set a working directory portably, so a child that needs another directory
//! than this process's, or that waits at a gate, starts as `/bin/sh` with no environment. The shell
//! enters the directory and replaces itself with `/usr/bin/env -i`, which replaces itself with the
//! program in exactly the stated environment, since every shell exports variables of its own, such
//! as `PWD` and `OLDPWD` after `cd`. Both keep the child's process ID and start. A held child waits
//! at the shell for a line on its standard input first; the end of that input makes it exit before
//! it starts the program.
use nix::{
    spawn::{PosixSpawnAttr, PosixSpawnFileActions, PosixSpawnFlags, posix_spawn},
    sys::{
        signal::{SigSet, Signal},
        wait::{WaitPidFlag, WaitStatus, waitpid},
    },
    unistd::Pid,
};
use std::{
    ffi::{CString, OsStr, OsString},
    io,
    os::{
        fd::{AsRawFd, BorrowedFd},
        unix::{
            ffi::{OsStrExt, OsStringExt},
            process::ExitStatusExt,
        },
    },
    path::Path,
    process::ExitStatus,
};

const SHELL: &str = "/bin/sh";
/// Enters the directory in `$1`, then runs `env -i` with the variables, program and arguments after
/// it.
const ENTER: &str = "cd -- \"$1\" || exit 126; shift; exec /usr/bin/env -i \"$@\"";
/// As [`ENTER`], first waiting for a line on standard input, which then becomes `/dev/null`.
const ENTER_AFTER_GATE: &str = concat!(
    "cd -- \"$1\" || exit 126; shift; read -r gate || exit 125; ",
    "exec /usr/bin/env -i \"$@\" </dev/null"
);

/// One child to start.
pub(crate) struct Launch<'a> {
    /// An absolute path that holds no `=`, which `env` would read as a variable.
    pub(crate) executable: &'a Path,
    pub(crate) arguments: &'a [&'a OsStr],
    /// Exactly the variables the child receives. One whose name is empty or starts with `-`, which
    /// no shell can name and `env` would read as an option, is left out.
    pub(crate) environment: &'a [(OsString, OsString)],
    pub(crate) directory: &'a Path,
    /// The child's standard input, output and error, duplicated onto 0 to 2 in order.
    pub(crate) streams: [BorrowedFd<'a>; 3],
    /// The child leads a new process group whose identifier is its own.
    pub(crate) process_group: bool,
    /// The child waits for a line on its standard input before it starts the program.
    pub(crate) gated: bool,
}

/// Starts `launch` with an empty signal mask and `SIGPIPE` at its default action, and returns the
/// child's process identifier; the caller reaps it through [`wait`]. A program that does not exist
/// or cannot be run, or a directory that cannot be entered, is refused before any process starts.
pub(crate) fn spawn(launch: &Launch<'_>) -> io::Result<u32> {
    if launch.executable.is_relative() || launch.executable.as_os_str().as_bytes().contains(&b'=') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a child's executable must be an absolute path without '='",
        ));
    }
    rustix::fs::access(launch.executable, rustix::fs::Access::EXEC_OK)?;
    let variables = launch
        .environment
        .iter()
        .filter(|(name, _)| !name.is_empty() && !name.as_bytes().starts_with(b"-"))
        .map(|(name, value)| {
            let mut entry = name.clone().into_vec();
            entry.push(b'=');
            entry.extend_from_slice(value.as_bytes());
            c_string(entry)
        })
        .collect::<io::Result<Vec<_>>>()?;
    let here = std::env::current_dir().is_ok_and(|current| current == launch.directory);
    // The macOS adapter's change of directory fails the launch itself, so a directory that is not
    // there or cannot be entered is refused here too; one removed after this check makes the shell
    // exit 126.
    if !here {
        if !std::fs::metadata(launch.directory)?.is_dir() {
            return Err(io::ErrorKind::NotADirectory.into());
        }
        rustix::fs::access(launch.directory, rustix::fs::Access::EXEC_OK)?;
    }
    let program = c_string(launch.executable.as_os_str().as_bytes().to_vec())?;
    let operands = launch
        .arguments
        .iter()
        .map(|argument| c_string(argument.as_bytes().to_vec()))
        .collect::<io::Result<Vec<_>>>()?;
    let (path, arguments, environment) = if here && !launch.gated {
        let arguments = std::iter::once(program.clone()).chain(operands).collect();
        (program, arguments, variables)
    } else {
        let script = if launch.gated {
            ENTER_AFTER_GATE
        } else {
            ENTER
        };
        let leading = ["sh", "-c", script, "sh"].map(|text| c_string(text.as_bytes().to_vec()));
        let mut arguments = leading.into_iter().collect::<io::Result<Vec<_>>>()?;
        arguments.push(c_string(launch.directory.as_os_str().as_bytes().to_vec())?);
        arguments.extend(variables);
        arguments.push(program);
        arguments.extend(operands);
        (c_string(SHELL.as_bytes().to_vec())?, arguments, Vec::new())
    };
    // The streams are duplicated onto 0 to 2 in order, so a source below 3 could be overwritten
    // before it is read; such a source is copied above them first.
    let copies = launch
        .streams
        .iter()
        .map(|stream| {
            (stream.as_raw_fd() < 3)
                .then(|| rustix::io::fcntl_dupfd_cloexec(stream, 3))
                .transpose()
        })
        .collect::<Result<Vec<_>, _>>()?;
    let inherited = open_descriptors()?;
    let mut actions = PosixSpawnFileActions::init()?;
    for ((target, stream), copy) in (0..).zip(launch.streams.iter()).zip(&copies) {
        let source = copy.as_ref().map_or(stream.as_raw_fd(), AsRawFd::as_raw_fd);
        actions.add_dup2(source, target)?;
    }
    close_each(&mut actions, &inherited)?;
    let mut attributes = PosixSpawnAttr::init()?;
    let mut flags =
        PosixSpawnFlags::POSIX_SPAWN_SETSIGMASK | PosixSpawnFlags::POSIX_SPAWN_SETSIGDEF;
    if launch.process_group {
        flags |= PosixSpawnFlags::POSIX_SPAWN_SETPGROUP;
        attributes.set_pgroup(Pid::from_raw(0))?;
    }
    attributes.set_flags(flags)?;
    attributes.set_sigmask(&SigSet::empty())?;
    let mut defaults = SigSet::empty();
    defaults.add(Signal::SIGPIPE);
    attributes.set_sigdefault(&defaults)?;
    let child = posix_spawn(
        path.as_c_str(),
        &actions,
        &attributes,
        &arguments,
        &environment,
    )?;
    u32::try_from(child.as_raw())
        .map_err(|_| io::Error::other("posix_spawn reported an invalid process ID"))
}

/// The status of `process` once it has ended, or `None` while it runs when `block` is false.
/// `process` must be a child this process spawned and has not yet reaped; the caller keeps the
/// status, because a reaped identifier may name another process later.
pub(crate) fn wait(process: u32, block: bool) -> io::Result<Option<ExitStatus>> {
    let pid = Pid::from_raw(
        i32::try_from(process).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?,
    );
    let flags = (!block).then_some(WaitPidFlag::WNOHANG);
    loop {
        match waitpid(pid, flags) {
            Ok(WaitStatus::Exited(_, code)) => {
                return Ok(Some(ExitStatus::from_raw((code & 0xff) << 8)));
            }
            Ok(WaitStatus::Signaled(_, signal, core)) => {
                let core = if core { 0x80 } else { 0 };
                return Ok(Some(ExitStatus::from_raw(signal as i32 | core)));
            }
            Ok(_) => return Ok(None),
            Err(nix::errno::Errno::EINTR) => {}
            Err(error) => return Err(error.into()),
        }
    }
}

/// Adds an action that closes each of `descriptors` after the streams are in place.
pub(crate) fn close_each(
    actions: &mut PosixSpawnFileActions,
    descriptors: &[i32],
) -> nix::Result<()> {
    for descriptor in descriptors {
        match actions.add_close(*descriptor) {
            // A descriptor above the current limit cannot be closed, nor can it be open.
            Ok(()) | Err(nix::errno::Errno::EBADF) => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

/// Every descriptor of this process above the standard streams, for [`close_each`].
pub(crate) fn open_descriptors() -> io::Result<Vec<i32>> {
    let mut descriptors = Vec::new();
    for entry in std::fs::read_dir("/proc/self/fd")? {
        let name = entry?.file_name();
        if let Some(descriptor) = name.to_str().and_then(|name| name.parse::<i32>().ok())
            && descriptor > 2
        {
            descriptors.push(descriptor);
        }
    }
    Ok(descriptors)
}

fn c_string(bytes: Vec<u8>) -> io::Result<CString> {
    CString::new(bytes).map_err(|_| io::ErrorKind::InvalidInput.into())
}
