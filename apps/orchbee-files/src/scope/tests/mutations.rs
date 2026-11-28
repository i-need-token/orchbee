//! Seeded evidence that the mutation check admits a path exactly when the location its change
//! reaches lies inside a member's, over generated filesystems of folders, files, links (junctions
//! on Windows), links that name nothing, link cycles, names that differ only in case and, on
//! Windows, 8.3 names. The check reads real locations as `canonicalize` reads them and walks up
//! from a missing one; the reference resolves each segment directly and spells what is missing as
//! requested. `ORCHBEE_FILES_SCOPE_SEED` explores another seed; a failure prints its seed, case,
//! request and filesystem.

use std::{
    collections::HashMap,
    io,
    path::{Path, PathBuf},
    sync::Arc,
};

use orchbee_files_protocol::{ConnectionId, ErrorCode, FilePath, WorkspaceId};

use crate::seeded::{self, SplitMix64};

use super::super::{
    Change, Filesystem, View,
    path::{self, NativePath},
};

const DEFAULT_SEED: u64 = 0x5c0e_0000_0000_1001;
const CASES: u64 = 400;
const REQUESTS: usize = 24;
/// The links one resolution follows before it reports a cycle, as an operating system gives up.
const HOPS: usize = 16;

/// The names entries are created with. On a machine that ignores case, `A` is refused beside `a`.
#[cfg(windows)]
const CREATED: &[&str] = &["a", "b", "A", "longname"];
#[cfg(not(windows))]
const CREATED: &[&str] = &["a", "b", "A", "c"];

/// The names requests spell, `longname`'s 8.3 alias among them on Windows.
#[cfg(windows)]
const SPELLED: &[&str] = &["a", "b", "A", "longname", "LONGNA~1"];
#[cfg(not(windows))]
const SPELLED: &[&str] = &["a", "b", "A", "c"];

#[test]
fn a_mutation_is_admitted_exactly_when_the_location_it_changes_lies_in_a_member() {
    let seed = seeded::seed("ORCHBEE_FILES_SCOPE_SEED", DEFAULT_SEED);
    for case in 0..CASES {
        let mut random = SplitMix64::new(seed ^ case);
        let model = generated(&mut random);
        let members = (0..=random.below(2))
            .map(|_| spelled(&mut random, 2))
            .collect::<Vec<_>>();
        let opened = (0..random.below(3))
            .map(|_| spelled(&mut random, 3))
            .collect::<Vec<_>>();
        let view = View {
            workspace: WorkspaceId::from_bytes([1; 16]).unwrap(),
            connection: ConnectionId::new(1).unwrap(),
            members: members.clone(),
            opened: Arc::new(
                opened
                    .iter()
                    .map(|path| (path.key(), (path.clone(), 1)))
                    .collect(),
            ),
        };
        for _ in 0..REQUESTS {
            let path = spelled(&mut random, 4);
            let change = if random.below(2) == 0 {
                Change::Content
            } else {
                Change::Entry
            };
            let context = format!(
                "seed {seed:#x} case {case}: {change:?} {} with members {members:?}, opened \
                 {opened:?}, in {model:#?}",
                path.as_str()
            );
            let admitted = view.mutable_on(&model, &FilePath::new(path.as_str()).unwrap(), change);
            assert_eq!(
                admitted.as_ref().err().map(|error| error.code),
                expected(&model, &members, &opened, &path, change),
                "{context}"
            );
            if let Ok(admitted) = admitted {
                assert_eq!(admitted, path, "{context}");
            }
        }
    }
}

/// What the check answers by the rule: an opened file's content anywhere; otherwise the changed
/// location, the path for content and its folder for an entry, lexically inside a member, and the
/// location it reaches inside a member's.
fn expected(
    model: &Model,
    members: &[NativePath],
    opened: &[NativePath],
    path: &NativePath,
    change: Change,
) -> Option<ErrorCode> {
    if change == Change::Content && opened.iter().any(|file| file.key() == path.key()) {
        return None;
    }
    let changed = match change {
        Change::Content => Some(path.clone()),
        Change::Entry => path.parent(),
    };
    let Some(changed) =
        changed.filter(|changed| members.iter().any(|member| changed.is_within(member)))
    else {
        return Some(ErrorCode::OutsideScope);
    };
    let reached = match model.reached(&changed) {
        Ok(reached) => reached,
        Err(code) => return Some(code),
    };
    let inside = members
        .iter()
        .filter_map(|member| model.reached(member).ok())
        .any(|member| path::real_within(&reached, &member));
    (!inside).then_some(ErrorCode::LinkTargetOutside)
}

/// A modelled filesystem: each folder's entries by the folder's real segments.
#[derive(Debug, Default)]
struct Model {
    children: HashMap<Vec<String>, Vec<Entry>>,
    /// Every folder in creation order, so generation picks one deterministically.
    folders: Vec<Vec<String>>,
}

#[derive(Debug)]
struct Entry {
    name: &'static str,
    node: Node,
}

#[derive(Debug)]
enum Node {
    Folder,
    File,
    /// A link, or on Windows a junction, naming an absolute path.
    Link(String),
}

/// Why a walk stopped.
enum Failure {
    NotFound,
    NotDirectory,
    Cycle,
}

impl Model {
    fn add(&mut self, folder: &[String], name: &'static str, node: Node) {
        if self.child(folder, name).is_some() {
            return;
        }
        if matches!(node, Node::Folder) {
            let mut inner = folder.to_vec();
            inner.push(name.to_owned());
            self.folders.push(inner);
        }
        self.children
            .entry(folder.to_vec())
            .or_default()
            .push(Entry { name, node });
    }

