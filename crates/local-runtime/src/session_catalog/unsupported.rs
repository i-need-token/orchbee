//! Named lifecycle effects are refused before observing or mutating host state.
use super::{
    NamedSessionDeleteFailure, NamedSessionDeleteOutcome, NamedSessionDeleteReason,
    UnrecognizedRecord,
};
use crate::{
    ActivationState, DataRoot, ModeCheckedLocalSessionRendezvous as Rendezvous,
    ModeCheckedSessionLock, RetainedChild, SessionName,
};
use std::{
    convert::Infallible,
    ffi::OsStr,
    io,
    path::{Path, PathBuf},
    time::Instant,
};
pub enum SessionCatalog {}
#[derive(Debug)]
pub struct NamedSessionEntry {
    pub name: SessionName,
    pub runtime: PathBuf,
    pub deleting: bool,
    pub unreadable: bool,
    pub conflicted: bool,
    unavailable: Infallible,
}
impl SessionCatalog {
    pub fn try_lock(_base: &Path, _root: &DataRoot) -> io::Result<Self> {
        Err(io::ErrorKind::Unsupported.into())
    }
    pub fn spelling(_root: &DataRoot, _name: &SessionName) -> io::Result<SessionName> {
        Err(io::ErrorKind::Unsupported.into())
    }
    pub fn prepare_activation(&self, _name: &SessionName) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }
    pub fn recover_inactive(
        &self,
        _session: &Rendezvous,
        _activation: &ModeCheckedSessionLock,
        _lifetime: &ModeCheckedSessionLock,
    ) -> io::Result<bool> {
        Err(io::ErrorKind::Unsupported.into())
    }
    pub fn begin_activation(
        &self,
        _session: &Rendezvous,
        _activation_id: [u8; 16],
    ) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }
    pub fn spawn_server(
        &self,
        _session: &Rendezvous,
        _activation: &ModeCheckedSessionLock,
        _executable: &Path,
        _arguments: &[&OsStr],
        _deadline: Instant,
    ) -> io::Result<RetainedChild> {
        Err(io::ErrorKind::Unsupported.into())
    }
    pub fn list(
        _base: &Path,
        _root: &DataRoot,
        _deadline: Instant,
    ) -> io::Result<Vec<NamedSessionEntry>> {
        Err(io::ErrorKind::Unsupported.into())
    }
    pub fn unrecognized_records(
        _root: &DataRoot,
        _scope: &str,
        _deadline: Instant,
    ) -> io::Result<Vec<UnrecognizedRecord>> {
        Err(io::ErrorKind::Unsupported.into())
    }
    pub fn delete(
        &self,
        _name: &SessionName,
        _deadline: Instant,
        _interrupted: impl FnMut() -> io::Result<bool>,
    ) -> Result<NamedSessionDeleteOutcome, NamedSessionDeleteFailure> {
        Err(NamedSessionDeleteFailure {
            reason: NamedSessionDeleteReason::Unsupported,
            pending: false,
        })
    }
}
impl NamedSessionEntry {
    pub fn cold_recoverable(&self) -> io::Result<bool> {
        Err(io::ErrorKind::Unsupported.into())
    }
    pub fn accepts_runtime_state(&self, _marker: ActivationState) -> bool {
        match self.unavailable {}
    }
    pub fn absent_runtime_state(&self) -> io::Result<Option<ActivationState>> {
        Err(io::ErrorKind::Unsupported.into())
    }
}
