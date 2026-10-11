//! Guarded filesystem mechanics for managed entries — the CLI's strengthened
//! publication model. Every admission is no-replace: files publish via
//! `link(2)`+`unlink(2)` (POSIX EEXIST on every filesystem), directories via
//! `renameat2(RENAME_NOREPLACE)` on Linux and `renameatx_np(RENAME_EXCL)` on
//! macOS; a platform without a directory no-replace primitive refuses
//! directory-kind effects rather than silently degrading. Move-aside uses the
//! same no-replace primitive with a bounded fresh-name retry — a collision can
//! never destroy an existing backup or a foreign arrival.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

/// Bounded fresh-name attempts for a backup destination.
const BACKUP_ATTEMPTS: usize = 8;
const STAGE_ATTEMPTS: usize = 8;

/// Why a guarded filesystem step did not land.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FsStep {
    Done,
    /// The destination was already occupied — a foreign arrival or a
    /// retained residue, never clobbered.
    Conflict,
    /// A real I/O failure (not the occupied-destination case).
    Failed,
}

/// `mkdtemp` equivalent: a sibling staging directory `.orchbee-agent-<tag>-*`
/// owned 0700, created no-replace under a bounded name retry. The canonical
/// `mkdir(parent, {mode:0o700, recursive})` applies the mode only to freshly
/// created directories — an existing parent keeps its permissions.
pub(crate) fn stage_directory(parent: &Path, tag: &str) -> io::Result<PathBuf> {
    let fresh = !parent.exists();
    fs::create_dir_all(parent)?;
    if fresh {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
    }
    for attempt in 0..STAGE_ATTEMPTS {
        let candidate = parent.join(format!(
            ".orchbee-agent-{tag}-{:x}{:x}",
            std::process::id(),
            nonce(attempt),
        ));
        #[cfg(test)]
        super::tests::fault_injection::boundary(
            match tag {
                "capsule" => "capsule-stage",
                _ => "stage-create",
            },
            &candidate,
        );
        #[cfg(test)]
        if let Some(errno) =
            super::tests::fault_injection::prim(super::tests::fault_injection::Prim::StageCreateDir)
        {
            return Err(super::tests::fault_injection::errno_error(errno));
        }
        match fs::create_dir(&candidate) {
            Ok(()) => {
                fs::set_permissions(
                    &candidate,
                    std::os::unix::fs::PermissionsExt::from_mode(0o700),
                )?;
                return Ok(candidate);
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "staging names exhausted",
    ))
}

fn nonce(attempt: usize) -> u64 {
    // Per-invocation uniqueness without a new dependency: address entropy
    // plus the attempt counter; the no-replace primitive is what enforces
    // correctness, not the name quality.
    static SEED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seed = SEED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|span| span.subsec_nanos() as u64)
        .unwrap_or(0);
    nanos ^ seed.wrapping_mul(0x9e3779b97f4a7c15) ^ (attempt as u64)
}

/// `O_NOFOLLOW`-disciplined write of `bytes` at `path`, `wx`+0600: fails
/// rather than following a swapped symlink or overwriting an arrival.
pub(crate) fn write_file_exclusive(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(path)
        .and_then(|mut file| std::io::Write::write_all(&mut file, bytes))
}

/// Copy a directory tree exactly (regular files + directories only — any
/// symlink or other kind is a typed failure, matching the capsule's own
/// no-symlink contract).
pub(crate) fn copy_tree(source: &Path, target: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(source)?;
    if metadata.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "symlink in payload",
        ));
    }
    if metadata.is_dir() {
        fs::create_dir(target)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            copy_tree(&entry.path(), &target.join(entry.file_name()))?;
        }
        return Ok(());
    }
    if metadata.is_file() {
        return fs::copy(source, target).map(|_| ());
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "non-regular payload entry",
    ))
}

/// No-replace publish of a staged entry at `target`. Files use link+unlink;
/// directories use the platform's atomic no-replace rename.
pub(crate) fn publish_no_replace(staged: &Path, target: &Path) -> FsStep {
    let metadata = match fs::symlink_metadata(staged) {
        Ok(metadata) => metadata,
        Err(_) => return FsStep::Failed,
    };
    if metadata.is_dir() {
        return move_no_replace(staged, target);
    }
    // File: link(2) is the portable no-clobber create — EEXIST when the
    // destination exists on any POSIX filesystem.
    #[cfg(test)]
    if let Some(errno) =
        super::tests::fault_injection::prim(super::tests::fault_injection::Prim::HardLink)
    {
        return match errno {
            nix::libc::EEXIST => FsStep::Conflict,
            _ => FsStep::Failed,
        };
    }
    match fs::hard_link(staged, target) {
        Ok(()) => match fs::remove_file(staged) {
            Ok(()) => FsStep::Done,
            Err(_) => FsStep::Done, // published; the staged link is residue the caller lists
        },
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => FsStep::Conflict,
        Err(error) if error.raw_os_error() == Some(nix::libc::EEXIST) => FsStep::Conflict,
        Err(_) => FsStep::Failed,
    }
}