    fn child(&self, folder: &[String], requested: &str) -> Option<&Entry> {
        self.children
            .get(folder)?
            .iter()
            .find(|entry| names(entry.name, requested))
    }

    /// The real segments `segments` lead to, following each link on the way, and the last
    /// segment's too when `follow_last`. Where nothing is, `append` spells the rest as requested;
    /// otherwise the walk fails there. Whatever a link names must be there.
    fn walk(
        &self,
        segments: &[&str],
        follow_last: bool,
        append: bool,
        hops: &mut usize,
    ) -> Result<Vec<String>, Failure> {
        let mut real = Vec::new();
        for (index, segment) in segments.iter().enumerate() {
            let last = index + 1 == segments.len();
            let Some(entry) = self.child(&real, segment) else {
                if append {
                    real.extend(segments[index..].iter().map(|&segment| segment.to_owned()));
                    return Ok(real);
                }
                return Err(Failure::NotFound);
            };
            match &entry.node {
                Node::Link(target) if !last || follow_last => {
                    *hops += 1;
                    if *hops > HOPS {
                        return Err(Failure::Cycle);
                    }
                    real = self.walk(&segments_of(target), true, false, hops)?;
                    if !last && self.is_file(&real) {
                        return Err(Failure::NotDirectory);
                    }
                }
                Node::File if !last => return Err(Failure::NotDirectory),
                Node::Folder | Node::File | Node::Link(_) => real.push(entry.name.to_owned()),
            }
        }
        Ok(real)
    }

    fn is_file(&self, real: &[String]) -> bool {
        real.split_last().is_some_and(|(name, folder)| {
            self.child(folder, name)
                .is_some_and(|entry| matches!(entry.node, Node::File))
        })
    }

    /// The location a change of `path` reaches, by the rule.
    fn reached(&self, path: &NativePath) -> Result<PathBuf, ErrorCode> {
        match self.walk(&segments_of(path.as_str()), true, true, &mut 0) {
            Ok(real) => Ok(canonical(&real)),
            // What is missing is spelled as requested, so only a link can name nothing.
            Err(Failure::NotFound) => Err(ErrorCode::LinkTargetOutside),
            Err(Failure::NotDirectory) => Err(ErrorCode::NotDirectory),
            Err(Failure::Cycle) => Err(ErrorCode::Failed),
        }
    }
}

impl Filesystem for Model {
    fn real(&self, path: &Path) -> io::Result<PathBuf> {
        let segments = segments_of(path.to_str().unwrap());
        self.walk(&segments, true, false, &mut 0)
            .map(|real| canonical(&real))
            .map_err(failed)
    }

    fn exists(&self, path: &Path) -> io::Result<bool> {
        let segments = segments_of(path.to_str().unwrap());
        match self.walk(&segments, false, false, &mut 0) {
            Ok(_) => Ok(true),
            Err(Failure::NotFound) => Ok(false),
            Err(failure) => Err(failed(failure)),
        }
    }
}

fn failed(failure: Failure) -> io::Error {
    match failure {
        Failure::NotFound => io::ErrorKind::NotFound.into(),
        Failure::NotDirectory => io::ErrorKind::NotADirectory.into(),
        Failure::Cycle => io::Error::other("too many links"),
    }
}

/// Whether a stored name answers to a requested one: by the machine's comparison of names, and on
/// Windows also by its 8.3 alias.
fn names(stored: &str, requested: &str) -> bool {
    let key = |name: &str| {
        if path::CASE_SENSITIVE {
            name.to_owned()
        } else {
            name.to_lowercase()
        }
    };
    key(stored) == key(requested)
        || (cfg!(windows) && stored == "longname" && key(requested) == key("LONGNA~1"))
}

/// The segments of a native absolute path below its root.
fn segments_of(path: &str) -> Vec<&str> {
    let below = if cfg!(windows) {
        &path[3..]
    } else {
        &path[1..]
    };
    below
        .split(std::path::MAIN_SEPARATOR)
        .filter(|segment| !segment.is_empty())
        .collect()
}

/// Real segments as `canonicalize` spells them.
fn canonical(real: &[String]) -> PathBuf {
    let mut path = PathBuf::from(if cfg!(windows) { r"\\?\C:\" } else { "/" });
    path.extend(real);
    path
}

/// A filesystem of a Workspace area `w` and an area `x` outside it, holding a few folders, files
/// and links to anywhere in either, the filesystem root included.
fn generated(random: &mut SplitMix64) -> Model {
    let mut model = Model::default();
    model.folders.push(Vec::new());
    model.add(&[], "w", Node::Folder);
    model.add(&[], "x", Node::Folder);
    for _ in 0..4 + random.below(14) {
        let folder = model.folders[random.below(model.folders.len())].clone();
        let name = CREATED[random.below(CREATED.len())];
        let node = match random.below(10) {
            0..=3 => Node::Folder,
            4 | 5 => Node::File,
            _ => Node::Link(spelled(random, 3).as_str().to_owned()),
        };
        model.add(&folder, name, node);
    }
    model
}

/// An absolute path in `w` or `x` of at most `depth` more segments, and now and then the
/// filesystem root itself.
fn spelled(random: &mut SplitMix64, depth: usize) -> NativePath {
    let root = if cfg!(windows) { r"C:\" } else { "/" };
    if random.below(16) == 0 {
        return NativePath::parse(root).unwrap();
    }
    let mut segments = vec![if random.below(4) == 0 { "x" } else { "w" }];
    for _ in 0..random.below(depth + 1) {
        segments.push(SPELLED[random.below(SPELLED.len())]);
    }
    NativePath::parse(&format!(
        "{root}{}",
        segments.join(std::path::MAIN_SEPARATOR_STR)
    ))
    .unwrap()
}
