//! The uninstall plan: what one run removes, computed as a value from the two resolved roots and
//! what they hold before anything is removed, then checked and fenced.
//!
//! Ownership: a run removes only the entries each root's layout names, and only from a root that
//! carries its layout's mark: the store's pointer `active.json`, the data root's layout record
//! `internal\version.json`. A root that holds anything this run would remove but not the mark
//! refuses the whole run. In a marked store root the platform installer's own files, which its
//! contract with the Client names, are the installer's: a person's run removes them with the
//! store, and the helper form leaves them to the uninstaller that runs it. Anything else in a root
//! stays and is counted. Each mark goes after everything else of its layout, and a root goes only
//! when it carried the mark and is empty by then, so an interrupted run leaves a root that the next
//! run still recognizes.
//!
//! The fence refuses the whole run when a root is unsafe to remove, when the roots overlap, when a
//! root is itself a link or junction, when a target is not strictly inside a root, or when a target
//! is reached through a link or junction. A target that is itself a link stays admitted: removal
//! never follows a link and removes the link alone.
use std::path::{Component, Path, PathBuf};

/// The store root's entries, as install, update, the PATH step and the platform installer's
/// Desktop leave them, in removal order; staging folders `.stage-<process>-<number>` and, when the
/// run removes them, the installer's own files follow. The pointer goes after all of them.
const STORE_ENTRIES: [&str; 5] = [
    "bin",
    "desktop",
    "active.pending",
    "versions",
    "install.lock",
];
/// The store's mark.
pub(super) const STORE_MARK: &str = "active.json";
/// The prefix of a publication's staging folder in the store root.
const STAGE_PREFIX: &str = ".stage-";

/// The data root's entries, the data-root layout's top level, in removal order.
const DATA_ENTRIES: [&str; 5] = [
    "config.toml",
    "sessions",
    "logs",
    "agent-detection",
    "internal",
];
/// The data root's mark, in its last entry.
pub(super) const DATA_MARK: [&str; 2] = ["internal", "version.json"];

/// The data-root entries a default uninstall removes, in removal order; everything else stays.
pub(super) const REMOVED_DATA: [&[&str]; 5] = [
    &["logs"],
    &["internal", "integrations"],
    &["internal", "cache"],
    &["internal", "runtime"],
    &["internal", "tmp"],
];

/// What one path holds, read without following a link.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Presence {
    Missing,
    Directory,
    File,
    /// A symbolic link, junction or other reparse point.
    Link,
    /// The path could not be read, so nothing about it is known.
    Unreadable,
}

/// The platform installer's own files in the store root, named by its contract with the Client,
/// and whether this run removes them.
#[derive(Clone, Copy)]
pub(super) struct Installer {
    /// Names directly in the store root.
    pub(super) files: &'static [&'static str],
    /// A person's run removes them with the store, so nothing of the installer outlives its
    /// registration; the helper form leaves them to the uninstaller that started it and waits for
    /// it, which removes them itself.
    pub(super) remove: bool,
}

/// The two roots one environment owns and the folders no root may contain or equal.
pub(super) struct Roots {
    pub(super) store: PathBuf,
    pub(super) data: PathBuf,
    /// The person's home, the profile's application-data folders, the system folders and the
    /// temporary folder: removing a root that is one of them, or holds one, would remove what
    /// Orchbee does not own.
    pub(super) protected: Vec<PathBuf>,
}

/// What one root holds, read before anything changes and without following a link.
#[derive(Clone, Debug)]
pub(super) struct Contents {
    /// The names directly inside the root; `None` when the folder could not be listed. A missing
    /// root holds none.
    pub(super) names: Option<Vec<String>>,
    /// Whether the root's mark reads as its layout's record.
    pub(super) marked: bool,
}

/// One folder or file the run removes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Target {
    pub(super) path: PathBuf,
    pub(super) presence: Presence,
    /// A name directly below `path` removed after everything else below it, before `path` itself:
    /// the data root's mark in `internal`.
    pub(super) last: Option<&'static str>,
}

/// One root's part of the plan.
#[derive(Debug, Eq, PartialEq)]
pub(super) struct RootPlan {
    pub(super) root: PathBuf,
    /// The layout's entries this run removes, in removal order, missing ones included so that the
    /// report can say they were already absent.
    pub(super) targets: Vec<Target>,
    /// The store's pointer, removed after every target; the data root's mark is a target's `last`.
    pub(super) mark: Option<Target>,
    /// Whether the root itself goes once nothing is left in it: it carried the mark and this run
    /// removes everything of its layout, or it did not exist when the run began.
    pub(super) remove_root: bool,
    /// How many entries the root holds that its layout does not name; they stay.
    pub(super) kept: usize,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct Plan {
    pub(super) data: RootPlan,
    pub(super) store: RootPlan,
}

/// Why the plan was refused; nothing was changed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Refusal {
    /// A root is relative, spells `.` or `..`, is a volume or a folder directly below one, or
    /// equals or holds a protected folder.
    UnsafeRoot(PathBuf),
    /// One root equals or holds the other, so removing one would remove part of the other.
    RootsOverlap,
    /// A target is neither one of the roots nor strictly inside one.
    OutsideRoots(PathBuf),
    /// A target is a root or a parent of one; a root goes only as an empty folder.
    RootParent(PathBuf),
    /// A root is a link or junction, or a target is reached through one or through a path that
    /// could not be read.
    ThroughLink(PathBuf),
    /// The store root holds what this run would remove but not a readable pointer.
    UnmarkedStore(PathBuf),
    /// The data root holds what this run would remove but not a readable layout record.
    UnmarkedData(PathBuf),
}

