//! Host suggestions for a new Machine's address: the plain aliases that `Host` lines name in the
//! person's OpenSSH client configuration and the files it includes, in the order OpenSSH reads
//! them. It reads nothing else, keeps nothing but the aliases, and stops at fixed bounds; a bound
//! exceeded or an include loop yields no suggestions at all. A form reads them on a thread of its
//! own and waits a bounded time, since a configuration or an include may sit on a path that does
//! not answer. It never decides how `ssh` connects, since OpenSSH reads its own configuration for
//! that. `apps/orchbee/docs/machines.md#ssh-host-suggestions` owns the rules.

use crate::machines::valid_target;
use crate::ui_wakeup::UiWakeup;
use std::{
    fmt,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock, PoisonError},
    thread,
    time::{Duration, Instant},
};

/// The most suggestions offered.
const SUGGESTIONS: usize = 32;
/// The most bytes one file may hold.
const FILE_BYTES: u64 = 256 * 1024;
/// The most bytes all files together may hold.
const TOTAL_BYTES: u64 = 1024 * 1024;
/// The most files and directories looked at, found or not, the configuration itself included.
const FILES: usize = 64;
/// The deepest include, as OpenSSH bounds it: the configuration is depth 0.
const DEPTH: usize = 16;
/// The most directory entries an include's `*` and `?` components may list in all.
const DIRECTORY_ENTRIES: usize = 4096;
/// How long a form waits for its suggestions: a read still running then offers none, rather than
/// suggestions appearing while the person types.
const READ_BOUND: Duration = Duration::from_secs(5);

/// The hosts one read finds, set once as it ends.
type Hosts = Arc<OnceLock<Arc<[String]>>>;

/// The one read in flight in this process, which a form opened while it runs joins instead of
/// starting another beside a path that does not answer.
static RUNNING: Mutex<Option<Hosts>> = Mutex::new(None);

/// The suggestions one form reads, on a thread of its own: it never holds the input thread, and a
/// read that fails, cannot start or outlasts its bound offers none, silently.
#[derive(Clone)]
pub(crate) struct HostRead {
    hosts: Hosts,
    deadline: Instant,
}

impl HostRead {
    /// Starts reading below `home`, or joins the read still running for an earlier form; `wakeup`
    /// is woken when a read this call starts ends. Without a home there is nothing to read.
    pub(crate) fn start(home: Option<PathBuf>, now: Instant, wakeup: Option<UiWakeup>) -> Self {
        let deadline = now + READ_BOUND;
        let Some(home) = home else {
            return Self {
                hosts: Arc::new(OnceLock::from(Arc::from([]))),
                deadline,
            };
        };
        let mut running = RUNNING.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(hosts) = running.as_ref().filter(|hosts| hosts.get().is_none()) {
            return Self {
                hosts: hosts.clone(),
                deadline,
            };
        }
        let hosts = Arc::new(OnceLock::new());
        let filled = hosts.clone();
        let started = thread::Builder::new()
            .name("orchbee-ssh-config".into())
            .spawn(move || {
                let _set = filled.set(host_suggestions(&home).into());
                if let Some(wakeup) = wakeup {
                    wakeup.signal();
                }
            });
        if started.is_err() {
            let _set = hosts.set(Arc::from([]));
        }
        *running = Some(hosts.clone());
        Self { hosts, deadline }
    }

    /// The suggestions once the read ended, or none once its bound passed; `None` while it runs.
    pub(crate) fn poll(&self, now: Instant) -> Option<Arc<[String]>> {
        if now >= self.deadline {
            return Some(Arc::from([]));
        }
        self.hosts.get().cloned()
    }
}

impl PartialEq for HostRead {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.hosts, &other.hosts) && self.deadline == other.deadline
    }
}

impl Eq for HostRead {}

impl fmt::Debug for HostRead {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HostRead")
            .field("ended", &self.hosts.get().is_some())
            .finish_non_exhaustive()
    }
}

/// The plain Host aliases of `<home>/.ssh/config` and the files it includes, in first-seen order,
/// each once whatever its letter case, at most 32. Patterns, negations and anything that is not a
/// valid SSH target are left out. A missing configuration or include gives nothing; a file over
/// 256 KiB, over 1 MiB in all, more than 64 files and directories looked at, found or not, an
/// include deeper than 16, or a file that includes itself through its own chain gives no
/// suggestions at all.
pub(crate) fn host_suggestions(home: &Path) -> Vec<String> {
    let mut reading = Reading {
        home,
        ssh: home.join(".ssh"),
        hosts: Vec::new(),
        files: 0,
        bytes: 0,
        entries: 0,
        chain: Vec::new(),
    };
    let config = reading.ssh.join("config");
    match reading.file(&config, 0) {
        Ok(()) => reading.hosts,
        Err(Abandoned) => Vec::new(),
    }
}

