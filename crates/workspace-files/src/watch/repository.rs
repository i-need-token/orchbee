//! What the watch lane knows of the repository a watched Workspace is in: where Git keeps the
//! index, `HEAD` and references, and which working-tree paths Git ignores, so that a change to
//! an ignored path moves no change list and, where directories are watched one by one, an
//! ignored directory such as build output costs no watches.
use crate::git::{self, Run};
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use std::collections::{BTreeSet, HashMap};
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant};

/// How long each Git command that finds a repository's directories and settings may run.
const DISCOVERY_DEADLINE: Duration = Duration::from_secs(4);
/// Raw output of the list of tracked files that ignore rules match, beyond which the lane stops
/// treating any path of the repository as ignored.
const TRACKED_IGNORED_CAP: usize = 4 * 1024 * 1024;

/// A working tree's repository, with every path canonical as the platform reports it.
#[derive(Debug)]
pub(super) struct Repository {
    top: PathBuf,
    /// The worktree's own Git directory, holding its index and `HEAD`.
    git_dir: PathBuf,
    /// The Git directory every worktree shares, holding references and `info/exclude`.
    common_dir: PathBuf,
    rules: Rules,
}

impl Repository {
    /// The repository `root` is in, or none outside a working tree or when Git cannot say.
    pub(super) fn discover(root: &Path) -> Option<Self> {
        let output = read(
            root,
            &[
                "rev-parse",
                "--show-toplevel",
                "--absolute-git-dir",
                "--git-common-dir",
            ],
        )?;
        let text = String::from_utf8(output).ok()?;
        let mut lines = text.lines();
        let (Some(top), Some(git_dir), Some(common_dir)) =
            (lines.next(), lines.next(), lines.next())
        else {
            return None;
        };
        let top = std::fs::canonicalize(top).ok()?;
        let git_dir = std::fs::canonicalize(git_dir).ok()?;
        // Older Git prints the common directory relative to where it ran.
        let common_dir = std::fs::canonicalize(root.join(common_dir)).ok()?;
        let case_insensitive = read(root, &["config", "--type=bool", "--get", "core.ignorecase"])
            .is_some_and(|value| value.trim_ascii() == b"true");
        let global = read(
            root,
            &["config", "--type=path", "--get", "core.excludesFile"],
        )
        .and_then(|value| String::from_utf8(value).ok())
        .map(|value| PathBuf::from(value.trim_end_matches(['\n', '\r'])))
        .or_else(default_excludes_file);
        let mut excludes_files = vec![common_dir.join("info").join("exclude")];
        excludes_files.extend(global);
        let rules = Rules {
            top: top.clone(),
            case_insensitive,
            excludes: Vec::new(),
            excludes_files,
            files: HashMap::new(),
            directories: HashMap::new(),
            tracked: Tracked::Stale,
        };
        let mut repository = Self {
            top,
            git_dir,
            common_dir,
            rules,
        };
        repository.rules.read_excludes();
        Some(repository)
    }

    pub(super) fn top(&self) -> &Path {
        &self.top
    }

    pub(super) fn git_dir(&self) -> &Path {
        &self.git_dir
    }

    pub(super) fn common_dir(&self) -> &Path {
        &self.common_dir
    }

    /// What a change at `path` inside the repository's Git directories means: `Some(true)` when it
    /// is the index, `HEAD` or a reference, so the change list may differ; `Some(false)` for Git's
    /// other work, objects, logs and lock files; none outside the Git directories.
    pub(super) fn git_path(&self, path: &Path) -> Option<bool> {
        if let Ok(rest) = path.strip_prefix(&self.git_dir)
            && (rest == Path::new("index") || rest == Path::new("HEAD"))
        {
            return Some(true);
        }
        if let Ok(rest) = path.strip_prefix(&self.common_dir) {
            let reference = rest == Path::new("packed-refs")
                || rest.starts_with("refs")
                    && !rest
                        .extension()
                        .is_some_and(|extension| extension == "lock");
            return Some(reference);
        }
        path.starts_with(&self.git_dir).then_some(false)
    }

    /// Whether `path` is the index, whose change may change which tracked files rules match.
    pub(super) fn is_index(&self, path: &Path) -> bool {
        path.strip_prefix(&self.git_dir)
            .is_ok_and(|rest| rest == Path::new("index"))
    }

    /// Whether a change at `path` changes the ignore rules: a `.gitignore` in the working tree, or
    /// the repository's `info/exclude`.
    pub(super) fn is_rule_file(&self, path: &Path) -> bool {
        if path
            .strip_prefix(&self.common_dir)
            .is_ok_and(|rest| rest == Path::new("info/exclude"))
        {
            return true;
        }
        path.starts_with(&self.top)
            && !path.starts_with(&self.git_dir)
            && !path.starts_with(&self.common_dir)
            && path.file_name().is_some_and(|name| name == ".gitignore")
    }

    /// Forgets the ignore rules read so far; they are read again when next needed.
    pub(super) fn rules_changed(&mut self) {
        self.rules.files.clear();
        self.rules.directories.clear();
        self.rules.read_excludes();
        self.rules.tracked = Tracked::Stale;
    }

    /// Notes that the index changed, so the tracked files that rules match are read again.
    pub(super) fn index_changed(&mut self) {
        self.rules.tracked = Tracked::Stale;
    }

    /// Whether Git ignores the working-tree path `path`: rules match it or a directory above it,
    /// and Git does not track it. A directory that holds a tracked file rules match is not
    /// ignored, so the files Git still follows inside it stay followed. When the lane cannot tell,
    /// nothing is ignored.
    pub(super) fn ignored(&mut self, path: &Path, is_dir: bool) -> bool {
        self.rules.ignored(path, is_dir)
    }
}