/// `renameat2(RENAME_NOREPLACE)` on Linux / `renameatx_np(RENAME_EXCL)` on
/// Apple — rustix carries the unsafe, this crate carries none. Platforms
/// without a no-replace primitive answer a typed failure, never a silent
/// `rename`.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn move_no_replace(source: &Path, target: &Path) -> FsStep {
    use rustix::fs::{CWD, Mode, OFlags, RenameFlags, openat, renameat_with};
    let open_parent = |path: &Path| {
        let directory = openat(
            CWD,
            path.parent()?,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .ok()?;
        Some((directory, path.file_name()?.to_os_string()))
    };
    let (Some((source_dir, source_name)), Some((target_dir, target_name))) =
        (open_parent(source), open_parent(target))
    else {
        return FsStep::Failed;
    };
    #[cfg(test)]
    if let Some(errno) =
        super::tests::fault_injection::prim(super::tests::fault_injection::Prim::RenameNoReplace)
    {
        return match errno {
            nix::libc::EEXIST | nix::libc::ENOTEMPTY => FsStep::Conflict,
            _ => FsStep::Failed,
        };
    }
    match renameat_with(
        &source_dir,
        source_name,
        &target_dir,
        target_name,
        RenameFlags::NOREPLACE,
    ) {
        Ok(()) => FsStep::Done,
        Err(rustix::io::Errno::EXIST) | Err(rustix::io::Errno::NOTEMPTY) => FsStep::Conflict,
        Err(_) => FsStep::Failed,
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn move_no_replace(_source: &Path, _target: &Path) -> FsStep {
    FsStep::Failed
}

/// Move `target` aside to a fresh `.orchbee-agent-backup-*` sibling using the
/// same no-replace primitive — a bounded fresh-name retry, and exhaustion is
/// a typed conflict that destroys nothing.
pub(crate) fn move_aside(target: &Path) -> Result<Option<PathBuf>, FsStep> {
    match fs::symlink_metadata(target) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(FsStep::Failed),
        Ok(_) => {}
    }
    let parent = target.parent().ok_or(FsStep::Failed)?;
    for attempt in 0..BACKUP_ATTEMPTS {
        let backup = parent.join(format!(
            ".orchbee-agent-backup-{:x}{:x}",
            std::process::id(),
            nonce(attempt),
        ));
        #[cfg(test)]
        super::tests::fault_injection::boundary("move-aside", &backup);
        // rename(2) here is safe by construction: the destination is a fresh
        // nonce name, so a same-instant attacker landing that exact name is
        // the residual — bounded retries keep it vanishingly small, and the
        // primitive is still the platform no-replace move where available.
        match move_no_replace(target, &backup) {
            FsStep::Done => return Ok(Some(backup)),
            FsStep::Conflict => continue,
            step => return Err(step),
        }
    }
    Err(FsStep::Conflict)
}

/// Restore a moved-aside backup back over the (still-absent) target —
/// no-replace again: a foreign arrival during the gap survives and the
/// backup is retained and named by the caller.
pub(crate) fn restore_backup(backup: &Path, target: &Path) -> FsStep {
    move_no_replace(backup, target)
}

/// One stable digest for a managed entry — a file's sha256, or the
/// canonical fingerprint digest of a directory tree. Backs the
/// digest-guarded backup removal: removal proceeds only while the captured
/// inspect-time digest still answers.
pub(crate) fn entry_digest(path: &Path) -> Option<String> {
    use sha2::Digest;
    let metadata = fs::symlink_metadata(path).ok()?;
    if metadata.file_type().is_symlink() {
        // A link's bytes are its target — a moved-aside symlink backup
        // verifies against exactly the string it still points at.
        let linked = fs::read_link(path).ok()?;
        return Some(format!(
            "{:x}",
            sha2::Sha256::digest(linked.as_os_str().as_encoded_bytes())
        ));
    }
    if metadata.is_file() {
        let bytes = fs::read(path).ok()?;
        return Some(format!("{:x}", sha2::Sha256::digest(&bytes)));
    }
    if metadata.is_dir() {
        let mut files = super::inspect::fingerprint_directory(path)?;
        // The walk order is not stable — sort by path so the digest is.
        files.sort();
        let mut hasher = sha2::Sha256::new();
        for (relative, digest) in files {
            hasher.update(relative.as_bytes());
            hasher.update(b"\0");
            hasher.update(digest.as_bytes());
            hasher.update(b"\0");
        }
        return Some(format!("{:x}", hasher.finalize()));
    }
    None
}

/// Remove one verified-owned target: a file/symlink unlinks; a directory is
/// removed through an `O_NOFOLLOW` recursive walk so a swapped symlink can
/// never redirect the removal. Mid-walk type changes are typed conflicts.
pub(crate) fn remove_owned(target: &Path, is_directory: bool) -> FsStep {
    match fs::symlink_metadata(target) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return FsStep::Done,
        Err(_) => return FsStep::Failed,
        Ok(metadata) => {
            if metadata.is_dir() && !is_directory {
                return FsStep::Conflict;
            }
        }
    }
    if is_directory {
        match remove_directory_no_follow(target) {
            Ok(()) => FsStep::Done,
            // A mid-walk type change (swapped symlink → ELOOP, swapped
            // file → ENOTDIR) or a foreign arrival is a typed conflict:
            // the walk refused to follow and the partial subtree stands.
            Err(error)
                if error.kind() == io::ErrorKind::AlreadyExists
                    || matches!(
                        error.raw_os_error(),
                        Some(nix::libc::ELOOP | nix::libc::ENOTDIR)
                    ) =>
            {
                FsStep::Conflict
            }
            Err(_) => FsStep::Failed,
        }
    } else {
        #[cfg(test)]
        if let Some(errno) =
            super::tests::fault_injection::prim(super::tests::fault_injection::Prim::UnlinkFile)
        {
            return match errno {
                nix::libc::ENOENT => FsStep::Done,
                _ => FsStep::Failed,
            };
        }
        match fs::remove_file(target) {
            Ok(()) => FsStep::Done,
            Err(error) if error.kind() == io::ErrorKind::NotFound => FsStep::Done,
            Err(_) => FsStep::Failed,
        }
    }
}

