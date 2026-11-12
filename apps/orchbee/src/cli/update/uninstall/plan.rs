//! The uninstall plan: every folder one run removes, computed as a value from the two resolved roots
//! and what exists before anything is removed, then fenced. The fence refuses the whole run when a
//! root is unsafe to remove, when a target is not one of the roots or strictly inside one, when a
//! target is a parent of a root, or when a target is reached through a link or junction. A target
//! that is itself a link stays admitted: removal never follows a link and removes the link alone.
use std::path::{Component, Path, PathBuf};

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

/// The two roots one environment owns and the folders no root may contain or equal.
pub(super) struct Roots {
    pub(super) store: PathBuf,
    pub(super) data: PathBuf,
    /// The person's home, the profile's application-data folders, the system folders and the
    /// temporary folder: removing a root that is one of them, or holds one, would remove what
    /// Orchbee does not own.
    pub(super) protected: Vec<PathBuf>,
}

/// One folder or file the run removes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Target {
    pub(super) path: PathBuf,
    pub(super) presence: Presence,
}

/// The data targets in removal order and the store root, which goes last. A data target that is
/// missing is kept so that the report can say it was already absent.
#[derive(Debug, Eq, PartialEq)]
pub(super) struct Plan {
    pub(super) data: Vec<Target>,
    pub(super) store: Target,
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
    /// A target is a parent of a root.
    RootParent(PathBuf),
    /// A target is reached through a link or junction, or through a path that could not be read.
    ThroughLink(PathBuf),
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
        }
    }

    /// The path the refusal names, when it names one.
    pub(super) fn path(&self) -> Option<&Path> {
        match self {
            Self::UnsafeRoot(path)
            | Self::OutsideRoots(path)
            | Self::RootParent(path)
            | Self::ThroughLink(path) => Some(path),
            Self::RootsOverlap => None,
        }
    }
}

/// The fenced plan for one run: with `delete_data` the whole data root, otherwise the entries of
/// [`REMOVED_DATA`], and the store root last. `presence` answers what exists without following a
/// link; nothing else is read.
pub(super) fn plan(
    roots: &Roots,
    delete_data: bool,
    presence: impl Fn(&Path) -> Presence,
) -> Result<Plan, Refusal> {
    let data_paths = if delete_data {
        vec![roots.data.clone()]
    } else {
        REMOVED_DATA
            .iter()
            .map(|parts| {
                parts
                    .iter()
                    .fold(roots.data.clone(), |path, part| path.join(part))
            })
            .collect()
    };
    let plan = Plan {
        data: data_paths
            .into_iter()
            .map(|path| Target {
                presence: presence(&path),
                path,
            })
            .collect(),
        store: Target {
            presence: presence(&roots.store),
            path: roots.store.clone(),
        },
    };
    fence(roots, plan.data.iter().chain([&plan.store]), &presence)?;
    Ok(plan)
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
            .any(|root| path.len() < root.len() && starts_with(root, &path))
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
