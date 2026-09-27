//! Private lifecycle records outlive the runtime leaf. Effects derive their paths from admission.
use crate::{
    ActivationState, ModeCheckedLocalSiteRendezvous as Rendezvous, WindowsSiteName,
    site_name_windows, windows_permissions as permissions,
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    time::Instant,
};

const MAXIMUM_BYTES: usize = 8192;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum Lifecycle {
    Active,
    Stopped {
        request_id: [u8; 16],
    },
    Deleting {
        stopped_request_id: Option<[u8; 16]>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Record {
    version: u8,
    pub(crate) name: String,
    runtime: String,
    pub(crate) activation_id: Option<[u8; 16]>,
    pub(crate) lifecycle: Lifecycle,
}

impl Record {
    pub(crate) fn new(
        base: &Path,
        name: &WindowsSiteName,
        activation_id: Option<[u8; 16]>,
        lifecycle: Lifecycle,
    ) -> io::Result<Self> {
        let scope = name.scope_for_user(&orchbee_local_ipc::current_user_sid()?);
        let runtime = base.join(scope);
        Ok(Self {
            version: 1,
            name: name.as_str().to_owned(),
            runtime: runtime
                .to_str()
                .ok_or(io::ErrorKind::InvalidInput)?
                .to_owned(),
            activation_id,
            lifecycle,
        })
    }
    pub(crate) fn activation(&self) -> Option<ActivationState> {
        match self.lifecycle {
            Lifecycle::Active => Some(ActivationState::Active),
            Lifecycle::Stopped { request_id } => Some(ActivationState::Stopped { request_id }),
            Lifecycle::Deleting { .. } => None,
        }
    }
    pub(crate) fn is_deleting(&self) -> bool {
        matches!(self.lifecycle, Lifecycle::Deleting { .. })
    }
    pub(crate) fn stopped_id(&self) -> Option<[u8; 16]> {
        match self.lifecycle {
            Lifecycle::Stopped { request_id } => Some(request_id),
            Lifecycle::Deleting { stopped_request_id } => stopped_request_id,
            Lifecycle::Active => None,
        }
    }
}

pub(crate) fn supports_scope(scope: &str, sid: &str) -> bool {
    scope
        == WindowsSiteName::parse("default")
            .expect("fixed name")
            .scope_for_user(sid)
        || (scope.len() == 63
            && scope
                .strip_prefix(site_name_windows::NAMED_PREFIX)
                .is_some_and(|tail| {
                    tail.bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                }))
}

pub(crate) struct Directory {
    path: PathBuf,
    base: PathBuf,
    sid: String,
    _pins: Vec<File>,
}

impl Directory {
    fn path(local: &Path, base: &Path) -> PathBuf {
        local
            .join("Orchbee")
            .join("session-catalog")
            .join(Rendezvous::host_endpoint_id(base))
    }
    pub(crate) fn open(local: &Path, base: &Path) -> io::Result<Option<Self>> {
        let _local = permissions::open_tree(local)?;
        let path = Self::path(local, base);
        let pins = match permissions::open_tree(&path) {
            Ok(pins) => pins,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        Ok(Some(Self {
            path,
            base: base.to_owned(),
            sid: orchbee_local_ipc::current_user_sid()?,
            _pins: pins,
        }))
    }
    pub(crate) fn create(local: &Path, base: &Path) -> io::Result<Self> {
        let mut pins = permissions::open_tree(local)?;
        let mut path = local.to_owned();
        for component in [
            "Orchbee",
            "session-catalog",
            &Rendezvous::host_endpoint_id(base),
        ] {
            path.push(component);
            let created = match fs::create_dir(&path) {
                Ok(()) => true,
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => false,
                Err(error) => return Err(error),
            };
            let mut pin = permissions::open_directory(&path, created)?;
            if created {
                permissions::protect_new(&mut pin)?;
            } else {
                permissions::require_private(&pin)?;
            }
            pins.push(pin);
        }
        Ok(Self {
            path,
            base: base.to_owned(),
            sid: orchbee_local_ipc::current_user_sid()?,
            _pins: pins,
        })
    }
    fn validate(&self, scope: &str, record: &Record) -> io::Result<()> {
        let name = WindowsSiteName::parse(&record.name).map_err(|_| io::ErrorKind::InvalidData)?;
        if record.version != 1
            || name.scope_for_user(&self.sid) != scope
            || Path::new(&record.runtime) != self.base.join(scope)
            || record.activation_id == Some([0; 16])
            || (!record.is_deleting() && record.activation_id.is_none())
            || record.stopped_id() == Some([0; 16])
        {
            return Err(io::ErrorKind::InvalidData.into());
        }
        Ok(())
    }
    fn record_path(&self, scope: &str, pending: bool) -> io::Result<PathBuf> {
        if !supports_scope(scope, &self.sid) {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        Ok(self.path.join(format!(
            "{scope}.{}",
            if pending { "pending" } else { "json" }
        )))
    }
    pub(crate) fn read(&self, scope: &str) -> io::Result<Option<Record>> {
        let file = match permissions::open_file(&self.record_path(scope, false)?, false, false) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let mut bytes = Vec::new();
        file.take((MAXIMUM_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
        if bytes.len() > MAXIMUM_BYTES {
            return Err(io::ErrorKind::InvalidData.into());
        }
        let record: Record =
            serde_json::from_slice(&bytes).map_err(|_| io::ErrorKind::InvalidData)?;
        self.validate(scope, &record)?;
        Ok(Some(record))
    }
    pub(crate) fn list(&self, deadline: Instant) -> io::Result<Vec<(String, Record)>> {
        let mut records = Vec::new();
        for entry in fs::read_dir(&self.path)? {
            if Instant::now() >= deadline {
                return Err(io::ErrorKind::TimedOut.into());
            }
            let entry = entry?;
            let leaf = entry.file_name();
            let leaf = leaf.to_str().ok_or(io::ErrorKind::InvalidData)?;
            if let Some(scope) = leaf.strip_suffix(".pending")
                && supports_scope(scope, &self.sid)
            {
                continue;
            }
            let scope = leaf
                .strip_suffix(".json")
                .ok_or(io::ErrorKind::InvalidData)?;
            let record = self.read(scope)?.ok_or(io::ErrorKind::NotFound)?;
            records.push((scope.to_owned(), record));
            if records.len() > 256 {
                return Err(io::ErrorKind::FileTooLarge.into());
            }
        }
        Ok(records)
    }
    pub(crate) fn publish(&self, scope: &str, record: &Record) -> io::Result<()> {
        self.validate(scope, record)?;
        // A corrupt current record is never replaced by an otherwise valid new value.
        let _current = self.read(scope)?;
        let bytes = serde_json::to_vec(record).map_err(io::Error::other)?;
        if bytes.len() > MAXIMUM_BYTES {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let staging = self.record_path(scope, true)?;
        remove_private(&staging)?;
        let mut file = permissions::open_file(&staging, true, true)?;
        let written = file.write_all(&bytes).and_then(|()| file.sync_all());
        drop(file);
        let target = self.record_path(scope, false)?;
        let published = written.and_then(|()| {
            winsafe::MoveFileEx(
                staging.to_str().ok_or(io::ErrorKind::InvalidInput)?,
                Some(target.to_str().ok_or(io::ErrorKind::InvalidInput)?),
                winsafe::co::MOVEFILE::REPLACE_EXISTING,
            )
            .map_err(io::Error::other)
        });
        if let Err(error) = published {
            remove_private(&staging)?;
            return Err(error);
        }
        Ok(())
    }
    pub(crate) fn remove(&self, scope: &str) -> io::Result<()> {
        let _current = self.read(scope)?;
        remove_private(&self.record_path(scope, true)?)?;
        remove_private(&self.record_path(scope, false)?)
    }
}

fn remove_private(path: &Path) -> io::Result<()> {
    match permissions::open_file(path, false, false) {
        Ok(file) => {
            drop(file);
            fs::remove_file(path)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

/// The Server retains one exact activation generation, never a pathname-only stop authority.
pub struct WindowsRetainedSessionRecord {
    directory: Directory,
    scope: String,
    active: Record,
}
impl WindowsRetainedSessionRecord {
    pub fn retain(local: &Path, site: &Rendezvous) -> io::Result<Option<Self>> {
        let runtime = site.runtime_directory();
        let scope = runtime
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or(io::ErrorKind::InvalidInput)?;
        if !supports_scope(scope, &orchbee_local_ipc::current_user_sid()?) {
            return Ok(None);
        }
        let base = runtime.parent().ok_or(io::ErrorKind::InvalidInput)?;
        let directory = Directory::open(local, base)?.ok_or(io::ErrorKind::NotFound)?;
        let active = directory.read(scope)?.ok_or(io::ErrorKind::NotFound)?;
        if active.lifecycle != Lifecycle::Active {
            return Err(io::ErrorKind::InvalidData.into());
        }
        Ok(Some(Self {
            directory,
            scope: scope.to_owned(),
            active,
        }))
    }
    /// Only after confirmed child exit and listener cleanup, before the runtime stop receipt.
    pub fn complete(&mut self, request_id: [u8; 16]) -> io::Result<()> {
        if request_id == [0; 16] {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let current = self
            .directory
            .read(&self.scope)?
            .ok_or(io::ErrorKind::NotFound)?;
        let mut completed = self.active.clone();
        completed.lifecycle = Lifecycle::Stopped { request_id };
        if current == completed {
            return Ok(());
        }
        if current != self.active {
            return Err(io::ErrorKind::InvalidData.into());
        }
        self.directory.publish(&self.scope, &completed)
    }
}
