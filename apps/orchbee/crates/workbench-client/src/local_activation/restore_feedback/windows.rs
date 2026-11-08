//! Retains the generation-scoped Windows reader and its existing I/O failures.
use super::Read;
use orchbee_local_runtime::ModeCheckedLocalSessionRendezvous;
use std::io;

pub(in crate::local_activation) fn read(session: &ModeCheckedLocalSessionRendezvous) -> Read {
    Ok((|| {
        let local = orchbee_local_runtime::local_state_root(|name| std::env::var_os(name))
            .ok_or(io::ErrorKind::NotFound)?;
        session.checkpoint_restore_notice(&local)
    })())
}
