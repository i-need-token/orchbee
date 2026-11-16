//! Explicit ephemeral frame-directory ownership and descriptor-relative private POSIX reads;
//! applications own leases and cleanup scheduling. A POSIX descriptor cannot exclude another
//! writer as a Windows handle without write sharing does, so privacy rests on the 0700 leaf the
//! effective user owns inside the already private runtime: every create, listing, read and
//! removal goes through the pinned runtime or leaf descriptor and never follows a symbolic link.
use nix::{
    dir::Dir,
    errno::Errno,
    fcntl::{AtFlags, OFlag, openat},
    sys::stat::{FileStat, Mode, SFlag, fstat, fstatat, mkdirat},
    unistd::{UnlinkatFlags, geteuid, unlinkat},
};
use orchbee_local_runtime::private_filesystem as permissions;
use std::{
    ffi::{OsStr, OsString},
    fs::File,
    io::{self, Read, Write},
    os::unix::ffi::OsStrExt,
    path::{Component, Path, PathBuf},
};

const OWNER: &str = ".orchbee-frame-owner.json";
/// Maximum complete private file-frame copy; dimensions and formats belong to the caller.
pub(crate) const FRAME_MAXIMUM_BYTES: usize = 16 * 1024 * 1024;
/// Nested producer folders removed at release; a deeper tree is refused and stays owned.
const MAXIMUM_DEPTH: usize = 16;
/// Names one release pass removes from a folder; any rest keeps the root for the next retry.
const MAXIMUM_LISTING: usize = 4096;
const DIRECTORY_MODE: u32 = 0o700;
const FILE_MODE: u32 = 0o600;
const DIRECTORY_OPEN_FLAGS: OFlag = OFlag::O_RDONLY
    .union(OFlag::O_DIRECTORY)
    .union(OFlag::O_CLOEXEC)
    .union(OFlag::O_NOFOLLOW);
/// Never blocks on a FIFO placed at a frame name; regular files ignore the flag.
const FRAME_OPEN_FLAGS: OFlag = OFlag::O_RDONLY
    .union(OFlag::O_CLOEXEC)
    .union(OFlag::O_NOFOLLOW)
    .union(OFlag::O_NONBLOCK);
