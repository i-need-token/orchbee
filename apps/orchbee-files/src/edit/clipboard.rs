//! Each connection's clipboard sources, which `fs.clipboard` replaces: the items a Client copied
//! or cut, each in the boundary of the Workspace the request named, which a paste may then read
//! and copy, and move when they were cut, after that root moves and from any Workspace of the
//! connection. The next `fs.clipboard` or the connection's end releases them.

use std::{
    collections::HashMap,
    sync::{Mutex, MutexGuard, PoisonError},
};

use orchbee_files_protocol::{ConnectionId, FilePath};

use crate::scope::path::NativePath;

/// The clipboard sources of every connection.
pub(super) struct Clipboards {
    held: Mutex<HashMap<ConnectionId, Held>>,
}

struct Held {
    paths: Vec<NativePath>,
    cut: bool,
}

impl Clipboards {
    pub(super) fn new() -> Self {
        Self {
            held: Mutex::new(HashMap::new()),
        }
    }

    /// Makes `paths`, which their Workspace's boundary admitted, `connection`'s clipboard
    /// sources; none clears them.
    pub(super) fn replace(&self, connection: ConnectionId, paths: Vec<NativePath>, cut: bool) {
        let mut held = self.lock();
        if paths.is_empty() {
            held.remove(&connection);
        } else {
            held.insert(connection, Held { paths, cut });
        }
    }

    /// `path` when it is one of `connection`'s clipboard sources, which a paste may read and copy.
    pub(super) fn copied(&self, connection: ConnectionId, path: &FilePath) -> Option<NativePath> {
        self.find(connection, path, false)
    }

    /// `path` when it is one of `connection`'s clipboard sources and they were cut, so a paste may
    /// move it.
    pub(super) fn cut(&self, connection: ConnectionId, path: &FilePath) -> Option<NativePath> {
        self.find(connection, path, true)
    }

    /// Releases `connection`'s clipboard sources.
    pub(super) fn connection_lost(&self, connection: ConnectionId) {
        self.lock().remove(&connection);
    }

    fn find(&self, connection: ConnectionId, path: &FilePath, cut: bool) -> Option<NativePath> {
        let path = NativePath::parse(path.as_str()).ok()?;
        let held = self.lock();
        let sources = held.get(&connection).filter(|held| held.cut || !cut)?;
        let key = path.key();
        sources
            .paths
            .iter()
            .any(|source| source.key() == key)
            .then_some(path)
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<ConnectionId, Held>> {
        self.held.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