impl Refusal {
    /// The closed reason token the document reports.
    pub(super) const fn reason(&self) -> &'static str {
        match self {
            Self::UnsafeRoot(_) => "unsafe-root",
            Self::RootsOverlap => "roots-overlap",
            Self::OutsideRoots(_) => "outside-roots",
            Self::RootParent(_) => "root-parent",
            Self::ThroughLink(_) => "through-link",
            Self::UnmarkedStore(_) => "not-orchbee-store",
            Self::UnmarkedData(_) => "not-orchbee-data",
        }
    }

    /// The path the refusal names, when it names one.
    pub(super) fn path(&self) -> Option<&Path> {
        match self {
            Self::UnsafeRoot(path)
            | Self::OutsideRoots(path)
            | Self::RootParent(path)
            | Self::ThroughLink(path)
            | Self::UnmarkedStore(path)
            | Self::UnmarkedData(path) => Some(path),
            Self::RootsOverlap => None,
        }
    }
}

/// The checked and fenced plan for one run: with `delete_data` every data-root entry, otherwise
/// the entries of [`REMOVED_DATA`], and every store entry with the pointer last, the installer's
/// own files among them when `installer` says this run removes them. `store` and `data` say what
/// each root holds; `presence` answers what one path holds without following a link.
pub(super) fn plan(
    roots: &Roots,
    delete_data: bool,
    installer: Installer,
    store: &Contents,
    data: &Contents,
    presence: impl Fn(&Path) -> Presence,
) -> Result<Plan, Refusal> {
    fence(roots, [], &presence)?;
    let store_names = names(&roots.store, store, &presence, Refusal::UnmarkedStore)?;
    let data_names = names(&roots.data, data, &presence, Refusal::UnmarkedData)?;
    let target = |path: PathBuf, last| Target {
        presence: presence(&path),
        path,
        last,
    };

    let pointer = target(roots.store.join(STORE_MARK), None);
    let store_marked = store.marked && pointer.presence == Presence::File;
    // The installer's files are its own only in a store root this environment marked; in any other
    // folder those names are another program's.
    let installer_files: &[&str] = if store_marked { installer.files } else { &[] };
    let mut stages: Vec<&String> = store_names.iter().filter(|name| stage(name)).collect();
    stages.sort();
    let store_targets: Vec<Target> = STORE_ENTRIES
        .iter()
        .map(|name| target(roots.store.join(name), None))
        .chain(
            stages
                .into_iter()
                .map(|name| target(roots.store.join(name), None)),
        )
        .chain(
            installer_files
                .iter()
                .filter(|_| installer.remove)
                .map(|name| target(roots.store.join(name), None)),
        )
        .collect();
    let store_plan = RootPlan {
        root: roots.store.clone(),
        targets: store_targets,
        mark: Some(pointer),
        remove_root: store_marked,
        kept: store_names
            .iter()
            .filter(|name| {
                !stage(name)
                    && !named(name, &STORE_ENTRIES)
                    && !named(name, installer_files)
                    && !same(name, STORE_MARK)
            })
            .count(),
    };

    let data_targets: Vec<Target> = if delete_data {
        DATA_ENTRIES
            .iter()
            .map(|name| {
                let last = (*name == DATA_MARK[0]).then_some(DATA_MARK[1]);
                target(roots.data.join(name), last)
            })
            .collect()
    } else {
        REMOVED_DATA
            .iter()
            .map(|parts| {
                target(
                    parts
                        .iter()
                        .fold(roots.data.clone(), |path, part| path.join(part)),
                    None,
                )
            })
            .collect()
    };
    let record = DATA_MARK
        .iter()
        .fold(roots.data.clone(), |path, part| path.join(part));
    let data_marked = data.marked
        && presence(&roots.data.join(DATA_MARK[0])) == Presence::Directory
        && presence(&record) == Presence::File;
    let data_plan = RootPlan {
        root: roots.data.clone(),
        targets: data_targets,
        mark: None,
        // A root missing now is one this run creates only to record the request; it goes again.
        remove_root: (delete_data && data_marked) || presence(&roots.data) == Presence::Missing,
        kept: data_names
            .iter()
            .filter(|name| !named(name, &DATA_ENTRIES))
            .count(),
    };

    // A mark removed last is a target too: nothing reaches it through a link either.
    let lasts: Vec<Target> = data_plan
        .targets
        .iter()
        .filter_map(|t| t.last.map(|last| target(t.path.join(last), None)))
        .collect();
    fence(
        roots,
        store_plan
            .targets
            .iter()
            .chain(&store_plan.mark)
            .chain(&data_plan.targets)
            .chain(&lasts),
        &presence,
    )?;
    // Ownership: nothing is removed from a root that lacks its mark.
    let holds = |part: &RootPlan| {
        part.targets
            .iter()
            .chain(&part.mark)
            .any(|target| target.presence != Presence::Missing)
    };
    if holds(&store_plan) && !store_marked {
        return Err(Refusal::UnmarkedStore(roots.store.clone()));
    }
    if holds(&data_plan) && !data_marked {
        return Err(Refusal::UnmarkedData(roots.data.clone()));
    }
    Ok(Plan {
        data: data_plan,
        store: store_plan,
    })
}

