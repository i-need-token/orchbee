//! Preserve the Windows poll adapter's exact expected-byte wake semantics.
use orchbee_local_ipc::LocalIo;
use std::{io, sync::atomic::AtomicU8};

pub(super) struct Signal;

impl Signal {
    pub(super) fn new() -> io::Result<Self> {
        Ok(Self)
    }
    pub(super) fn wake<'a>(&'a self, state: &'a AtomicU8) -> LocalIo<'a> {
        LocalIo::Wake(state, 1)
    }
    pub(super) fn notify(&self) {
        // OwnedCancellation has already published the exact signalled byte.
    }
}