/// Recursive removal that never follows a symlink: every level is opened
/// `O_DIRECTORY|O_NOFOLLOW` relative to its parent fd, so a swapped symlink
/// can only produce a typed failure, never a redirected delete.
fn remove_directory_no_follow(target: &Path) -> io::Result<()> {
    use nix::dir::Dir;
    use nix::fcntl::{OFlag, openat};
    use nix::sys::stat::Mode;
    use nix::unistd::{UnlinkatFlags, unlinkat};

    let parent = target
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "target has no parent"))?;
    let name = target
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "target has no name"))?;
    let parent_fd = openat(
        nix::fcntl::AT_FDCWD,
        parent,
        OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
        Mode::empty(),
    )
    .map_err(io::Error::from)?;
    // The root itself must still be a real directory — a mid-window swap to
    // a symlink or file fails here, and removal is refused.
    let mut dir = Dir::openat(
        &parent_fd,
        name,
        OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
        Mode::empty(),
    )
    .map_err(io::Error::from)?;
    remove_contents(&mut dir)?;
    unlinkat(&parent_fd, name, UnlinkatFlags::RemoveDir).map_err(io::Error::from)
}

fn remove_contents(dir: &mut nix::dir::Dir) -> io::Result<()> {
    use std::os::fd::AsFd;

    use nix::dir::Dir;
    use nix::fcntl::OFlag;
    use nix::sys::stat::Mode;
    use nix::unistd::{UnlinkatFlags, unlinkat};

    // `iter` borrows the directory mutably — collect entries first so the
    // fd can serve openat/unlinkat below.
    use std::os::unix::ffi::OsStrExt;
    let mut entries = Vec::new();
    for entry in dir.iter() {
        let entry = entry.map_err(io::Error::from)?;
        entries.push((
            std::ffi::OsStr::from_bytes(entry.file_name().to_bytes()).to_os_string(),
            entry.file_type(),
        ));
    }
    for (name, file_type) in entries {
        let name = name.as_os_str();
        if name.as_encoded_bytes() == b"." || name.as_encoded_bytes() == b".." {
            continue;
        }
        // Test seam: the mid-walk boundary lets a harness swap the component
        // before this level re-inspects it — the O_NOFOLLOW open must still
        // refuse a symlink.
        #[cfg(test)]
        super::tests::fault_injection::boundary("remove-dir-walk", Path::new(name));
        if file_type == Some(nix::dir::Type::Directory) {
            // Open the child no-follow relative to this level: a symlink or
            // non-directory here fails typed instead of being followed.
            #[cfg(test)]
            if let Some(errno) = super::tests::fault_injection::prim(
                super::tests::fault_injection::Prim::WalkOpenDir,
            ) {
                return Err(super::tests::fault_injection::errno_error(errno));
            }
            let mut sub = Dir::openat(
                dir.as_fd(),
                name,
                OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            )
            .map_err(io::Error::from)?;
            remove_contents(&mut sub)?;
            #[cfg(test)]
            if let Some(errno) =
                super::tests::fault_injection::prim(super::tests::fault_injection::Prim::WalkUnlink)
            {
                return Err(super::tests::fault_injection::errno_error(errno));
            }
            unlinkat(dir.as_fd(), name, UnlinkatFlags::RemoveDir).map_err(io::Error::from)?;
        } else {
            // Files, symlinks, and any other kind unlink by name — a link's
            // target is never followed.
            #[cfg(test)]
            if let Some(errno) =
                super::tests::fault_injection::prim(super::tests::fault_injection::Prim::WalkUnlink)
            {
                return Err(super::tests::fault_injection::errno_error(errno));
            }
            unlinkat(dir.as_fd(), name, UnlinkatFlags::NoRemoveDir).map_err(io::Error::from)?;
        }
    }
    Ok(())
}
