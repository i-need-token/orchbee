//! Checkpoint storage is explicitly unavailable without its admitted platform implementation.
use super::{CheckpointLoad, CheckpointRestoreNotice, LayoutHistoryEntry};
use crate::{DataRoot, ModeCheckedLocalSessionRendezvous, RetainedSessionRecord};
use std::io;
pub enum SessionStore {}
impl SessionStore {
    pub fn open(
        _root: &DataRoot,
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
    pub fn read_layout_history(&self) -> io::Result<Vec<LayoutHistoryEntry>> {
        Err(io::ErrorKind::Unsupported.into())
    }
    pub fn read_layout_history_entry(&self, _saved_at: u64) -> io::Result<Option<Vec<u8>>> {
        Err(io::ErrorKind::Unsupported.into())
    }
    pub fn terminal_output_tokens(&self) -> io::Result<Vec<u64>> {
        Err(io::ErrorKind::Unsupported.into())
    }
    pub fn read_terminal_output(&self, _token: u64) -> io::Result<Option<Vec<u8>>> {
        Err(io::ErrorKind::Unsupported.into())
    }
    pub fn write_terminal_output(&mut self, _token: u64, _payload: &[u8]) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }
    pub fn remove_terminal_output(&mut self, _token: u64) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }
    pub fn record_layout_history(
        &mut self,
        _saved_at: u64,
        _payload: &[u8],
        _keep: usize,
    ) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }
}
