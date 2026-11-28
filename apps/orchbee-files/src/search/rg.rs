//! Running rg and what text searches and file listings read from a run besides its results, as
//! VS Code ef5ca45 reads it: whether the folder can be searched at all, the standard error kept for
//! the error message and the symbolic link loops it reports (`RipgrepSymlinkLoopDetector`), and
//! which messages fail the run, by each engine's `rgErrorMsgForDisplay`.
//!
//! A run starts through [`Launch`], which the service fills with the verified program and a test
//! with scripted output. Every rg the service starts runs through `orchbee-git-process`, so a run
//! that stops kills rg's process tree and returns only once it ended; on macOS and Linux the
//! Worker hears of rg's process group until then, to end it should this process be killed.

use std::{path::Path, process::Command};

use orchbee_files_protocol::{ErrorCode, FilesError, Signed, Text, Token};
use orchbee_git_process::{ObserveError, Observer, Run};
use orchbee_local_runtime::{LogLevel, LogValue, log_event};

use super::{js, program::Program};

/// More symbolic link loops than this in one run make the folder's run start again without
/// following links, as `MAX_RIPGREP_SYMLINK_LOOPS`.
const MAXIMUM_SYMLINK_LOOPS: usize = 1000;
/// The standard error a run keeps for its error message, as VS Code keeps under a million
/// characters.
const MAXIMUM_STDERR_BYTES: usize = 1_000_000;
/// The start of a standard error line the loop count reads; a loop's message is near its start.
const MAXIMUM_STDERR_LINE: usize = 64 * 1024;
/// The longest `detail` an error carries.
const MAXIMUM_DETAIL_BYTES: usize = 4096;

/// What one run tells its consumer, and how the consumer stops it. A consumer that stops a run
/// records why itself.
pub(super) trait Feed {
    /// Whether the run goes on; asked before every read and while the last output drains.
    fn more(&mut self) -> bool;
    /// Takes the next bytes of standard output, which may end anywhere; false stops the run.
    fn stdout(&mut self, bytes: &[u8]) -> bool;
    /// Takes the next bytes of standard error, which may end anywhere.
    fn stderr(&mut self, bytes: &[u8]);
}

/// How a run ended.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Ended {
    /// rg exited by itself; its exit status says nothing VS Code reads.
    Exited,
    /// The feed stopped it, and its process tree ended.
    Stopped,
    /// rg is missing, is not the pinned program, or could not start.
    Unavailable,
    /// Its pipes could not be made, reading rg failed, or its process tree could not be proven
    /// ended.
    Failed,
}

/// Starts rg with `arguments` in `folder` and feeds its output to `feed` until it ends.
pub(super) trait Launch: Send + Sync {
    fn run(&self, arguments: &[String], folder: &Path, feed: &mut dyn Feed) -> Ended;
}

/// The verified rg beside this process's executable.
pub(super) struct Process {
    program: Program,
}

impl Process {
    pub(super) const fn new() -> Self {
        Self {
            program: Program::new(),
        }
    }
}

impl Launch for Process {
    fn run(&self, arguments: &[String], folder: &Path, feed: &mut dyn Feed) -> Ended {
        let Some(program) = self.program.verified() else {
            return Ended::Unavailable;
        };
        let mut command = Command::new(program);
        command.args(arguments).current_dir(folder);
        let mut observer = Observed {
            feed,
            #[cfg(unix)]
            group: None,
        };
        match Run::new(command).observe(&mut observer) {
            Ok(_) => Ended::Exited,
            Err(ObserveError::Stopped(Stopped)) => Ended::Stopped,
            Err(ObserveError::Spawn(kind)) => {
                log_event(
                    LogLevel::Warning,
                    "rg-spawn-failed",
                    &[("error", LogValue::from(kind))],
                );
                Ended::Unavailable
            }
            Err(ObserveError::Pipe | ObserveError::Io | ObserveError::Unsettled) => Ended::Failed,
        }
    }
}

/// The feed stopped the run.
struct Stopped;

struct Observed<'f> {
    feed: &'f mut dyn Feed,
    /// The Worker hears of rg's group until the run settled it.
    #[cfg(unix)]
    group: Option<orchbee_local_runtime::ReportedGroup<'static>>,
}

impl Observer for Observed<'_> {
    type Stop = Stopped;

    fn check(&mut self) -> Result<(), Stopped> {
        if self.feed.more() {
            Ok(())
        } else {
            Err(Stopped)
        }
    }

    fn stdout(&mut self, bytes: &[u8]) -> Result<(), Stopped> {
        if self.feed.stdout(bytes) {
            Ok(())
        } else {
            Err(Stopped)
        }
    }

    fn stderr(&mut self, bytes: &[u8]) {
        self.feed.stderr(bytes);
    }

    #[cfg(unix)]
    fn started(&mut self, child: &orchbee_local_runtime::CommandChild) {
        self.group = crate::groups::report(child);
    }
}

