//! Durable registration and completion effects have no implementation on this host.
use crate::{DataRoot, ModeCheckedLocalSessionRendezvous};
use std::io;
pub enum RetainedSessionRecord {}
impl RetainedSessionRecord {
    pub fn retain(
        _root: &DataRoot,
        _session: &ModeCheckedLocalSessionRendezvous,
    ) -> io::Result<Self> {
        Err(io::ErrorKind::Unsupported.into())
    }
    pub fn complete(&mut self, _request_id: [u8; 16]) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }
}
