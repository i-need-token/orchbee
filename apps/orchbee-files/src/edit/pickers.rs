//! Picker sessions, which let a Save As or clone dialog browse the whole machine. `picker.open`
//! opens one under the session the Client chose and answers the home directory and the filesystem
//! roots, `picker.list` pages any directory's entries as `fs.list` pages them, without loading the
//! directory for change delivery, and `picker.close`, ten minutes without a request or the
//! connection's end closes it. A connection holds at most 16, the least recently used giving way
//! to a new one. A session serves only requests that name the Workspace it opened in.
//!
//! An open session is also the picker grant, which [`Service::picked`](super::Service::picked)
//! checks.

use std::{
    collections::HashMap,
    sync::{Mutex, MutexGuard, PoisonError},
    time::{Duration, Instant},
};

use orchbee_files_protocol::{
    ConnectionId, ErrorCode, FilePath, FilesError, List, PickerOpenResult, SessionId, WorkspaceId,
};

use crate::blocking::Helpers;

/// How long a session lives without a request.
const IDLE: Duration = Duration::from_secs(10 * 60);
/// Sessions one connection holds at once.
const MAXIMUM_PICKERS: usize = 16;

/// The open picker sessions of every connection.
pub(super) struct Pickers {
    open: Mutex<HashMap<(ConnectionId, SessionId), Picker>>,
}

struct Picker {
    workspace: WorkspaceId,
    touched: Instant,
}

impl Pickers {
    pub(super) fn new() -> Self {
        Self {
            open: Mutex::new(HashMap::new()),
        }
    }

    /// `picker.open` of `picker` for `connection` in `workspace`.
    ///
    /// # Errors
    ///
    /// `invalid-request` when the connection holds an open session under that name.
    pub(super) fn open(
        &self,
        connection: ConnectionId,
        workspace: WorkspaceId,
        picker: SessionId,
    ) -> Result<(), FilesError> {
        let now = Instant::now();
        let mut open = self.lock();
        open.retain(|_, held| now.duration_since(held.touched) < IDLE);
        if open.contains_key(&(connection, picker)) {
            return Err(FilesError::new(ErrorCode::InvalidRequest));
        }
        let held = open.iter().filter(|(key, _)| key.0 == connection);
        if held.clone().count() >= MAXIMUM_PICKERS {
            let oldest = held
                .min_by_key(|(_, held)| held.touched)
                .map(|(key, _)| *key);
            if let Some(oldest) = oldest {
                open.remove(&oldest);
            }
        }
        open.insert(
            (connection, picker),
            Picker {
                workspace,
                touched: now,
            },
        );
        Ok(())
    }

    /// Whether `connection` holds `picker` open in `workspace`, which this use keeps open for ten
    /// more minutes.
    pub(super) fn holds(
        &self,
        connection: ConnectionId,
        workspace: WorkspaceId,
        picker: SessionId,
    ) -> bool {
        let now = Instant::now();
        let mut open = self.lock();
        let Some(held) = open.get_mut(&(connection, picker)) else {
            return false;
        };
        if now.duration_since(held.touched) >= IDLE {
            open.remove(&(connection, picker));
            return false;
        }
        if held.workspace != workspace {
            return false;
        }
        held.touched = now;
        true
    }

    /// `picker.close`; a session that is not open is closed already.
    pub(super) fn close(&self, connection: ConnectionId, picker: SessionId) {
        self.lock().remove(&(connection, picker));
    }

    /// Closes every session of `connection`.
    pub(super) fn connection_lost(&self, connection: ConnectionId) {
        self.lock().retain(|key, _| key.0 != connection);
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<(ConnectionId, SessionId), Picker>> {
        self.open.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Where a picker opens: the home directory, and the root of each filesystem.
///
/// # Errors
///
/// `failed` without a home directory a record carries, and the helpers' refusal when no root
/// answers.
pub(super) fn places(helpers: &Helpers) -> Result<PickerOpenResult, FilesError> {
    let failed = || FilesError::new(ErrorCode::Failed);
    let home = std::env::home_dir()
        .and_then(|home| home.into_os_string().into_string().ok())
        .and_then(|home| FilePath::new(home).ok())
        .ok_or_else(failed)?;
    let roots = List::new(roots(helpers)?).map_err(|_| failed())?;
    Ok(PickerOpenResult { home, roots })
}

#[cfg(unix)]
fn roots(_helpers: &Helpers) -> Result<Vec<FilePath>, FilesError> {
    FilePath::new("/")
        .map(|root| vec![root])
        .map_err(|_| FilesError::new(ErrorCode::Failed))
}

/// The root of each drive letter the system holds now. A probe opens the drive's volume with no
/// access, which asks nothing of its media, so a drive without a disk raises no dialog. The probes
/// run at once, each on a helper, so drives that do not answer cost one deadline together.
#[cfg(windows)]
fn roots(helpers: &Helpers) -> Result<Vec<FilePath>, FilesError> {
    use std::os::windows::fs::OpenOptionsExt as _;

    let probe = |letter: char| {
        let present = helpers.run(orchbee_files_protocol::Operation::PickerOpen, move || {
            let volume = format!(r"\\.\{letter}:");
            std::fs::OpenOptions::new()
                .access_mode(0)
                .open(volume)
                .is_ok()
        });
        present.map(|present| present.then(|| format!(r"{letter}:\")))
    };
    let probed: Vec<_> = std::thread::scope(|scope| {
        let probes: Vec<_> = ('A'..='Z')
            .map(|letter| {
                std::thread::Builder::new()
                    .name("orchbee-files-call".into())
                    .spawn_scoped(scope, move || probe(letter))
            })
            .collect();
        probes
            .into_iter()
            .map(|probe| {
                let joined = probe.ok().and_then(|probe| probe.join().ok());
                joined.unwrap_or_else(|| Err(FilesError::new(ErrorCode::Failed)))
            })
            .collect()
    });
    let mut roots = Vec::new();
    let mut refused = None;
    for probe in probed {
        match probe {
            Ok(root) => roots.extend(root.and_then(|root| FilePath::new(root).ok())),
            Err(error) => refused = Some(error),
        }
    }
    match refused {
        Some(error) if roots.is_empty() => Err(error),
        _ => Ok(roots),
    }
}
