//! Repository discovery for one Workspace root: `git rev-parse` at the root, and one level below it
//! only in folders holding `.git`, as VS Code scans to its default depth of one. A repository whose
//! toplevel is above the root is reported apart: it opens only with the user's consent. The
//! repository that holds the root is trusted for its owner, as the Worker trusts the repository of
//! a Directory the Workspace admitted; Git's refusal of any other repository another user owns
//! lists it as unsafe.

use super::{
    command::{self, Access, RelayVariables},
    errors::{Classifier, GitError, GitErrorCode},
    native_paths,
    output::GitOutput,
    runner::{self, Limits},
};
use orchbee_git_process::canonical_path;
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread,
    time::Duration,
};

pub(super) const DEADLINE: Duration = Duration::from_secs(10);
/// Folders probed at once; a root may hold hundreds of repositories.
const CONCURRENCY: usize = 5;
/// Folders VS Code's scan skips by default.
const IGNORED_FOLDERS: [&str; 1] = ["node_modules"];

/// A repository Git reported for a folder.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Found {
    /// The canonical toplevel in native spelling, which names the repository on the wire.
    pub(super) toplevel: String,
    /// The working tree's own Git directory, where an operation in progress leaves its files.
    pub(super) git_dir: PathBuf,
    /// The `safe.directory` value each command passes, in Git's spelling, for the repository
    /// that holds the root; `None` keeps Git's ownership check.
    pub(super) safe_directory: Option<String>,
}

impl Found {
    /// A Git command in the repository's toplevel, trusted when the repository holds the root.
    pub(super) fn command(&self, access: Access, relay: Option<&RelayVariables>) -> Command {
        let mut command = command::git(Path::new(&self.toplevel), access, relay);
        if let Some(safe_directory) = &self.safe_directory {
            command
                .arg("-c")
                .arg(format!("safe.directory={safe_directory}"));
        }
        command
    }
}

/// What `rev-parse` says about one folder.
#[derive(Debug)]
pub(super) enum Probe {
    Repository(Found),
    /// The folder is in no repository's working tree.
    Outside,
    /// Git refused the repository at this native path for its owner.
    Unsafe(String),
    Failed(GitError),
}

/// What one discovery found at and below a root.
#[derive(Debug, Default)]
pub(super) struct Discovered {
    /// The repository whose toplevel is the root, if any.
    pub(super) root: Option<Found>,
    /// Repositories in folders one level below the root, in folder-name order.
    pub(super) nested: Vec<Found>,
    /// The repository above the root that the root is inside, if any.
    pub(super) parent: Option<Found>,
    pub(super) unsafe_paths: Vec<String>,
    /// Why discovery could not run Git, when it could not.
    pub(super) failure: Option<GitError>,
}

/// Discovers the repositories of `root`, which a missing root has none of.
pub(super) fn discover(
    root: &str,
    cancel: &Arc<AtomicBool>,
    output: &Mutex<GitOutput>,
) -> Discovered {
    let mut discovered = Discovered::default();
    let Ok(canonical) = canonical_path(Path::new(root)) else {
        return discovered;
    };
    let root = canonical.as_native();
    let limits = Limits::new("rev-parse", DEADLINE, Arc::clone(cancel));
    let trusted = holder(Path::new(root));
    match probe(Path::new(root), trusted.as_deref(), &limits, output) {
        Probe::Repository(found) if native_paths::same(&found.toplevel, root) => {
            discovered.root = Some(found);
        }
        Probe::Repository(found) => {
            if native_paths::relative(&found.toplevel, root).is_some() {
                discovered.parent = Some(found);
            }
        }
        Probe::Outside => {}
        Probe::Unsafe(path) => discovered.unsafe_paths.push(path),
        Probe::Failed(error) => {
            discovered.failure = Some(error);
            return discovered;
        }
    }
    let folders = holding_git(Path::new(root));
    let next = AtomicUsize::new(0);
    let results = Mutex::new(Vec::new());
    thread::scope(|scope| {
        for _ in 0..CONCURRENCY.min(folders.len()) {
            scope.spawn(|| {
                while let Some(folder) = folders.get(next.fetch_add(1, Ordering::Relaxed)) {
                    let probed = probe(folder, None, &limits, output);
                    if let Ok(mut results) = results.lock() {
                        results.push((folder.clone(), probed));
                    }
                }
            });
        }
    });
    let mut results = results.into_inner().unwrap_or_default();
    results.sort_by(|left, right| left.0.cmp(&right.0));
    for (folder, probed) in results {
        match probed {
            // A folder whose `.git` belongs to the repository around it is not a repository of its
            // own: only a toplevel at the folder itself counts.
            Probe::Repository(found)
                if folder
                    .to_str()
                    .is_some_and(|folder| native_paths::same(&found.toplevel, folder)) =>
            {
                discovered.nested.push(found);
            }
            Probe::Unsafe(path) => discovered.unsafe_paths.push(path),
            Probe::Repository(_) | Probe::Outside | Probe::Failed(_) => {}
        }
    }
    discovered
}

