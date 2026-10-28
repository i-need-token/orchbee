//! Retains the generation-scoped Windows reader and its existing I/O failures.
use super::Read;
use orchbee_local_runtime::ModeCheckedLocalSessionRendezvous;
use std::{io, path::PathBuf};

pub(in crate::local_activation) fn read(session: &ModeCheckedLocalSessionRendezvous) -> Read {
    Ok((|| {
        let local = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .ok_or(io::ErrorKind::NotFound)?;
        session.checkpoint_restore_notice(&local)
    })())
}