/// The names in one root: none when it is missing; a root that is a link or cannot be listed is
/// refused as on the way to its entries, and a root that is a file as unmarked.
fn names(
    root: &Path,
    contents: &Contents,
    presence: &impl Fn(&Path) -> Presence,
    unmarked: fn(PathBuf) -> Refusal,
) -> Result<Vec<String>, Refusal> {
    match presence(root) {
        Presence::Missing => Ok(Vec::new()),
        Presence::Directory => contents
            .names
            .clone()
            .ok_or_else(|| Refusal::ThroughLink(root.to_owned())),
        Presence::File => Err(unmarked(root.to_owned())),
        Presence::Link | Presence::Unreadable => Err(Refusal::ThroughLink(root.to_owned())),
    }
}

/// Whether `name` is a publication's staging folder: `.stage-<digits>-<digits>`.
fn stage(name: &str) -> bool {
    name.strip_prefix(STAGE_PREFIX)
        .and_then(|rest| rest.split_once('-'))
        .is_some_and(|(process, number)| {
            [process, number]
                .iter()
                .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
        })
}

fn named(name: &str, entries: &[&str]) -> bool {
    entries.iter().any(|entry| same(name, entry))
}

fn same(left: &str, right: &str) -> bool {
    fold(left) == fold(right)
}

/// Admits every target or refuses the whole set; see the module documentation.
pub(super) fn fence<'a>(
    roots: &Roots,
    targets: impl IntoIterator<Item = &'a Target>,
    presence: &impl Fn(&Path) -> Presence,
) -> Result<(), Refusal> {
    let unsafe_root = |root: &PathBuf| Refusal::UnsafeRoot(root.clone());
    let store = key(&roots.store).ok_or_else(|| unsafe_root(&roots.store))?;
    let data = key(&roots.data).ok_or_else(|| unsafe_root(&roots.data))?;
    for (root, root_key) in [(&roots.store, &store), (&roots.data, &data)] {
        // The first part is the volume; a root needs at least two folders below it.
        if root_key.len() < 3
            || roots
                .protected
                .iter()
                .filter_map(|folder| key(folder))
                .any(|folder| starts_with(&folder, root_key))
        {
            return Err(unsafe_root(root));
        }
    }
    if starts_with(&store, &data) || starts_with(&data, &store) {
        return Err(Refusal::RootsOverlap);
    }
    for target in targets {
        let path = key(&target.path).ok_or_else(|| Refusal::OutsideRoots(target.path.clone()))?;
        if [&store, &data]
            .iter()
            .any(|root| path.len() <= root.len() && starts_with(root, &path))
        {
            return Err(Refusal::RootParent(target.path.clone()));
        }
        let Some(root) = [&store, &data]
            .into_iter()
            .find(|root| starts_with(&path, root))
        else {
            return Err(Refusal::OutsideRoots(target.path.clone()));
        };
        // Every folder from the root down to the target's parent must be a real folder.
        for folder in target.path.ancestors().skip(1) {
            if !key(folder).is_some_and(|folder| starts_with(&folder, root)) {
                break;
            }
            if matches!(presence(folder), Presence::Link | Presence::Unreadable) {
                return Err(Refusal::ThroughLink(folder.to_owned()));
            }
        }
    }
    Ok(())
}

/// The comparison form of an absolute path: its volume, then its folder names, with letter case
/// folded on Windows. `None` for a relative path or one that spells `.` or `..`.
fn key(path: &Path) -> Option<Vec<String>> {
    if !path.is_absolute() {
        return None;
    }
    let mut key = vec![String::new()];
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => key[0] = fold(&prefix.as_os_str().to_string_lossy()),
            Component::RootDir => {}
            Component::Normal(part) => key.push(fold(&part.to_string_lossy())),
            Component::CurDir | Component::ParentDir => return None,
        }
    }
    Some(key)
}

fn fold(text: &str) -> String {
    if cfg!(windows) {
        text.to_lowercase()
    } else {
        text.to_owned()
    }
}

/// Whether `path` is `base` or below it.
fn starts_with(path: &[String], base: &[String]) -> bool {
    path.len() >= base.len() && path.iter().zip(base).all(|(left, right)| left == right)
}
