//! Bounded, generation-bound restore feedback. Reading never takes the Server's state lock.
use crate::{
    CheckpointRestoreNotice as Notice, ModeCheckedLocalSessionRendezvous as Rendezvous,
    RetainedSessionRecord, SessionStore,
    durable_sessions::{Directory, Lifecycle, checkpoint_path, supports_scope},
    private_filesystem as permissions,
};
use std::{
    fs,
    io::{self, Read, Write},
    path::Path,
};

pub(crate) const FILE: &str = "checkpoint.restore-notice";
pub(crate) const PENDING: &str = "checkpoint.restore-notice.pending";
const PREFIX: &[u8] = b"orchbee-checkpoint-restore-v1:";
const LENGTH: usize = PREFIX.len() + 32 + 2;

fn encode(generation: [u8; 16], notice: Option<Notice>) -> io::Result<Vec<u8>> {
    if generation == [0; 16] {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let mut bytes = PREFIX.to_vec();
    for byte in generation {
        bytes.push(b"0123456789abcdef"[(byte >> 4) as usize]);
        bytes.push(b"0123456789abcdef"[(byte & 15) as usize]);
    }
    bytes.push(b':');
    bytes.push(match notice {
        None => b'N',
        Some(Notice::Recovered) => b'R',
        Some(Notice::Empty) => b'E',
    });
    Ok(bytes)
}
fn decode(bytes: &[u8]) -> io::Result<([u8; 16], Option<Notice>)> {
    let invalid = || io::Error::from(io::ErrorKind::InvalidData);
    if bytes.len() != LENGTH {
        return Err(invalid());
    }
    let tail = bytes.strip_prefix(PREFIX).ok_or_else(invalid)?;
    if tail[32] != b':' {
        return Err(invalid());
    }
    let mut generation = [0; 16];
    let digit = |b| match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        _ => Err(invalid()),
    };
    for (i, pair) in tail[..32].chunks_exact(2).enumerate() {
        generation[i] = digit(pair[0])? * 16 + digit(pair[1])?;
    }
    if generation == [0; 16] {
        return Err(invalid());
    }
    let notice = match tail[33] {
        b'N' => None,
        b'R' => Some(Notice::Recovered),
        b'E' => Some(Notice::Empty),
        _ => return Err(invalid()),
    };
    Ok((generation, notice))
}

impl SessionStore {
    pub fn publish_restore_notice(
        &self,
        record: &RetainedSessionRecord,
        notice: Option<Notice>,
    ) -> io::Result<()> {
        let generation = record.current_activation_id()?;
        if self.directory != record.checkpoint_directory() {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let bytes = encode(generation, notice)?;
        let pending = self.directory.join(PENDING);
        let target = self.directory.join(FILE);
        remove_pending(&pending)?;
        // Pin and admit any preceding file before replacement, without interpreting stale content.
        let previous = match permissions::open_snapshot_file(&target) {
            Ok(file) => Some(file),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => return Err(error),
        };
        let mut file = permissions::open_file(&pending, true, true)?;
        let written = file.write_all(&bytes).and_then(|()| file.sync_all());
        drop(file);
        drop(previous);
        let result = written.and_then(|()| {
            record.current_activation_id().and_then(|current| {
                if current != generation {
                    return Err(io::ErrorKind::InvalidData.into());
                }
                winsafe::MoveFileEx(
                    text(&pending)?,
                    Some(text(&target)?),
                    winsafe::co::MOVEFILE::REPLACE_EXISTING,
                )
                .map_err(io::Error::other)
            })
        });
        if let Err(error) = result {
            return match remove_pending(&pending) {
                Ok(()) => Err(error),
                Err(cleanup) => Err(io::Error::other(format!(
                    "{error}; restore notice staging cleanup unconfirmed: {cleanup}"
                ))),
            };
        }
        Ok(())
    }
}
impl Rendezvous {
    /// Only an admitted active catalog generation may supply feedback for this runtime.
    pub fn checkpoint_restore_notice(&self, local: &Path) -> io::Result<Option<Notice>> {
        let runtime = self.runtime_directory();
        let scope = runtime
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or(io::ErrorKind::InvalidInput)?;
        if !supports_scope(scope, &orchbee_local_ipc::current_user_sid()?) {
            return Ok(None);
        }
        let Some(directory) =
            Directory::open(local, runtime.parent().ok_or(io::ErrorKind::InvalidInput)?)?
        else {
            return Ok(None);
        };
        let Some(record) = directory.read(scope)? else {
            return Ok(None);
        };
        if record.lifecycle != Lifecycle::Active {
            return Err(io::ErrorKind::InvalidData.into());
        }
        let path = checkpoint_path(local, runtime);
        let _pins = match permissions::open_tree(&path) {
            Ok(pins) => pins,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let file = match permissions::open_snapshot_file(&path.join(FILE)) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let mut bytes = Vec::new();
        file.take((LENGTH + 1) as u64).read_to_end(&mut bytes)?;
        let (generation, notice) = decode(&bytes)?;
        let current = directory.read(scope)?.ok_or(io::ErrorKind::NotFound)?;
        if !current.same_active_generation(&record) {
            return Err(io::ErrorKind::InvalidData.into());
        }
        Ok((record.activation_id == Some(generation))
            .then_some(notice)
            .flatten())
    }
}
pub(crate) fn remove_pending(path: &Path) -> io::Result<()> {
    match permissions::open_file(path, false, false) {
        Ok(file) => {
            drop(file);
            fs::remove_file(path)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}
fn text(path: &Path) -> io::Result<&str> {
    path.to_str()
        .ok_or_else(|| io::ErrorKind::InvalidInput.into())
}

#[cfg(test)]
mod tests;
