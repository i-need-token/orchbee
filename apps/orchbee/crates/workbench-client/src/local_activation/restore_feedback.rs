//! The checkpoint restore notice an activation reports. The read keeps the generation scope and I/O
//! failures of the Session storage it reads.
use orchbee_local_runtime::{CheckpointRestoreNotice, ModeCheckedLocalSessionRendezvous};
use std::io;

pub(super) fn read(
    session: &ModeCheckedLocalSessionRendezvous,
) -> io::Result<Option<CheckpointRestoreNotice>> {
    let root = orchbee_local_runtime::DataRoot::resolve(|name| std::env::var_os(name))
        .map_err(|_| io::Error::from(io::ErrorKind::NotFound))?;
    session.checkpoint_restore_notice(&root)
}