/// The folder OpenSSH reads `.ssh/config` below on this platform: `USERPROFILE` on Windows and
/// `HOME` elsewhere, the variable the data root's home follows. `None` when it is unset or not an
/// absolute path.
#[cfg_attr(
    test,
    expect(dead_code, reason = "a unit test never reads the person's SSH home")
)]
pub(crate) fn ssh_home() -> Option<PathBuf> {
    let variable = orchbee_local_runtime::DataRoot::home_variable();
    let home = PathBuf::from(std::env::var_os(variable)?);
    home.is_absolute().then_some(home)
}

/// A bound was exceeded or an include looped, so the whole reading yields nothing.
struct Abandoned;

/// One reading's progress against its bounds.
struct Reading<'home> {
    home: &'home Path,
    ssh: PathBuf,
    hosts: Vec<String>,
    files: usize,
    /// Bytes read so far; never above `TOTAL_BYTES`, since each file is read within what is left.
    bytes: u64,
    entries: usize,
    /// The files whose includes are being read, outermost first, by canonical path.
    chain: Vec<PathBuf>,
}

impl Reading<'_> {
    /// Counts one file or directory looked at, found or not, so many paths that do not answer
    /// add up to the bound like files read.
    fn look(&mut self) -> Result<(), Abandoned> {
        self.files += 1;
        if self.files > FILES {
            return Err(Abandoned);
        }
        Ok(())
    }

    fn file(&mut self, path: &Path, depth: usize) -> Result<(), Abandoned> {
        self.look()?;
        // Only a regular file is opened, so a pipe named by an include cannot block the reading;
        // anything missing or unreadable is skipped as OpenSSH skips a missing include.
        if !fs::metadata(path).is_ok_and(|metadata| metadata.is_file()) {
            return Ok(());
        }
        let Ok(identity) = fs::canonicalize(path) else {
            return Ok(());
        };
        if depth > DEPTH || self.chain.contains(&identity) {
            return Err(Abandoned);
        }
        let Ok(file) = File::open(path) else {
            return Ok(());
        };
        let limit = FILE_BYTES.min(TOTAL_BYTES - self.bytes);
        let mut bytes = Vec::new();
        // One byte past the limit tells a file that fills it from a longer one.
        if file.take(limit + 1).read_to_end(&mut bytes).is_err() {
            return Ok(());
        }
        let read = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        if read > limit {
            return Err(Abandoned);
        }
        self.bytes += read;
        self.chain.push(identity);
        for line in bytes.split(|byte| *byte == b'\n') {
            if let Ok(line) = std::str::from_utf8(line) {
                self.line(line, depth)?;
            }
        }
        self.chain.pop();
        Ok(())
    }

    /// One line's `Host` aliases or `Include` files. An include inside a `Match` or `Host` block
    /// counts as well, since a suggestion names a host before any block could match it.
    fn line(&mut self, line: &str, depth: usize) -> Result<(), Abandoned> {
        let Some((keyword, arguments)) = directive(line) else {
            return Ok(());
        };
        if keyword.eq_ignore_ascii_case("host") {
            for alias in arguments {
                self.alias(alias);
            }
        } else if keyword.eq_ignore_ascii_case("include") {
            for argument in arguments {
                let Some(path) = self.resolve(&argument) else {
                    continue;
                };
                for file in self.expand(&path)? {
                    self.file(&file, depth + 1)?;
                }
            }
        }
        Ok(())
    }

    fn alias(&mut self, alias: String) {
        let plain = !alias.contains(['*', '?']) && !alias.starts_with('!') && valid_target(&alias);
        // OpenSSH matches host patterns without regard to letter case, so `Dev` and `dev` name one
        // host.
        if plain
            && self.hosts.len() < SUGGESTIONS
            && !self
                .hosts
                .iter()
                .any(|host| host.eq_ignore_ascii_case(&alias))
        {
            self.hosts.push(alias);
        }
    }

    /// An include argument as a path: `~/` starts at the home folder and a relative path at the
    /// `.ssh` folder. Another person's `~name` is not looked up.
    fn resolve(&self, argument: &str) -> Option<PathBuf> {
        match argument.strip_prefix('~') {
            Some(rest) => rest
                .strip_prefix(std::path::is_separator)
                .map(|rest| self.home.join(rest)),
            None => Some(self.ssh.join(argument)),
        }
    }

    /// The files an include path names: the path itself, or the matches of its `*` and `?`
    /// components sorted by path, as glob(3) sorts them.
    fn expand(&mut self, path: &Path) -> Result<Vec<PathBuf>, Abandoned> {
        let mut found = vec![PathBuf::new()];
        let mut globbed = false;
        for component in path.components() {
            let name = component.as_os_str();
            let Some(pattern) = name.to_str().filter(|name| name.contains(['*', '?'])) else {
                for candidate in &mut found {
                    candidate.push(name);
                }
                continue;
            };
            globbed = true;
            let mut matched = Vec::new();
            for directory in &found {
                self.look()?;
                let Ok(entries) = fs::read_dir(directory) else {
                    continue;
                };
                for entry in entries {
                    self.entries += 1;
                    if self.entries > DIRECTORY_ENTRIES {
                        return Err(Abandoned);
                    }
                    let Ok(entry) = entry else {
                        continue;
                    };
                    let entry_name = entry.file_name();
                    if entry_name
                        .to_str()
                        .is_some_and(|entry_name| glob_matches(pattern, entry_name))
                    {
                        matched.push(directory.join(entry_name));
                    }
                }
            }
            found = matched;
        }
        if globbed {
            found.sort();
        }
        Ok(found)
    }
}

