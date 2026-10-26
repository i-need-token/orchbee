//! Explicit ephemeral frame-directory ownership and pinned private Windows reads; applications own leases and cleanup scheduling.
use orchbee_local_runtime::private_filesystem as permissions;
use std::{
    fs::{self, File},
    io::{self, Read, Write},
    os::windows::fs::MetadataExt,
    path::{Component, Path, PathBuf},
};

const OWNER: &str = ".orchbee-frame-owner.json";
/// Maximum complete private file-frame copy; dimensions and formats belong to the caller.
pub(crate) const FRAME_MAXIMUM_BYTES: usize = 16 * 1024 * 1024;
type Identity = (u64, u64);

enum State {
    Reserved,
    Preparing(Option<Identity>),
    Ready { pin: File, identity: Identity },
    Releasing(Option<Identity>),
    Released,
}

/// Owns an exclusively created runtime leaf until explicit successful `reap`; drop never proves cleanup.
pub(crate) struct PrivateFrameDirectory {
    path: PathBuf,
    runtime: PathBuf,
    _ancestors: Vec<File>,
    state: State,
    owner_record: Vec<u8>,
}

impl PrivateFrameDirectory {
    /// Pins an existing private runtime and derives a fresh leaf without creating or adopting it.
    pub(crate) fn reserve(runtime: &Path, nonce: &str) -> io::Result<Self> {
        if nonce.len() != 32
            || !nonce
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || nonce.bytes().all(|b| b == b'0')
        {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let ancestors = permissions::open_tree(runtime)?;
        Ok(Self {
            path: runtime.join(format!("graphics-{nonce}")),
            runtime: runtime.to_owned(),
            _ancestors: ancestors,
            state: State::Reserved,
            owner_record: Vec::new(),
        })
    }

    /// Creates once, protects the new root and flushes its bounded owner record; failure retains ownership.
    pub(crate) fn create(&mut self, owner_record: &[u8]) -> io::Result<()> {
        if !matches!(self.state, State::Reserved)
            || owner_record.is_empty()
            || owner_record.len() > 4096
        {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        self.owner_record = owner_record.to_vec();
        fs::create_dir(&self.path)?;
        self.state = State::Preparing(None);
        let mut pin = permissions::open_directory(&self.path, true)?;
        let identity = identity(&pin)?;
        self.state = State::Preparing(Some(identity));
        permissions::protect_new(&mut pin)?;
        let mut owner = permissions::open_file(&self.path.join(OWNER), true, true)?;
        owner.write_all(owner_record)?;
        owner.sync_all()?;
        drop(owner);
        self.state = State::Ready { pin, identity };
        Ok(())
    }

    /// Exact derived root; only successfully created roots may be advertised by the application.
    #[must_use]
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    /// Copies one exact regular direct child while excluding existing or future write/delete handles.
    pub(crate) fn read_frame(&self, path: &Path, expected_bytes: usize) -> io::Result<Vec<u8>> {
        let State::Ready {
            pin,
            identity: expected_root,
        } = &self.state
        else {
            return Err(io::ErrorKind::NotConnected.into());
        };
        if expected_bytes == 0 || expected_bytes > FRAME_MAXIMUM_BYTES {
            return Err(io::ErrorKind::FileTooLarge.into());
        }
        let leaf = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(io::ErrorKind::InvalidInput)?;
        if path.parent() != Some(self.path.as_path())
            || path
                .components()
                .any(|part| matches!(part, Component::CurDir | Component::ParentDir))
            || path.to_str().is_none_or(|text| {
                text.split(['/', '\\'])
                    .any(|part| matches!(part, "." | ".."))
            })
            || !frame_leaf(leaf)
            || identity(pin)? != *expected_root
        {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let mut file = permissions::open_snapshot_file(path)?;
        if file.metadata()?.len() != expected_bytes as u64 {
            return Err(io::ErrorKind::InvalidData.into());
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(expected_bytes)
            .map_err(|_| io::Error::from(io::ErrorKind::OutOfMemory))?;
        bytes.resize(expected_bytes, 0);
        file.read_exact(&mut bytes)?;
        let mut end = [0u8; 1];
        if file.read(&mut end)? != 0 || file.metadata()?.len() != expected_bytes as u64 {
            return Err(io::ErrorKind::InvalidData.into());
        }
        Ok(bytes)
    }

    /// Removes only this created leaf after consumers end; a failure retains the same retryable owner.
    pub(crate) fn reap(&mut self) -> io::Result<()> {
        let expected = match &self.state {
            State::Reserved | State::Released => {
                self.state = State::Released;
                return Ok(());
            }
            State::Preparing(identity) | State::Releasing(identity) => *identity,
            State::Ready { identity, .. } => Some(*identity),
        };
        self.state = State::Releasing(expected); // Release the root pin before owned deletion.
        let pin = match permissions::open_directory(&self.path, false) {
            Ok(pin) => pin,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                self.state = State::Released;
                return Ok(());
            }
            Err(error) => return Err(error),
        };
        permissions::require_private(&pin)?;
        let current_identity = identity(&pin)?;
        if expected.is_some_and(|saved| current_identity != saved)
            || self.path.parent() != Some(self.runtime.as_path())
            || fs::canonicalize(&self.path)?.parent()
                != Some(fs::canonicalize(&self.runtime)?.as_path())
        {
            return Err(io::Error::from(io::ErrorKind::PermissionDenied));
        }
        // The saved root identity and pinned ancestors are checked before deletion; std does not
        // traverse Windows directory reparse points. No arbitrary or pre-existing root is owned.
        drop(pin);
        // Keep recovery ownership until all producer contents are gone. A busy frame must not
        // consume the owner note just because directory enumeration happened to visit it first.
        for entry in fs::read_dir(&self.path)? {
            let entry = entry?;
            if entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.eq_ignore_ascii_case(OWNER))
            {
                continue;
            }
            let metadata = fs::symlink_metadata(entry.path())?;
            if metadata.file_attributes() & 0x400 != 0 {
                if metadata.is_dir() {
                    fs::remove_dir(entry.path())?;
                } else {
                    fs::remove_file(entry.path())?;
                }
            } else if metadata.is_dir() {
                if entry
                    .path()
                    .canonicalize()?
                    .starts_with(self.path.canonicalize()?)
                {
                    fs::remove_dir_all(entry.path())?;
                } else {
                    return Err(io::Error::from(io::ErrorKind::PermissionDenied));
                }
            } else {
                fs::remove_file(entry.path())?;
            }
        }
        match fs::remove_file(self.path.join(OWNER)) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        if let Err(error) = fs::remove_dir(&self.path) {
            // Recreate the consumed owner note if a directory handle blocks the final removal.
            // Any restoration failure still retains this in-memory owner and refuses clean stop.
            self.restore_owner()?;
            return Err(error);
        }
        self.state = State::Released;
        Ok(())
    }

    fn restore_owner(&self) -> io::Result<()> {
        let pin = permissions::open_directory(&self.path, false)?;
        permissions::require_private(&pin)?;
        if let State::Releasing(Some(saved)) = self.state
            && identity(&pin)? != saved
        {
            return Err(io::Error::from(io::ErrorKind::PermissionDenied));
        }
        let mut file = permissions::open_file(&self.path.join(OWNER), true, true)?;
        file.write_all(&self.owner_record)?;
        file.sync_all()
    }
}

fn identity(file: &File) -> io::Result<Identity> {
    permissions::file_identity(file)
}
fn frame_leaf(name: &str) -> bool {
    if name.is_empty()
        || name.eq_ignore_ascii_case(OWNER)
        || name.ends_with(['.', ' '])
        || name.encode_utf16().count() > 255
        || name.chars().any(|c| {
            c.is_control() || matches!(c, ':' | '<' | '>' | '"' | '|' | '?' | '*' | '/' | '\\')
        })
    {
        return false;
    }
    let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    !matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) && !(stem.len() == 4
        && (stem.starts_with("COM") || stem.starts_with("LPT"))
        && matches!(stem.as_bytes()[3], b'1'..=b'9'))
}

#[cfg(test)]
mod tests;
