//! Admits macOS objects only on volumes that enforce file ownership and only without extended
//! ACLs, checking pathname identity against the retained descriptor around the native ACL lookup.
//! It does not repair permissions or provide path stability after the admission call.

use nix::{
    mount::MntFlags,
    sys::{stat::fstat, statfs::fstatfs},
};
use std::{fs, io, os::fd::AsFd, os::unix::fs::MetadataExt, path::Path};

/// A volume mounted with "ignore ownership" reports every file as the caller's, so neither the
/// owner nor the mode bits keep other users out. macOS ACL entries can grant access beyond the
/// POSIX mode bits, so only objects without an extended ACL are admitted. The native ACL API is
/// pathname-only, so the path is checked against the descriptor identity both before and after
/// the lookup.
pub(crate) fn require_private(fd: &impl AsFd, path: &Path) -> io::Result<()> {
    if fstatfs(fd.as_fd())?
        .flags()
        .contains(MntFlags::MNT_IGNORE_OWNERSHIP)
    {
        return Err(crate::AdmissionRefusal::io(
            crate::RefusalReason::PermissionsNotKept,
            path,
        ));
    }
    let descriptor_status = fstat(fd.as_fd())?;
    let before = fs::symlink_metadata(path)?;
    if !matches_descriptor(&before, &descriptor_status) {
        return Err(changed_path());
    }
    let acl = exacl::getfacl(path, None::<exacl::AclOption>)?;
    let after = fs::symlink_metadata(path)?;
    if !matches_descriptor(&after, &descriptor_status) {
        return Err(changed_path());
    }
    if !acl.is_empty() {
        return Err(crate::AdmissionRefusal::io(
            crate::RefusalReason::AccessControlList,
            path,
        ));
    }
    Ok(())
}

fn matches_descriptor(metadata: &fs::Metadata, status: &nix::sys::stat::FileStat) -> bool {
    u64::try_from(status.st_dev).is_ok_and(|device| device == metadata.dev())
        && status.st_ino == metadata.ino()
}

fn changed_path() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "native development runtime path changed while validating ACLs",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use exacl::{AclEntry, Perm};
    use std::{
        fs::{self, File, OpenOptions},
        os::unix::fs::{OpenOptionsExt, PermissionsExt},
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = crate::test_temporary_directory().join(format!(
                "orchbee-macos-acl-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
            Self(path)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _removed = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn descriptor_acl_probe_accepts_acl_free_objects() {
        let fixture = Fixture::new();
        let file_path = fixture.0.join("artifact");
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o700)
            .open(&file_path)
            .unwrap();
        require_private(&File::open(&fixture.0).unwrap(), &fixture.0).unwrap();
        require_private(&File::open(&file_path).unwrap(), &file_path).unwrap();
    }

    #[test]
    fn descriptor_acl_probe_rejects_an_extended_acl() {
        let fixture = Fixture::new();
        let file_path = fixture.0.join("artifact");
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o700)
            .open(&file_path)
            .unwrap();
        let current_user = nix::unistd::geteuid().to_string();
        exacl::setfacl(
            &[&file_path],
            &[AclEntry::allow_user(&current_user, Perm::READ, None)],
            None::<exacl::AclOption>,
        )
        .unwrap();
        assert_eq!(
            require_private(&file, &file_path).unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
    }
}
