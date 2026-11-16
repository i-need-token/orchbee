//! Real paths for the runtime script operands a POSIX sample leaves unverified. A package manager
//! installs a JavaScript Agent on POSIX as a bin link, and the shebang hands the runtime that link,
//! where npm's Windows shim hands it the package entry. The runtime reads the file the link
//! resolves to, so that path, read against the process's own working folder, is the evidence a
//! Windows command line carries. A row adopts it only when it proves a provider; every other row,
//! and every Windows row, stays as the Host reported it.
//!
//! The operand and the working folder come from another process, and a path on an unresponsive
//! network mount can hold a file system call indefinitely. Only one retained thread reads the file
//! system: a sample waits a short fixed time for its answer and otherwise keeps the Host's row, so
//! one stuck lookup holds that thread alone and never a terminal's routing.
#[cfg(not(windows))]
use orchbee_agent_observation_protocol::{
    AgentProcessRecognition as Recognition, AgentUnresolvedProcess, classify_agent_process,
    unverified_script_operand,
};
#[cfg(not(windows))]
use orchbee_terminal_protocol::TerminalProcess;
use orchbee_terminal_protocol::TerminalProcessInfo;
#[cfg(not(windows))]
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock, mpsc},
    thread,
    time::{Duration, Instant},
};

/// How long a sample waits for one real path. A later answer still serves later samples.
#[cfg(not(windows))]
pub(super) const ANSWER_WAIT: Duration = Duration::from_millis(25);
/// How long a remembered answer serves its path, so a terminal sampled every few seconds reads a
/// link once.
#[cfg(not(windows))]
const ANSWER_LIFETIME: Duration = Duration::from_secs(10);
#[cfg(not(windows))]
const REMEMBERED_ANSWERS: usize = 64;

/// The sample with each such operand replaced by its real path in its row's command line, or
/// `None` when no row changes.
#[cfg(not(windows))]
pub(crate) fn resolve_script_operands(sample: &TerminalProcessInfo) -> Option<TerminalProcessInfo> {
    static RESOLVER: OnceLock<Option<OperandResolver>> = OnceLock::new();
    let resolver = RESOLVER.get_or_init(|| OperandResolver::spawn(file_real_path));
    resolve_with(resolver.as_ref()?, sample)
}

/// A Windows Host's command line already names the file a runtime runs, so no row changes.
#[cfg(windows)]
pub(crate) fn resolve_script_operands(
    _sample: &TerminalProcessInfo,
) -> Option<TerminalProcessInfo> {
    None
}

#[cfg(not(windows))]
pub(super) fn resolve_with(
    resolver: &OperandResolver,
    sample: &TerminalProcessInfo,
) -> Option<TerminalProcessInfo> {
    let mut resolved: Option<TerminalProcessInfo> = None;
    for (index, row) in sample.processes.iter().enumerate() {
        if let Some(line) = resolved_line(row, resolver) {
            resolved.get_or_insert_with(|| sample.clone()).processes[index].command_line =
                Some(line);
        }
    }
    resolved
}

/// The file `path` resolves to, when it is a regular file with a UTF-8 path.
#[cfg(not(windows))]
pub(super) fn file_real_path(path: &Path) -> Option<String> {
    std::fs::canonicalize(path)
        .ok()
        .filter(|real| real.is_file())?
        .into_os_string()
        .into_string()
        .ok()
}

/// Owns the one thread that reads the file system for samples. A caller skips at once while an
/// earlier lookup is unanswered or another caller is waiting, so at most one call is ever stuck.
#[cfg(not(windows))]
pub(super) struct OperandResolver {
    exchange: Mutex<Exchange>,
}

#[cfg(not(windows))]
struct Exchange {
    requests: mpsc::SyncSender<PathBuf>,
    answers: mpsc::Receiver<(PathBuf, Option<String>)>,
    /// A request was sent whose answer has not been received.
    pending: bool,
    remembered: VecDeque<(PathBuf, Option<String>, Instant)>,
}

