//! macOS and Linux have no hub yet: no holder starts, and nothing is asked of a hub.
use super::{Hub, Shared};
use crate::background_thread::BackgroundThread;
use std::{io, sync::Arc};

pub(super) fn spawn(_shared: Arc<Shared>) -> io::Result<BackgroundThread> {
    Err(io::ErrorKind::Unsupported.into())
}

pub(super) fn reload(hub: Hub) {
    match hub {}
}

pub(super) fn connect(_shared: &Arc<Shared>, _machine: String, _number: u64) {}
