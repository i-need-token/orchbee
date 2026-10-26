//! Checkpoint storage is explicitly unavailable without its admitted platform implementation.
use super::{CheckpointLoad, CheckpointRestoreNotice};
use crate::{ModeCheckedLocalSessionRendezvous, RetainedSessionRecord};
use std::{io, path::Path};
pub enum SessionStore {}
impl SessionStore {
    pub fn open(
        _local: &Path,
        _session: &ModeCheckedLocalSessionRendezvous,
        _maximum_bytes: usize,
    ) -> io::Result<Self> {
        Err(io::ErrorKind::Unsupported.into())
    }
    pub fn read(&self) -> io::Result<Option<Vec<u8>>> {
        Err(io::ErrorKind::Unsupported.into())
    }
    pub fn read_validated<T>(
        &self,
        _parse: impl FnMut(&[u8]) -> Option<T>,
    ) -> io::Result<CheckpointLoad<T>> {
        Err(io::ErrorKind::Unsupported.into())
    }
    pub fn publish(&mut self, _bytes: &[u8]) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }
    pub fn publish_validated(
        &mut self,
        _bytes: &[u8],
        _validate: impl FnMut(&[u8]) -> bool,
    ) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }
    pub fn publish_restore_notice(
        &self,
        _record: &RetainedSessionRecord,
        _notice: Option<CheckpointRestoreNotice>,
    ) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }
}
