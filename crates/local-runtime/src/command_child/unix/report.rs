//! A [`CommandChild`] owner's report of the groups it leads children in, which its launcher reads to
//! end the groups the owner leaves when it ends first.
//!
//! An owner settles each group it starts, but only while it runs: one that is killed leaves its
//! groups running, since the system gives their leaders to another parent and nothing holds the
//! groups any more. So the owner writes `+<pid> <start>` to a pipe its launcher reads once a group
//! started, and `-<pid> <start>` once the group settled; once the owner ended, the launcher ends each
//! group still reported, `SIGTERM` and then `SIGKILL` after two seconds, as a settlement does. Each
//! line is one write of fewer than `PIPE_BUF` bytes, so it arrives whole, and any other line, such as
//! a readiness marker, is passed over.
//!
//! A group is signalled only while its leader still runs as reported, compared right before each
//! signal: a running leader keeps its process ID, and with it the group's, from naming another
//! process, and the window between that comparison and the signal is the one every `kill` has. A
//! member that outlives a leader which ended on `SIGTERM` therefore receives no `SIGKILL`, and a group
//! whose start the owner did not report before it was killed is not known at all.
use std::{
    collections::HashSet,
    fs::File,
    io::{self, Read, Write},
    os::fd::AsFd,
    thread,
    time::{Duration, Instant},
};

use orchbee_local_ipc::{PollFd, PollFlags, PollTimeout, poll};
use rustix::{
    io::Errno,
    process::{Pid, Signal, kill_process_group},
};

use super::{CommandChild, POLL_SLICE, TERM_GRACE};

/// How often the launcher looks again whether the owner ended while its report stays open, as when
/// a process that inherited the report's writer still holds it.
const WATCH_SLICE: Duration = Duration::from_millis(500);
/// The groups the launcher holds for one owner; a group reported past them is left out.
const MAXIMUM_GROUPS: usize = 1024;
/// The longest line of a report: a sign, a process ID, a space and a start, without its line feed.
const MAXIMUM_LINE: usize = 1 + 10 + 1 + 20;

/// An owner's report to the launcher that started it.
pub struct GroupReport {
    output: File,
}

impl GroupReport {
    /// A report through this process's standard output, which its launcher reads.
    ///
    /// # Errors
    ///
    /// Returns an error when standard output cannot be duplicated.
    pub fn standard_output() -> io::Result<Self> {
        let output = io::stdout().as_fd().try_clone_to_owned()?;
        Ok(Self {
            output: File::from(output),
        })
    }

    /// Reports `child`'s group until the returned guard drops, which reports it settled; nothing for
    /// a child whose root already ended.
    ///
    /// # Errors
    ///
    /// Returns an error when the line cannot be written, as once the launcher stopped reading.
    pub fn report(&self, child: &CommandChild) -> io::Result<Option<ReportedGroup<'_>>> {
        let Ok(group) = child.exact_identity() else {
            return Ok(None);
        };
        self.write('+', group)?;
        Ok(Some(ReportedGroup {
            report: self,
            group,
        }))
    }

    fn write(&self, sign: char, (pid, start): (u32, u64)) -> io::Result<()> {
        (&self.output).write_all(format!("{sign}{pid} {start}\n").as_bytes())
    }
}

/// A group its owner reported, until this drops once the owner settled it.
pub struct ReportedGroup<'a> {
    report: &'a GroupReport,
    group: (u32, u64),
}

impl Drop for ReportedGroup<'_> {
    fn drop(&mut self) {
        // The owner reaped the leader while settling, so a launcher that misses this line, or no
        // longer reads, finds the leader gone and signals nothing.
        let _ = self.report.write('-', self.group);
    }
}

/// What [`end_reported_groups`] ended.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EndedGroups {
    /// The groups whose leader still ran as reported once the owner ended, each of which was
    /// signalled.
    pub signalled: usize,
    /// The groups reported while the report already held as many as it keeps, which were left out.
    pub left_out: usize,
}

/// Reads the report the owner `owner`, a process ID and start time, writes to `reports` until the
/// owner ended, then ends each group still reported whose leader runs as reported: `SIGTERM`, then
/// `SIGKILL` to each leader still running two seconds later. The report ends at the end of its pipe
/// or, while a process that inherited its writer keeps it open, once the owner is seen ended; either
/// way the groups wait for the owner's end, so a running owner's groups are never signalled.
///
/// # Errors
///
/// Returns an error when the report cannot be read or a group cannot be signalled, after ending the
/// groups it read; a group that cannot be signalled leaves the others signalled all the same.
pub fn end_reported_groups(
    mut reports: impl Read + AsFd,
    owner: (u32, u64),
) -> io::Result<EndedGroups> {
    let watch = PollTimeout::try_from(WATCH_SLICE)
        .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
    let mut groups = Groups::default();
    let mut buffer = [0_u8; 4096];
    let read = loop {
        match readable(&reports, watch) {
            Ok(true) => {}
            Ok(false) if crate::process_ended(owner) => {
                break take_buffered(&mut reports, &mut groups, &mut buffer);
            }
            Ok(false) => continue,
            Err(error) => break Err(error),
        }
        match reports.read(&mut buffer) {
            Ok(0) => break Ok(()),
            Ok(count) => groups.take(buffer.get(..count).unwrap_or_default()),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => break Err(error),
        }
    };
    // An owner holds its standard output until it ends, so this wait is short after the pipe ends,
    // and a report that failed still waits for it.
    while !crate::process_ended(owner) {
        thread::sleep(WATCH_SLICE);
    }
    let signalled = end(&groups.reported)?;
    read.map(|()| EndedGroups {
        signalled,
        left_out: groups.left_out,
    })
}