/// The repository whose working tree holds `directory`, as `git rev-parse` reports it. With
/// `trusted`, the `safe.directory` value of the repository expected to hold `directory`, Git skips
/// its ownership check for that repository alone.
pub(super) fn probe(
    directory: &Path,
    trusted: Option<&str>,
    limits: &Limits,
    output: &Mutex<GitOutput>,
) -> Probe {
    let mut command = command::git(directory, Access::Read, None);
    if let Some(trusted) = trusted {
        command.arg("-c").arg(format!("safe.directory={trusted}"));
    }
    command.args(["rev-parse", "--show-toplevel", "--git-dir"]);
    let ran = match runner::run(command, Vec::new(), limits, output) {
        Ok(ran) => ran,
        Err(error) => return Probe::Failed(error),
    };
    if !ran.status.is_some_and(|status| status.success()) {
        let error = GitError::failed(Classifier::General, &ran.stdout, &ran.stderr);
        return match error.git_error_code {
            Some(GitErrorCode::NotASafeGitRepository) => {
                unsafe_path(&ran.stderr).map_or(Probe::Failed(error), Probe::Unsafe)
            }
            // Inside a Git directory or a bare repository there is no working tree to show.
            _ => Probe::Outside,
        };
    }
    let text = String::from_utf8_lossy(&ran.stdout);
    let mut lines = text.lines();
    let (Some(toplevel), Some(git_dir)) = (lines.next(), lines.next()) else {
        return Probe::Outside;
    };
    let Ok(toplevel) = canonical_path(Path::new(toplevel)) else {
        return Probe::Outside;
    };
    let safe_directory = trusted
        .filter(|trusted| *trusted == toplevel.git_spelling())
        .map(str::to_owned);
    Probe::Repository(Found {
        toplevel: toplevel.as_native().to_owned(),
        git_dir: directory.join(git_dir),
        safe_directory,
    })
}

/// The `safe.directory` value of the nearest folder at or above `root` that holds `.git`, in
/// Git's canonical spelling.
fn holder(root: &Path) -> Option<String> {
    let folder = root
        .ancestors()
        .find(|folder| folder.join(".git").symlink_metadata().is_ok())?;
    let canonical = canonical_path(folder).ok()?;
    Some(canonical.git_spelling().into_owned())
}

/// The folders directly below `root` that hold `.git`, skipping links and VS Code's ignored names.
fn holding_git(root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = root.read_dir() else {
        return Vec::new();
    };
    let mut folders = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| !IGNORED_FOLDERS.contains(&name))
        })
        .map(|entry| entry.path())
        .filter(|folder| folder.join(".git").symlink_metadata().is_ok())
        .collect::<Vec<_>>();
    folders.sort();
    folders
}

/// The repository Git names in its dubious-ownership refusal, in native spelling.
fn unsafe_path(stderr: &[u8]) -> Option<String> {
    const LEAD: &str = "detected dubious ownership in repository at '";
    let text = String::from_utf8_lossy(stderr);
    let start = text.find(LEAD)? + LEAD.len();
    let rest = text.get(start..)?;
    let path = rest.get(..rest.find('\'')?)?;
    let native = if native_paths::SEPARATOR == '/' {
        path.to_owned()
    } else {
        path.replace('/', "\\")
    };
    native_paths::well_formed(&native).then_some(native)
}
