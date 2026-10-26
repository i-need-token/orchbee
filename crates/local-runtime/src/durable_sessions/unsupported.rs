//! Durable registration and completion effects have no implementation on this host.
use crate::ModeCheckedLocalSessionRendezvous;
use std::{io, path::Path};
pub enum RetainedSessionRecord {}
impl RetainedSessionRecord {
    pub fn retain(
        _local: &Path,
        _session: &ModeCheckedLocalSessionRendezvous,
    ) -> io::Result<Option<Self>> {
        Err(io::ErrorKind::Unsupported.into())
    }
    pub fn complete(&mut self, _request_id: [u8; 16]) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }
}