/// A line's keyword and arguments, as ssh_config(5) reads them: the keyword ends at whitespace or
/// one `=`, arguments split at whitespace, double or single quotes group, and an argument that
/// starts with `#` begins a comment. `None` for a blank or comment line, or one whose quotes are
/// unbalanced, which OpenSSH rejects.
fn directive(line: &str) -> Option<(&str, Vec<String>)> {
    let line = line.trim_matches(|character: char| character.is_ascii_whitespace());
    let end = line
        .find(|character: char| character.is_ascii_whitespace() || character == '=')
        .unwrap_or(line.len());
    let (keyword, rest) = line.split_at(end);
    if keyword.is_empty() || keyword.starts_with('#') {
        return None;
    }
    let rest = rest.trim_start_matches(|character: char| character.is_ascii_whitespace());
    let rest = rest.strip_prefix('=').map_or(rest, |rest| {
        rest.trim_start_matches(|character: char| character.is_ascii_whitespace())
    });
    Some((keyword, arguments(rest)?))
}

fn arguments(text: &str) -> Option<Vec<String>> {
    let mut arguments = Vec::new();
    let mut characters = text.chars().peekable();
    loop {
        while characters
            .next_if(|character| character.is_ascii_whitespace())
            .is_some()
        {}
        if matches!(characters.peek(), None | Some('#')) {
            return Some(arguments);
        }
        let mut argument = String::new();
        let mut quote = None;
        for character in characters.by_ref() {
            match quote {
                None if character.is_ascii_whitespace() => break,
                None if character == '"' || character == '\'' => quote = Some(character),
                Some(open) if character == open => quote = None,
                _ => argument.push(character),
            }
        }
        if quote.is_some() {
            return None;
        }
        arguments.push(argument);
    }
}

/// Whether the file name `name` matches `pattern`, where `*` stands for any run of characters and
/// `?` for one; as in glob(3), a leading `.` matches only a literal one.
fn glob_matches(pattern: &str, name: &str) -> bool {
    if name.starts_with('.') && !pattern.starts_with('.') {
        return false;
    }
    let pattern: Vec<char> = pattern.chars().collect();
    let name: Vec<char> = name.chars().collect();
    let (mut at_pattern, mut at_name) = (0, 0);
    // The last `*` seen and the name position it currently stands up to, to retry it one longer.
    let mut star = None;
    while let Some(&character) = name.get(at_name) {
        match pattern.get(at_pattern) {
            Some('*') => {
                star = Some((at_pattern, at_name));
                at_pattern += 1;
            }
            Some(&wanted) if wanted == '?' || wanted == character => {
                at_pattern += 1;
                at_name += 1;
            }
            _ => {
                let Some((star_pattern, star_name)) = star else {
                    return false;
                };
                star = Some((star_pattern, star_name + 1));
                at_pattern = star_pattern + 1;
                at_name = star_name + 1;
            }
        }
    }
    pattern
        .get(at_pattern..)
        .is_some_and(|rest| rest.iter().all(|wanted| *wanted == '*'))
}

#[cfg(test)]
mod tests;