const OWNER_CREATE_FLAGS: OFlag = OFlag::O_WRONLY
    .union(OFlag::O_CREAT)
    .union(OFlag::O_EXCL)
    .union(OFlag::O_CLOEXEC)
    .union(OFlag::O_NOFOLLOW);
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
    leaf: String,
    /// The admitted private runtime, which every leaf operation is relative to.
    runtime: File,
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
            || runtime.to_str().is_none()
        {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let runtime_pin = permissions::open_tree(runtime)?
            .pop()
            .ok_or_else(|| io::Error::from(io::ErrorKind::PermissionDenied))?;
        let leaf = format!("graphics-{nonce}");
        Ok(Self {
            path: runtime.join(&leaf),
            leaf,
            runtime: runtime_pin,
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
        mkdirat(&self.runtime, self.leaf.as_str(), mode(DIRECTORY_MODE))?;
        self.state = State::Preparing(None);
        let mut pin = self.open_leaf()?;
        let identity = permissions::file_identity(&pin)?;
        self.state = State::Preparing(Some(identity));
        permissions::protect_new(&mut pin)?;
        let mut owner = create_owner(&pin)?;
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

    /// Copies one exact regular single-link direct child the effective user owns, opened relative
    /// to the pinned leaf. The producer closes its writer first; the length is checked before and
    /// after the copy, so a frame still growing or shrinking is refused. The frame's own mode is
    /// the producer's, since the private leaf already keeps every other user out.
    pub(crate) fn read_frame(&self, path: &Path, expected_bytes: usize) -> io::Result<Vec<u8>> {
        let State::Ready { pin, .. } = &self.state else {
            return Err(io::ErrorKind::NotConnected.into());
        };
        if expected_bytes == 0 || expected_bytes > FRAME_MAXIMUM_BYTES {
            return Err(io::ErrorKind::FileTooLarge.into());
        }
        let leaf = path
            .file_name()
            .and_then(OsStr::to_str)
            .ok_or(io::ErrorKind::InvalidInput)?;
        if path.parent() != Some(self.path.as_path())
            || path
                .components()
                .any(|part| matches!(part, Component::CurDir | Component::ParentDir))
            || path
                .to_str()
                .is_none_or(|text| text.split('/').any(|part| matches!(part, "." | "..")))
            || !frame_leaf(leaf)
        {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let mut file =
            File::from(openat(pin, leaf, FRAME_OPEN_FLAGS, Mode::empty()).map_err(refusal)?);
        let status = fstat(&file)?;
        if kind(&status) != SFlag::S_IFREG
            || status.st_nlink != 1
            || status.st_uid != geteuid().as_raw()
        {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
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
        let pin = match self.open_leaf() {
            Ok(pin) => pin,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                self.state = State::Released;
                return Ok(());
            }
            Err(error) => return Err(error),
        };
        self.require_owned(&pin)?;
        // Keep recovery ownership until all producer contents are gone. A busy frame must not
        // consume the owner note just because enumeration happened to visit it first.
        empty_directory(&pin, 0)?;
        remove(&pin, OsStr::new(OWNER), UnlinkatFlags::NoRemoveDir)?;
        drop(pin);
        if let Err(error) = unlinkat(&self.runtime, self.leaf.as_str(), UnlinkatFlags::RemoveDir) {
            // A producer that added an entry since the listing keeps the leaf; recreate the
            // consumed owner note so this owner still describes it and retries later.
            self.restore_owner()?;
            return Err(error.into());
        }
        self.state = State::Released;
        Ok(())
    }

    fn restore_owner(&self) -> io::Result<()> {
        let pin = self.open_leaf()?;
        self.require_owned(&pin)?;
        let mut file = create_owner(&pin)?;
        file.write_all(&self.owner_record)?;
        file.sync_all()
    }

    /// The opened leaf is still private and, once created, the very directory this owner made.
    fn require_owned(&self, pin: &File) -> io::Result<()> {
        permissions::require_private(pin)?;
        if let State::Releasing(Some(saved)) = self.state
            && permissions::file_identity(pin)? != saved
        {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        Ok(())
    }

    /// The leaf as a real directory in the pinned runtime; a link or another object there is a
    /// refusal, never followed.
    fn open_leaf(&self) -> io::Result<File> {
        openat(
            &self.runtime,
            self.leaf.as_str(),
            DIRECTORY_OPEN_FLAGS,
            Mode::empty(),
        )
        .map(File::from)
        .map_err(refusal)
    }
}

/// Creates the owner note exclusively in the pinned leaf as a private file.
fn create_owner(pin: &File) -> io::Result<File> {
    let mut file =
        File::from(openat(pin, OWNER, OWNER_CREATE_FLAGS, mode(FILE_MODE)).map_err(refusal)?);
    permissions::protect_new(&mut file)?;
    Ok(file)
}

/// Removes every entry below `directory` except the owner note at the top, each relative to its
/// pinned folder and never through a link.
fn empty_directory(directory: &File, depth: usize) -> io::Result<()> {
    for name in listing(directory)? {
        if depth == 0
            && name
                .to_str()
                .is_some_and(|name| name.eq_ignore_ascii_case(OWNER))
        {
            continue;
        }
        let name = name.as_os_str();
        match fstatat(directory, name, AtFlags::AT_SYMLINK_NOFOLLOW) {
            Ok(status) if kind(&status) == SFlag::S_IFDIR => {
                if depth + 1 >= MAXIMUM_DEPTH {
                    return Err(io::ErrorKind::PermissionDenied.into());
                }
                let child = File::from(
                    openat(directory, name, DIRECTORY_OPEN_FLAGS, Mode::empty())
                        .map_err(refusal)?,
                );
                empty_directory(&child, depth + 1)?;
                remove(directory, name, UnlinkatFlags::RemoveDir)?;
            }
            Ok(_) => remove(directory, name, UnlinkatFlags::NoRemoveDir)?,
            Err(Errno::ENOENT) => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

/// Up to [`MAXIMUM_LISTING`] names in the pinned folder, read through a fresh description of it
/// before any removal, since removing entries during a listing may skip others.
fn listing(directory: &File) -> io::Result<Vec<OsString>> {
    let mut folder = Dir::openat(directory, ".", DIRECTORY_OPEN_FLAGS, Mode::empty())?;
    let mut names = Vec::new();
    for entry in folder.iter() {
        let entry = entry?;
        let name = OsStr::from_bytes(entry.file_name().to_bytes());
        if name == "." || name == ".." {
            continue;
        }
        if names.len() == MAXIMUM_LISTING {
            break;
        }
        names.push(name.to_owned());
    }
    Ok(names)
}

fn remove(directory: &File, name: &OsStr, flag: UnlinkatFlags) -> io::Result<()> {
    match unlinkat(directory, name, flag) {
        Ok(()) | Err(Errno::ENOENT) => Ok(()),
        Err(error) => Err(error.into()),
    }
}

/// A symbolic link or a non-directory where a directory belongs is an unsafe object, not a
/// missing one; every other error keeps its own meaning.
fn refusal(error: Errno) -> io::Error {
    match error {
        Errno::ELOOP | Errno::ENOTDIR => io::ErrorKind::PermissionDenied.into(),
        error => error.into(),
    }
}

fn kind(status: &FileStat) -> SFlag {
    SFlag::from_bits_truncate(status.st_mode) & SFlag::S_IFMT
}

fn mode(bits: u32) -> Mode {
    Mode::from_bits_truncate(bits as nix::libc::mode_t)
}

/// A frame name: one UTF-8 component of at most 255 bytes without control characters, never the
/// owner note. The owner comparison ignores ASCII case because the default macOS volume does.
fn frame_leaf(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && !matches!(name, "." | "..")
        && !name.eq_ignore_ascii_case(OWNER)
        && !name.chars().any(|c| c.is_control() || c == '/')
}

#[cfg(test)]
mod tests;
