use crate::{ModeCheckedLocalSessionRendezvous, SessionScope};
use std::{
    fs::{self, File},
    os::unix::fs::PermissionsExt,
};

pub(crate) fn named_acl(owner_permissions: u16) -> Vec<u8> {
    let mut bytes = 2_u32.to_le_bytes().to_vec();
    for (tag, permissions, id) in [
        (1_u16, owner_permissions, u32::MAX),
        (2, 7, nix::unistd::geteuid().as_raw() + 1),
        (4, 0, u32::MAX),
        (16, 0, u32::MAX),
        (32, 0, u32::MAX),
    ] {
        bytes.extend(tag.to_le_bytes());
        bytes.extend(permissions.to_le_bytes());
        bytes.extend(id.to_le_bytes());
    }
    bytes
}

#[test]
fn exact_mode_does_not_hide_named_or_default_acls_from_runtime_admission() {
    let _guard = crate::rendezvous::tests::FILESYSTEM_TEST_LOCK
        .lock()
        .unwrap();
    let base = std::env::temp_dir().join(format!("ob-acl-{}", std::process::id()));
    fs::create_dir(&base).unwrap();
    fs::set_permissions(&base, fs::Permissions::from_mode(0o700)).unwrap();
    for attribute in ["system.posix_acl_access", "system.posix_acl_default"] {
        let scope = SessionScope::parse(if attribute.ends_with("access") {
            "orchbee-access"
        } else {
            "orchbee-default"
        })
        .unwrap();
        let session =
            ModeCheckedLocalSessionRendezvous::prepare_mode_checked(&base, scope.clone()).unwrap();
        let file = File::open(session.runtime_directory()).unwrap();
        rustix::fs::fsetxattr(
            &file,
            attribute,
            &named_acl(7),
            rustix::fs::XattrFlags::empty(),
        )
        .unwrap();
        assert_eq!(
            file.metadata().unwrap().permissions().mode() & 0o7777,
            0o700
        );
        assert_eq!(
            super::require_private(&file).unwrap_err().kind(),
            std::io::ErrorKind::PermissionDenied
        );
        assert!(ModeCheckedLocalSessionRendezvous::validate_mode_checked(&base, scope).is_err());
    }
    let session = ModeCheckedLocalSessionRendezvous::prepare_mode_checked(
        &base,
        SessionScope::parse("orchbee-files").unwrap(),
    )
    .unwrap();
    drop(session.try_acquire_activation_lock().unwrap());
    session.commit_activation().unwrap();
    for name in ["activation.lock", "activation.started"] {
        let file = File::open(session.runtime_directory().join(name)).unwrap();
        rustix::fs::fsetxattr(
            &file,
            "system.posix_acl_access",
            &named_acl(6),
            rustix::fs::XattrFlags::empty(),
        )
        .unwrap();
        assert_eq!(
            file.metadata().unwrap().permissions().mode() & 0o7777,
            0o600
        );
    }
    assert!(session.try_acquire_activation_lock().is_err());
    assert!(session.activation_started().is_err());
    fs::remove_dir_all(&base).unwrap();
}
