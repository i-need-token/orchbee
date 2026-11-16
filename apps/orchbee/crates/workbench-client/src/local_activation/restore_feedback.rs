//! Optional activation-generation feedback, separate from a supported reader's I/O failure. The
//! reader keeps the generation scope and I/O failures of the Session storage it reads.
use orchbee_local_runtime::{CheckpointRestoreNotice, ModeCheckedLocalSessionRendezvous};
use std::io;

/// No restore-feedback reader exists; activation may continue without a notice.
pub(super) struct Unsupported;

/// Windows and macOS Sessions keep a checkpoint and publish its restore notice. A Linux Session
/// keeps none yet, so its activation performs no storage probe. A policy constant, so every branch
/// compiles everywhere.
const READER: bool = cfg!(any(windows, target_os = "macos"));

pub(super) fn read(
    session: &ModeCheckedLocalSessionRendezvous,
) -> Result<io::Result<Option<CheckpointRestoreNotice>>, Unsupported> {
    if !READER {
        return Err(Unsupported);
    }
    Ok((|| {
        let root = orchbee_local_runtime::DataRoot::resolve(|name| std::env::var_os(name))
            .map_err(|_| io::Error::from(io::ErrorKind::NotFound))?;
        session.checkpoint_restore_notice(&root)
    })())
}