/// The tracked files that ignore rules match, and every directory above one of them.
#[derive(Debug)]
enum Tracked {
    /// Not read since the index or the rules last changed.
    Stale,
    Known {
        files: BTreeSet<PathBuf>,
        directories: BTreeSet<PathBuf>,
    },
    /// Git could not list them; nothing counts as ignored.
    Unknown,
}

/// Git's ignore rules for one working tree, read lazily: each directory's `.gitignore` when a path
/// beneath it is first asked about, then `info/exclude` and the user's excludes file.
#[derive(Debug)]
struct Rules {
    top: PathBuf,
    case_insensitive: bool,
    excludes: Vec<Gitignore>,
    excludes_files: Vec<PathBuf>,
    /// Each directory's own `.gitignore`, empty where it has none.
    files: HashMap<PathBuf, Gitignore>,
    /// Whether rules match each directory asked about, ignoring the directories above it.
    directories: HashMap<PathBuf, bool>,
    tracked: Tracked,
}

impl Rules {
    fn read_excludes(&mut self) {
        self.excludes = self
            .excludes_files
            .iter()
            .map(|file| self.matcher(&self.top, file))
            .collect();
    }

    fn matcher(&self, directory: &Path, file: &Path) -> Gitignore {
        let mut builder = GitignoreBuilder::new(directory);
        let _set = builder.case_insensitive(self.case_insensitive);
        // A missing or unreadable file adds no rules; a bad line skips that line alone.
        let _partial = builder.add(file);
        builder.build().unwrap_or_else(|_| Gitignore::empty())
    }

    fn ignored(&mut self, path: &Path, is_dir: bool) -> bool {
        let Ok(relative) = path.strip_prefix(&self.top) else {
            return false;
        };
        if relative.as_os_str().is_empty() {
            return false;
        }
        if matches!(self.tracked, Tracked::Stale) {
            self.tracked = self.list_tracked();
        }
        match &self.tracked {
            Tracked::Known { files, directories } => {
                if files.contains(path) || directories.contains(path) {
                    return false;
                }
            }
            Tracked::Stale | Tracked::Unknown => return false,
        }
        let names: Vec<Component<'_>> = relative.components().collect();
        let mut current = self.top.clone();
        for (index, name) in names.iter().enumerate() {
            current.push(name);
            if index + 1 == names.len() {
                return self.matches(&current, is_dir);
            }
            // Git never looks inside a directory its rules match, whatever rules below it say.
            let ignored = match self.directories.get(&current) {
                Some(ignored) => *ignored,
                None => {
                    let ignored = self.matches(&current, true);
                    self.directories.insert(current.clone(), ignored);
                    ignored
                }
            };
            if ignored {
                return true;
            }
        }
        false
    }

    /// Whether the rules match `path` itself: the nearest `.gitignore` with a matching line
    /// decides, then `info/exclude`, then the user's excludes file.
    fn matches(&mut self, path: &Path, is_dir: bool) -> bool {
        let mut directory = path.parent();
        while let Some(current) = directory.filter(|current| current.starts_with(&self.top)) {
            if !self.files.contains_key(current) {
                let matcher = self.matcher(current, &current.join(".gitignore"));
                self.files.insert(current.to_path_buf(), matcher);
            }
            if let Some(matcher) = self.files.get(current) {
                let found = matcher.matched(path, is_dir);
                if found.is_ignore() {
                    return true;
                }
                if found.is_whitelist() {
                    return false;
                }
            }
            directory = current.parent();
        }
        for matcher in &self.excludes {
            let found = matcher.matched(path, is_dir);
            if found.is_ignore() {
                return true;
            }
            if found.is_whitelist() {
                return false;
            }
        }
        false
    }

    fn list_tracked(&self) -> Tracked {
        let output = match git::run(
            &self.top,
            &[
                "ls-files",
                "-z",
                "--cached",
                "--ignored",
                "--exclude-standard",
            ],
            None,
            TRACKED_IGNORED_CAP,
            Instant::now() + DISCOVERY_DEADLINE,
        ) {
            Run::Exited {
                status: Some(0),
                stdout,
                truncated: false,
            } => stdout,
            Run::Exited { .. } | Run::TimedOut | Run::Unavailable => return Tracked::Unknown,
        };
        let mut files = BTreeSet::new();
        let mut directories = BTreeSet::new();
        for record in output.split(|byte| *byte == 0) {
            let Ok(record) = std::str::from_utf8(record) else {
                return Tracked::Unknown;
            };
            if record.is_empty() {
                continue;
            }
            // Pushed name by name: a verbatim Windows path reads no `/` as a separator.
            let mut path = self.top.clone();
            let mut names = record.split('/').peekable();
            while let Some(name) = names.next() {
                path.push(name);
                if names.peek().is_some() {
                    directories.insert(path.clone());
                }
            }
            files.insert(path);
        }
        Tracked::Known { files, directories }
    }
}

/// What a short Git command printed, when it succeeded.
fn read(root: &Path, args: &[&str]) -> Option<Vec<u8>> {
    match git::run(
        root,
        args,
        None,
        64 * 1024,
        Instant::now() + DISCOVERY_DEADLINE,
    ) {
        Run::Exited {
            status: Some(0),
            stdout,
            truncated: false,
        } => Some(stdout),
        Run::Exited { .. } | Run::TimedOut | Run::Unavailable => None,
    }
}

/// Where Git looks for the user's excludes file when `core.excludesFile` is not set.
fn default_excludes_file() -> Option<PathBuf> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .filter(|value| !value.is_empty())
                .map(|home| PathBuf::from(home).join(".config"))
        })?;
    Some(config.join("git").join("ignore"))
}
