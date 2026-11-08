//! Retains the generation-scoped Windows reader and its existing I/O failures.
use super::Read;
use orchbee_local_runtime::ModeCheckedLocalSessionRendezvous;
use std::io;

pub(in crate::local_activation) fn read(session: &ModeCheckedLocalSessionRendezvous) -> Read {
    Ok((|| {
        let root = orchbee_local_runtime::DataRoot::resolve(|name| std::env::var_os(name))
            .map_err(|_| io::Error::from(io::ErrorKind::NotFound))?;
        session.checkpoint_restore_notice(&root)
    })())
}
