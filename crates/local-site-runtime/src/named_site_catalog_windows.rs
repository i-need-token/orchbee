//! Cooperating native Windows activators and stopped-session deletion share one namespace owner.
use crate::{
    ActivationState, LockAttempt, ModeCheckedLocalSiteRendezvous as Rendezvous, SiteScope,
    WindowsSiteName, site_name_windows, windows_permissions as permissions,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, File, TryLockError},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    time::Instant,
};

const MAXIMUM_SESSIONS: usize = 256;
const INTENT_SUFFIX: &str = ".deleting";
const RUNTIME_FILES: &[&str] = &[
    "session-name.json",
    "activation.started",
    "activation.lock",
    "site-lifetime.lock",
];
const CHECKPOINT_FILES: &[&str] = &["checkpoint.json", "checkpoint.pending", "state.lock"];

pub struct WindowsNamedSiteCatalog {
    base: PathBuf,
    _lock: File,
    _directories: Vec<File>,
}

#[derive(Debug)]
pub struct WindowsNamedSiteEntry {
    pub name: WindowsSiteName,
    pub runtime: PathBuf,
    pub deleting: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowsNamedSiteDeleteOutcome {
    Absent,
    Deleted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowsNamedSiteDeleteReason {
    DefaultSession,
    InUse,
    RecoveryRequired,
    Filesystem,
    TimedOut,
    Interrupted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WindowsNamedSiteDeleteFailure {
    pub reason: WindowsNamedSiteDeleteReason,
    pub pending: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Intent {
    version: u8,
    name: String,
    stopped_request_id: Option<[u8; 16]>,
}

fn scope_matches(scope: &str) -> bool {
    scope.len() == 63
        && scope
            .strip_prefix(site_name_windows::NAMED_PREFIX)
            .is_some_and(|hash| {
                hash.bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            })
}

fn intent_path(base: &Path, scope: &str) -> PathBuf {
    base.join(format!("{scope}{INTENT_SUFFIX}"))
}

fn read_intent(base: &Path, scope: &str) -> io::Result<Option<Intent>> {
    let file = match permissions::open_file(&intent_path(base, scope), false, false) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let mut bytes = Vec::new();
    file.take(1025).read_to_end(&mut bytes)?;
    let intent: Intent = serde_json::from_slice(&bytes).map_err(|_| io::ErrorKind::InvalidData)?;
    let name = WindowsSiteName::parse(&intent.name).map_err(|_| io::ErrorKind::InvalidData)?;
    if bytes.len() > 1024
        || intent.version != 1
        || name.as_str() == "default"
        || intent.stopped_request_id == Some([0; 16])
        || name.scope_for_user(&orchbee_local_ipc::current_user_sid()?) != scope
    {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(Some(intent))
}

pub(crate) fn require_not_deleting(base: &Path, scope: &str) -> io::Result<()> {
    if scope_matches(scope) && read_intent(base, scope)?.is_some() {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "named session deletion is pending",
        ));
    }
    Ok(())
}

impl WindowsNamedSiteCatalog {
    /// Only declared activation/deletion effects acquire this owner. Reads use `list` instead.
    pub fn try_lock(base: &Path) -> io::Result<Self> {
        let directories = permissions::open_tree(base)?;
        let base = base.canonicalize()?;
        let user = WindowsSiteName::parse("default")?
            .scope_for_user(&orchbee_local_ipc::current_user_sid()?);
        let path = base.join(format!("{user}.sessions.lock"));
        let lock = match permissions::open_file(&path, true, true) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                permissions::open_file(&path, false, true)?
            }
            Err(error) => return Err(error),
        };
        match lock.try_lock() {
            Ok(()) => Ok(Self {
                base,
                _lock: lock,
                _directories: directories,
            }),
            Err(TryLockError::WouldBlock) => Err(io::ErrorKind::WouldBlock.into()),
            Err(TryLockError::Error(error)) => Err(error),
        }
    }

    pub fn list(base: &Path, deadline: Instant) -> io::Result<Vec<WindowsNamedSiteEntry>> {
        let _directories = permissions::open_tree(base)?;
        let base = base.canonicalize()?;
        let sid = orchbee_local_ipc::current_user_sid()?;
        let mut names = BTreeMap::new();
        for entry in fs::read_dir(&base)? {
            check_deadline(deadline)?;
            let entry = entry?;
            let Some(leaf) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let (name, deleting) = if scope_matches(&leaf) {
                let _pins = permissions::open_tree(&entry.path())?;
                match read_intent(&base, &leaf)? {
                    Some(intent) => (WindowsSiteName::parse(&intent.name)?, true),
                    None => (site_name_windows::read(&entry.path())?, false),
                }
            } else if let Some(scope) = leaf
                .strip_suffix(INTENT_SUFFIX)
                .filter(|scope| scope_matches(scope))
            {
                let intent = read_intent(&base, scope)?.ok_or(io::ErrorKind::NotFound)?;
                (WindowsSiteName::parse(&intent.name)?, true)
            } else {
                continue;
            };
            names
                .entry(name.as_str().to_owned())
                .and_modify(|pending| *pending |= deleting)
                .or_insert(deleting);
            if names.len() >= MAXIMUM_SESSIONS {
                return Err(io::ErrorKind::FileTooLarge.into());
            }
        }
        let default = WindowsSiteName::parse("default")?;
        let mut result = vec![WindowsNamedSiteEntry {
            runtime: base.join(default.scope_for_user(&sid)),
            name: default,
            deleting: false,
        }];
        for (name, deleting) in names {
            let name = WindowsSiteName::parse(&name)?;
            result.push(WindowsNamedSiteEntry {
                runtime: base.join(name.scope_for_user(&sid)),
                name,
                deleting,
            });
        }
        Ok(result)
    }

    pub fn delete(
        &self,
        name: &WindowsSiteName,
        local_app_data: &Path,
        deadline: Instant,
        mut interrupted: impl FnMut() -> io::Result<bool>,
    ) -> Result<WindowsNamedSiteDeleteOutcome, WindowsNamedSiteDeleteFailure> {
        use WindowsNamedSiteDeleteReason as Reason;
        let failure = |reason, pending| WindowsNamedSiteDeleteFailure { reason, pending };
        if name.as_str() == "default" {
            return Err(failure(Reason::DefaultSession, false));
        }
        let mut check = || {
            if interrupted()? {
                Err(io::ErrorKind::Interrupted.into())
            } else {
                check_deadline(deadline)
            }
        };
        let mut pending = false;
        let mut refused = None;
        let result = (|| -> io::Result<WindowsNamedSiteDeleteOutcome> {
            let scope = name.scope_for_user(&orchbee_local_ipc::current_user_sid()?);
            let runtime = self.base.join(&scope);
            let previous = read_intent(&self.base, &scope)?;
            pending = previous.is_some();
            check()?;
            let _app_data = permissions::open_tree(local_app_data)?;
            let mut site = match Rendezvous::for_named_deletion(
                &self.base,
                SiteScope::parse(&scope).map_err(io::Error::other)?,
                pending,
            ) {
                Ok(site) => Some(site),
                Err(crate::RendezvousError::System { source, .. })
                    if source.kind() == io::ErrorKind::NotFound =>
                {
                    None
                }
                Err(error) => return Err(io::Error::other(error)),
            };
            let mut activation = None;
            let mut lifetime = None;
            let mut stopped = previous
                .as_ref()
                .and_then(|intent| intent.stopped_request_id);
            if let Some(site) = &site {
                activation = match site
                    .try_acquire_activation_lock()
                    .map_err(io::Error::other)?
                {
                    LockAttempt::Acquired(lock) => Some(lock),
                    LockAttempt::Contended => {
                        refused = Some(Reason::InUse);
                        return Err(io::ErrorKind::WouldBlock.into());
                    }
                };
                lifetime = match site
                    .try_acquire_site_lifetime_lock()
                    .map_err(io::Error::other)?
                {
                    LockAttempt::Acquired(lock) => Some(lock),
                    LockAttempt::Contended => {
                        refused = Some(Reason::InUse);
                        return Err(io::ErrorKind::WouldBlock.into());
                    }
                };
                match site.activation_state()? {
                    ActivationState::Active => {
                        refused = Some(Reason::RecoveryRequired);
                        return Err(io::ErrorKind::InvalidData.into());
                    }
                    ActivationState::Stopped { request_id } => stopped = Some(request_id),
                    ActivationState::Absent => {}
                }
            }
            for endpoint in Rendezvous::socket_budget_paths(&runtime)
                .into_iter()
                .chain([Rendezvous::observation_endpoint_path(&runtime)])
            {
                if orchbee_local_ipc::endpoint_exists(&endpoint)? {
                    refused = Some(Reason::InUse);
                    return Err(io::ErrorKind::WouldBlock.into());
                }
            }
            let state_path = local_app_data
                .join("Orchbee")
                .join("native-sessions")
                .join(Rendezvous::host_endpoint_id(&runtime));
            let state = OwnedDirectory::open_optional(&state_path, CHECKPOINT_FILES, false)?;
            let runtime_directory = OwnedDirectory::open_optional(&runtime, RUNTIME_FILES, true)?;
            if site.is_none() && runtime_directory.is_some() && !pending {
                return Err(io::ErrorKind::InvalidData.into());
            }
            if site.is_none() && state.is_none() && runtime_directory.is_none() && !pending {
                return Ok(WindowsNamedSiteDeleteOutcome::Absent);
            }
            if site.is_none() && !pending {
                refused = Some(Reason::RecoveryRequired);
                return Err(io::ErrorKind::InvalidData.into());
            }
            let state_lock = match &state {
                Some(state) => state.try_existing_lock("state.lock")?,
                None => None,
            };
            check()?;
            if !pending {
                let intent = Intent {
                    version: 1,
                    name: name.as_str().to_owned(),
                    stopped_request_id: stopped,
                };
                let bytes = serde_json::to_vec(&intent).map_err(io::Error::other)?;
                let staging = self.base.join(format!("{scope}.deleting.pending"));
                remove_private_file_if_present(&staging)?;
                let mut file = permissions::open_file(&staging, true, true)?;
                let written = file.write_all(&bytes).and_then(|()| file.sync_all());
                drop(file);
                if let Err(error) = written {
                    remove_private_file_if_present(&staging)?;
                    return Err(error);
                }
                let target = intent_path(&self.base, &scope);
                if let Err(error) = winsafe::MoveFileEx(
                    staging.to_str().ok_or(io::ErrorKind::InvalidInput)?,
                    Some(target.to_str().ok_or(io::ErrorKind::InvalidInput)?),
                    winsafe::co::MOVEFILE::REPLACE_EXISTING,
                ) {
                    remove_private_file_if_present(&staging)?;
                    return Err(io::Error::other(error));
                }
                pending = true;
            }
            drop(state_lock);
            if let Some(state) = state {
                state.remove(CHECKPOINT_FILES, false, &mut check)?;
            }
            drop(lifetime.take());
            drop(activation.take());
            drop(site.take());
            if let Some(runtime_directory) = runtime_directory {
                runtime_directory.remove(RUNTIME_FILES, true, &mut check)?;
            }
            check()?;
            let intent = permissions::open_file(&intent_path(&self.base, &scope), false, false)?;
            drop(intent);
            fs::remove_file(intent_path(&self.base, &scope))?;
            Ok(WindowsNamedSiteDeleteOutcome::Deleted)
        })();
        result.map_err(|error| {
            failure(
                refused.unwrap_or(if error.kind() == io::ErrorKind::Interrupted {
                    Reason::Interrupted
                } else if error.kind() == io::ErrorKind::TimedOut {
                    Reason::TimedOut
                } else if error.kind() == io::ErrorKind::WouldBlock {
                    Reason::InUse
                } else {
                    Reason::Filesystem
                }),
                pending,
            )
        })
    }
}

fn check_deadline(deadline: Instant) -> io::Result<()> {
    if Instant::now() >= deadline {
        Err(io::ErrorKind::TimedOut.into())
    } else {
        Ok(())
    }
}

fn remove_private_file_if_present(path: &Path) -> io::Result<()> {
    match permissions::open_file(path, false, false) {
        Ok(file) => {
            drop(file);
            fs::remove_file(path)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

struct OwnedDirectory {
    path: PathBuf,
    pins: Vec<File>,
}
impl OwnedDirectory {
    fn open_optional(path: &Path, files: &[&str], host: bool) -> io::Result<Option<Self>> {
        let pins = match permissions::open_tree(path) {
            Ok(pins) => pins,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let leaf = entry.file_name();
            let name = leaf.to_str().ok_or(io::ErrorKind::InvalidData)?;
            if files.contains(&name) {
                permissions::open_file(&entry.path(), false, false)?;
            } else if host && name == "host" {
                let _pins = permissions::open_tree(&entry.path())?;
                if fs::read_dir(entry.path())?.next().is_some() {
                    return Err(io::ErrorKind::InvalidData.into());
                }
            } else {
                return Err(io::ErrorKind::InvalidData.into());
            }
        }
        Ok(Some(Self {
            path: path.to_owned(),
            pins,
        }))
    }

    fn try_existing_lock(&self, name: &str) -> io::Result<Option<File>> {
        let file = match permissions::open_file(&self.path.join(name), false, true) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        match file.try_lock() {
            Ok(()) => Ok(Some(file)),
            Err(TryLockError::WouldBlock) => Err(io::ErrorKind::WouldBlock.into()),
            Err(TryLockError::Error(error)) => Err(error),
        }
    }

    fn remove(
        mut self,
        files: &[&str],
        host: bool,
        check: &mut dyn FnMut() -> io::Result<()>,
    ) -> io::Result<()> {
        for name in files {
            check()?;
            let path = self.path.join(name);
            match permissions::open_file(&path, false, false) {
                Ok(file) => {
                    drop(file);
                    fs::remove_file(path)?;
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
        if host {
            let path = self.path.join("host");
            match permissions::open_tree(&path) {
                Ok(pins) => {
                    drop(pins);
                    fs::remove_dir(path)?;
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
        check()?;
        self.pins.pop();
        fs::remove_dir(&self.path)
    }
}
