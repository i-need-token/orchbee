//! Linux's recursive watches: a walker finds each folder of a tree, leaving out the folders VS
//! Code's default `files.watcherExclude` excludes and following no link, and each folder gets its
//! own inotify watch. A folder appearing in a tree is walked and watched; one vanishing drops the
//! watches at and below it. Every watch of the process counts against one budget: a quarter of
//! `max_user_watches`, at most 131,072, or 2,048 when the limit cannot be read. Watches are held
//! per request, so folders two requests share keep their watch until both end.

use std::{collections::HashMap, fs};

use super::{super::excluded, Need};
use crate::scope::path::NativePath;

/// The process's watches and the requests that hold them.
pub(super) struct Trees {
    budget: usize,
    /// Each watched folder, by [`NativePath::key`], with the number of requests holding it.
    folders: HashMap<String, (NativePath, u32)>,
    /// Each recursive request's folders, by its root's [`NativePath::key`].
    trees: HashMap<String, (NativePath, Vec<NativePath>)>,
}

impl Default for Trees {
    fn default() -> Self {
        let limit = fs::read_to_string("/proc/sys/fs/inotify/max_user_watches")
            .ok()
            .and_then(|limit| limit.trim().parse::<usize>().ok());
        Self {
            budget: limit.map_or(2_048, |limit| (limit / 4).min(131_072)),
            folders: HashMap::new(),
            trees: HashMap::new(),
        }
    }
}

impl Trees {
    /// The watches the process may hold.
    pub(super) const fn budget(&self) -> usize {
        self.budget
    }

    /// The folders of `folders` that no request holds yet.
    pub(super) fn fresh<'a>(&self, folders: &'a [NativePath]) -> Vec<&'a NativePath> {
        folders
            .iter()
            .filter(|folder| !self.folders.contains_key(&folder.key()))
            .collect()
    }

    /// Whether `more` watches fit the budget.
    pub(super) fn fits(&self, more: usize) -> bool {
        self.folders.len() + more <= self.budget
    }

    /// Records that the request at `path` holds `folders`, which are watched.
    pub(super) fn hold(&mut self, path: &NativePath, need: Need, folders: Vec<NativePath>) {
        for folder in &folders {
            self.folders
                .entry(folder.key())
                .or_insert_with(|| (folder.clone(), 0))
                .1 += 1;
        }
        if need == Need::Tree {
            self.trees.insert(path.key(), (path.clone(), folders));
        }
    }

    /// Adds `folders`, which are watched, to the tree whose root is `tree`.
    pub(super) fn extend(&mut self, tree: &NativePath, folders: Vec<NativePath>) {
        let Some((_, held)) = self.trees.get_mut(&tree.key()) else {
            return;
        };
        for folder in folders {
            let key = folder.key();
            if held.iter().any(|already| already.key() == key) {
                continue;
            }
            self.folders
                .entry(key)
                .or_insert_with(|| (folder.clone(), 0))
                .1 += 1;
            held.push(folder);
        }
    }

    /// Ends the request at `path` and returns the folders no request holds any longer.
    pub(super) fn release(&mut self, path: &NativePath, need: Need) -> Vec<NativePath> {
        let folders = match need {
            Need::Tree => self
                .trees
                .remove(&path.key())
                .map(|(_, folders)| folders)
                .unwrap_or_default(),
            Need::Folder => vec![path.clone()],
        };
        self.drop_folders(folders)
    }

    /// The root of the tree that holds `path`.
    pub(super) fn tree_of(&self, path: &NativePath) -> Option<NativePath> {
        self.trees
            .values()
            .find(|(root, _)| path.is_within(root))
            .map(|(root, _)| root.clone())
    }

    /// Drops the folders at and below `path` from every tree, now that it is gone, and returns
    /// those no request holds any longer.
    pub(super) fn vanished(&mut self, path: &NativePath) -> Vec<NativePath> {
        let mut gone = Vec::new();
        for (root, held) in self.trees.values_mut() {
            // A tree's own root ends with its request.
            held.retain(|folder| {
                let below = folder.key() != root.key() && folder.is_within(path);
                if below {
                    gone.push(folder.clone());
                }
                !below
            });
        }
        self.drop_folders(gone)
    }

    fn drop_folders(&mut self, folders: Vec<NativePath>) -> Vec<NativePath> {
        folders
            .into_iter()
            .filter(|folder| {
                let key = folder.key();
                let Some((_, holders)) = self.folders.get_mut(&key) else {
                    return false;
                };
                *holders -= 1;
                if *holders > 0 {
                    return false;
                }
                self.folders.remove(&key);
                true
            })
            .collect()
    }
}

/// The folders of the tree at `root`, root first, without the excluded ones and without
/// following links; `None` when there are more than `limit`. A folder whose name this machine
/// cannot spell, or that cannot be read, is not walked.
pub(super) fn directories(root: &NativePath, limit: usize) -> Option<Vec<NativePath>> {
    let mut found = vec![root.clone()];
    let mut next = 0;
    while let Some(folder) = found.get(next).cloned() {
        next += 1;
        let Ok(entries) = fs::read_dir(folder.as_path()) else {
            continue;
        };
        for entry in entries.flatten() {
            // The entry's own type: a link to a folder is not one.
            if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                continue;
            }
            let Some(child) = entry.file_name().to_str().and_then(|name| {
                let parent = folder.as_str().trim_end_matches('/');
                NativePath::parse(&format!("{parent}/{name}")).ok()
            }) else {
                continue;
            };
            if excluded(root, &child) {
                continue;
            }
            if found.len() == limit {
                return None;
            }
            found.push(child);
        }
    }
    Some(found)
}

#[cfg(test)]
mod tests;