#[cfg(not(windows))]
impl OperandResolver {
    /// `None` when the thread cannot start; samples then keep the Host's rows.
    pub(super) fn spawn(
        mut lookup: impl FnMut(&Path) -> Option<String> + Send + 'static,
    ) -> Option<Self> {
        let (requests, received) = mpsc::sync_channel::<PathBuf>(1);
        let (answer, answers) = mpsc::sync_channel(1);
        thread::Builder::new()
            .name("orchbee-script-operands".into())
            .spawn(move || {
                // Ends once its owner drops the request sender.
                while let Ok(path) = received.recv() {
                    let verdict = lookup(&path);
                    if answer.send((path, verdict)).is_err() {
                        return;
                    }
                }
            })
            .ok()?;
        Some(Self {
            exchange: Mutex::new(Exchange {
                requests,
                answers,
                pending: false,
                remembered: VecDeque::new(),
            }),
        })
    }

    /// The lookup's answer for `path`, or `None` when none arrives within [`ANSWER_WAIT`].
    fn real_path(&self, path: &Path) -> Option<String> {
        let Ok(mut exchange) = self.exchange.try_lock() else {
            return None;
        };
        let now = Instant::now();
        exchange
            .remembered
            .retain(|(_, _, at)| now.saturating_duration_since(*at) < ANSWER_LIFETIME);
        if let Some((_, verdict, _)) = exchange.remembered.iter().find(|(p, ..)| p == path) {
            return verdict.clone();
        }
        if exchange.pending {
            let Ok((answered, verdict)) = exchange.answers.try_recv() else {
                return None;
            };
            exchange.pending = false;
            exchange.remember(answered.clone(), verdict.clone(), now);
            if answered == path {
                return verdict;
            }
        }
        exchange.requests.try_send(path.to_owned()).ok()?;
        exchange.pending = true;
        let (answered, verdict) = exchange.answers.recv_timeout(ANSWER_WAIT).ok()?;
        exchange.pending = false;
        exchange.remember(answered.clone(), verdict.clone(), Instant::now());
        (answered == path).then_some(verdict).flatten()
    }
}

#[cfg(not(windows))]
impl Exchange {
    fn remember(&mut self, path: PathBuf, verdict: Option<String>, at: Instant) {
        self.remembered.retain(|(p, ..)| *p != path);
        if self.remembered.len() >= REMEMBERED_ANSWERS {
            self.remembered.pop_front();
        }
        self.remembered.push_back((path, verdict, at));
    }
}

/// The row's command line with its unverified script operand replaced by the file it resolves to,
/// when that file proves a provider and the line still fits the Host's form and bound.
#[cfg(not(windows))]
fn resolved_line(row: &TerminalProcess, resolver: &OperandResolver) -> Option<String> {
    let image = row.image.as_deref()?;
    let mut argv = super::decoded_argv(row.command_line.as_deref())?;
    let index = unverified_script_operand(Some(image), Some(&argv))?;
    let operand = Path::new(&argv[index]);
    let path = if operand.is_absolute() {
        operand.to_owned()
    } else {
        let directory = Path::new(row.current_directory.as_deref()?);
        if !directory.is_absolute() {
            return None;
        }
        directory.join(operand)
    };
    argv[index] = resolver.real_path(&path)?;
    let proven = match classify_agent_process(Some(image), Some(&argv)) {
        Recognition::Interactive { .. } | Recognition::Noninteractive { .. } => true,
        Recognition::Unresolved { provider, reason } => {
            provider.is_some() && reason == AgentUnresolvedProcess::UnclassifiedArguments
        }
        Recognition::NotAgent | Recognition::RemoteClient => false,
    };
    if !proven {
        return None;
    }
    let line = super::host_conventions::line(&argv)?;
    (super::decoded_argv(Some(&line)).as_ref() == Some(&argv)).then_some(line)
}
