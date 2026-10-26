//! Refines descriptor admission to supported ext4 or tmpfs objects and rejects extended access
//! ACLs beyond private mode bits. It does not create, repair, or remove a caller's filesystem
//! object.

use nix::sys::statfs::{EXT4_SUPER_MAGIC, TMPFS_MAGIC, fstatfs};
use std::{io, os::fd::AsFd};

/// The managed development boundary claims only local POSIX-ACL filesystem semantics.
pub(super) fn require_private(fd: &impl AsFd) -> io::Result<()> {
    let filesystem = fstatfs(fd)?.filesystem_type();
    if filesystem != EXT4_SUPER_MAGIC && filesystem != TMPFS_MAGIC {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "native development runtime requires ext4 or tmpfs",
        ));
    }
    for name in ["system.posix_acl_access", "system.posix_acl_default"] {
        let mut probe = [0_u8; 1];
        match rustix::fs::fgetxattr(fd, name, &mut probe[..]) {
            Err(rustix::io::Errno::NODATA) => {}
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "native development runtime rejects extended ACLs",
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
pub(super) mod tests;