/// rg is missing, is not the pinned program, or could not start.
pub(super) fn unavailable() -> FilesError {
    FilesError::unavailable(Token::new("search-binary").expect("the reason is a token"))
}

/// Whether rg can run in `folder`: it is a directory. Checked first, since rg started in a folder
/// that does not exist fails as a missing program does.
///
/// # Errors
///
/// `not-directory`, or the code of the operating system's failure to read the folder.
pub(super) fn searchable(folder: &Path) -> Result<(), FilesError> {
    match std::fs::metadata(folder) {
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Ok(_) => Err(FilesError::new(ErrorCode::NotDirectory)),
        Err(error) => {
            let code = match error.kind() {
                std::io::ErrorKind::NotFound => ErrorCode::NotFound,
                std::io::ErrorKind::PermissionDenied => ErrorCode::PermissionDenied,
                _ => ErrorCode::Failed,
            };
            let mut failure = FilesError::new(code);
            failure.os_error = error
                .raw_os_error()
                .and_then(|number| Signed::new(i64::from(number)).ok());
            Err(failure)
        }
    }
}

/// One run's standard error: what is kept for its error message and the loops it reported.
#[derive(Default)]
pub(super) struct Stderr {
    kept: Vec<u8>,
    /// The start of the line not yet ended.
    line: Vec<u8>,
    loops: usize,
}

impl Stderr {
    /// Takes the next bytes, counting the loops they report while `counting`; true once more
    /// loops than a run may report were counted.
    pub(super) fn push(&mut self, bytes: &[u8], counting: bool) -> bool {
        if self.kept.len() + bytes.len() < MAXIMUM_STDERR_BYTES {
            self.kept.extend_from_slice(bytes);
        }
        if !counting {
            return false;
        }
        let mut rest = bytes;
        loop {
            let (part, end) = match rest.iter().position(|byte| *byte == b'\n') {
                Some(end) => (&rest[..end], Some(end)),
                None => (rest, None),
            };
            let room = MAXIMUM_STDERR_LINE.saturating_sub(self.line.len());
            self.line.extend_from_slice(&part[..part.len().min(room)]);
            let Some(end) = end else {
                break;
            };
            if contains(&self.line, b"File system loop found") {
                self.loops += 1;
            }
            self.line.clear();
            rest = &rest[end + 1..];
        }
        self.loops > MAXIMUM_SYMLINK_LOOPS
    }

    /// What was kept.
    pub(super) fn kept(&self) -> &[u8] {
        &self.kept
    }
}

/// The text search's `rgErrorMsgForDisplay`: the failure a run's standard error reports, when it is
/// one VS Code shows; any other standard error is not a failure.
pub(super) fn text_error(stderr: &[u8]) -> Option<FilesError> {
    if stderr.is_empty() {
        return None;
    }
    let text = String::from_utf8_lossy(stderr);
    let lines: Vec<&str> = text.split('\n').collect();
    let first = js::trim(lines.first().copied().unwrap_or_default());
    let (code, message) = if lines
        .iter()
        .any(|line| line.starts_with("regex parse error"))
    {
        (ErrorCode::InvalidRegex, regex_parse_error(&lines))
    } else if let Some((_, name)) = first.split_once("grep config error: unknown encoding: ") {
        (
            ErrorCode::InvalidRequest,
            format!("Unknown encoding: {name}"),
        )
    } else if first.starts_with("error parsing glob") {
        (ErrorCode::InvalidRequest, uppercase_first(first))
    } else if first.starts_with("the literal") {
        (ErrorCode::InvalidRegex, uppercase_first(first))
    } else if first.starts_with("PCRE2: error compiling pattern") {
        (ErrorCode::InvalidRegex, first.to_owned())
    } else {
        return None;
    };
    Some(with_detail(code, &message))
}

/// `buildRegexParseError`.
fn regex_parse_error(lines: &[&str]) -> String {
    let mut message = "Regex parse error".to_owned();
    if let Some(pcre2) = lines.iter().find(|line| line.starts_with("PCRE2:")) {
        let rest = pcre2.replacen("PCRE2:", "", 1);
        if let Some(explanation) = rest.split(':').nth(1) {
            message.push(':');
            message.push_str(explanation);
        }
    }
    message
}

/// An error of `code` whose `detail` is `message`, as much of it as an error carries.
fn with_detail(code: ErrorCode, message: &str) -> FilesError {
    let mut error = FilesError::new(code);
    let mut end = message.len().min(MAXIMUM_DETAIL_BYTES);
    while !message.is_char_boundary(end) {
        end -= 1;
    }
    error.detail = Text::new(message[..end].replace('\0', "")).ok();
    error
}

/// The text with its first character uppercased, as `charAt(0).toUpperCase()` does for the ASCII
/// text rg writes.
fn uppercase_first(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}