/// Whether `reports` has bytes or its end to read within `timeout`.
fn readable(reports: &impl AsFd, timeout: PollTimeout) -> io::Result<bool> {
    let mut sources = [PollFd::new(reports.as_fd(), PollFlags::POLLIN)];
    match poll(&mut sources, timeout) {
        Ok(ready) => Ok(ready > 0),
        Err(error) if error.kind() == io::ErrorKind::Interrupted => Ok(false),
        Err(error) => Err(error),
    }
}

/// Takes the lines an ended owner left in `reports` without waiting for more, since a process that
/// inherited the writer may keep the pipe open.
fn take_buffered(
    reports: &mut (impl Read + AsFd),
    groups: &mut Groups,
    buffer: &mut [u8],
) -> io::Result<()> {
    loop {
        if !readable(&*reports, PollTimeout::ZERO)? {
            return Ok(());
        }
        match reports.read(buffer) {
            Ok(0) => return Ok(()),
            Ok(count) => groups.take(buffer.get(..count).unwrap_or_default()),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
}

/// Signals each group of `reported` whose leader still runs as reported: `SIGTERM`, then, once every
/// leader ended or the grace passed, `SIGKILL` to each leader still running. Answers how many ran.
fn end(reported: &HashSet<(u32, u64)>) -> io::Result<usize> {
    let running = reported
        .iter()
        .copied()
        .filter(|group| leads(*group))
        .collect::<Vec<_>>();
    let mut failure = None;
    for group in &running {
        signal(*group, Signal::TERM, &mut failure);
    }
    let grace = Instant::now() + TERM_GRACE;
    while Instant::now() < grace && running.iter().any(|group| leads(*group)) {
        thread::sleep(POLL_SLICE);
    }
    for group in &running {
        signal(*group, Signal::KILL, &mut failure);
    }
    failure.map_or(Ok(running.len()), Err)
}

/// Sends `signal` to the group `group` names while its leader still runs as reported, keeping the
/// first failure in `failure`. A group with no member left has nothing to signal, which macOS
/// refuses with `EPERM`.
fn signal(group: (u32, u64), signal: Signal, failure: &mut Option<io::Error>) {
    let Some(pid) = i32::try_from(group.0).ok().and_then(Pid::from_raw) else {
        return;
    };
    if !leads(group) {
        return;
    }
    match kill_process_group(pid, signal) {
        Ok(()) | Err(Errno::SRCH | Errno::PERM) => {}
        Err(error) => {
            failure.get_or_insert(error.into());
        }
    }
}

/// Whether the leader `group` names still runs with its reported start. A start that cannot be read
/// names no leader, so nothing is signalled without it.
fn leads((pid, start): (u32, u64)) -> bool {
    crate::process_observation::identity(pid).is_ok_and(|leader| leader.creation_time() == start)
}

/// The groups a report names so far.
#[derive(Default)]
struct Groups {
    reported: HashSet<(u32, u64)>,
    left_out: usize,
    /// The start of a line the last read ended in, kept up to one byte past the longest line, so a
    /// longer line is passed over.
    line: Vec<u8>,
}

impl Groups {
    fn take(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            if byte == b'\n' {
                self.end_line();
            } else if self.line.len() <= MAXIMUM_LINE {
                self.line.push(byte);
            }
        }
    }

    fn end_line(&mut self) {
        let line = std::mem::take(&mut self.line);
        if line.len() > MAXIMUM_LINE {
            return;
        }
        let Some((&sign, group)) = line.split_first() else {
            return;
        };
        let Some(group) = parse(group) else {
            return;
        };
        match sign {
            b'+' if self.reported.len() < MAXIMUM_GROUPS => {
                self.reported.insert(group);
            }
            b'+' => self.left_out += 1,
            b'-' => {
                self.reported.remove(&group);
            }
            _ => {}
        }
    }
}

/// A process ID and start, as decimal digits around one space.
fn parse(group: &[u8]) -> Option<(u32, u64)> {
    let group = std::str::from_utf8(group).ok()?;
    let (pid, start) = group.split_once(' ')?;
    let digits = |text: &str| !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit());
    if !digits(pid) || !digits(start) {
        return None;
    }
    Some((pid.parse().ok()?, start.parse().ok()?))
}

#[cfg(test)]
mod tests;
