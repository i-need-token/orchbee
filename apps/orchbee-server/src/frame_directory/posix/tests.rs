//! Descriptor traces for private creation, owned single-link copies, no adoption and retryable
//! retirement on a real POSIX file system.
use super::*;
use std::{
    fs::{self, DirBuilder},
    os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt, symlink},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(1);

/// A private runtime under the canonical temporary folder, since `/tmp` and `/var` are links on
/// macOS and admission never follows a link.
struct Runtime {
    base: PathBuf,
    path: PathBuf,
}
impl Runtime {
    fn new() -> Self {
        let base = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "orchbee-frame-directory-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let path = base.join("runtime");
        DirBuilder::new()
            .mode(0o700)
            .recursive(true)
            .create(&path)
            .unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        Self { base, path }
    }
    fn reserve(&self, id: u128) -> PrivateFrameDirectory {
        PrivateFrameDirectory::reserve(&self.path, &format!("{id:032x}")).unwrap()
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

fn denied(result: io::Result<Vec<u8>>) -> bool {
    matches!(result, Err(error) if error.kind() == io::ErrorKind::PermissionDenied)
}

#[test]
fn frame_directory_creates_a_private_root_and_copies_only_owned_single_link_children() {
    let runtime = Runtime::new();
    let mut root = runtime.reserve(1);
    assert_eq!(
        root.path(),
        runtime.path.join(format!("graphics-{:032x}", 1))
    );
    assert!(!root.path().exists());
    assert!(matches!(
        root.read_frame(&root.path().join("frame"), 4),
        Err(error) if error.kind() == io::ErrorKind::NotConnected
    ));
    root.create(br#"{"owner":"test"}"#).unwrap();
    assert_eq!(
        fs::symlink_metadata(root.path()).unwrap().mode() & 0o7777,
        0o700
    );
    let owner = root.path().join(OWNER);
    assert_eq!(fs::symlink_metadata(&owner).unwrap().mode() & 0o7777, 0o600);
    assert_eq!(fs::read(&owner).unwrap(), br#"{"owner":"test"}"#);

    // A producer writes with its own umask; the private leaf keeps other users out.
    let frame = root.path().join("帧.rgba");
    fs::write(&frame, [0, 10, 255, 128]).unwrap();
    fs::set_permissions(&frame, fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(root.read_frame(&frame, 4).unwrap(), [0, 10, 255, 128]);
    assert!(matches!(
        root.read_frame(&frame, 3),
        Err(error) if error.kind() == io::ErrorKind::InvalidData
    ));
    assert!(matches!(
        root.read_frame(&frame, FRAME_MAXIMUM_BYTES + 1),
        Err(error) if error.kind() == io::ErrorKind::FileTooLarge
    ));
    for path in [
        runtime.path.join("帧.rgba"),
        root.path().join("nested").join("帧.rgba"),
        PathBuf::from(format!("{}/../帧.rgba", root.path().display())),
        root.path().join(OWNER),
        root.path().join(OWNER.to_ascii_uppercase()),
    ] {
        assert!(matches!(
            root.read_frame(&path, 4),
            Err(error) if error.kind() == io::ErrorKind::InvalidInput
        ));
    }

    let outside = runtime.base.join("outside.rgba");
    fs::write(&outside, [1, 2, 3, 4]).unwrap();
    let link = root.path().join("link.rgba");
    symlink(&outside, &link).unwrap();
    assert!(denied(root.read_frame(&link, 4)));
    let hard = root.path().join("hard.rgba");
    fs::hard_link(&outside, &hard).unwrap();
    assert!(denied(root.read_frame(&hard, 4)));
    fs::create_dir(root.path().join("folder")).unwrap();
    assert!(denied(root.read_frame(&root.path().join("folder"), 4)));
    let fifo = root.path().join("fifo");
    nix::unistd::mkfifo(&fifo, Mode::from_bits_truncate(0o600)).unwrap();
    // The open never waits for a writer.
    assert!(denied(root.read_frame(&fifo, 4)));

    root.reap().unwrap();
    assert!(!root.path().exists());
    assert_eq!(fs::read(&outside).unwrap(), [1, 2, 3, 4]);
}

#[test]
fn frame_directory_never_adopts_or_follows_an_existing_leaf_or_unsafe_runtime() {
    let runtime = Runtime::new();
    let mut existing = runtime.reserve(2);
    fs::create_dir(existing.path()).unwrap();
    assert!(existing.create(b"{}").is_err());
    assert!(!existing.path().join(OWNER).exists());

    let mut linked = runtime.reserve(3);
    let elsewhere = runtime.base.join("elsewhere");
    DirBuilder::new().mode(0o700).create(&elsewhere).unwrap();
    symlink(&elsewhere, linked.path()).unwrap();
    assert!(linked.create(b"{}").is_err());
    assert!(!elsewhere.join(OWNER).exists());
    assert!(fs::symlink_metadata(linked.path()).unwrap().is_symlink());

    let shared = runtime.base.join("shared");
    DirBuilder::new().mode(0o755).create(&shared).unwrap();
    fs::set_permissions(&shared, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(PrivateFrameDirectory::reserve(&shared, &format!("{:032x}", 4)).is_err());
    let alias = runtime.base.join("alias");
    symlink(&runtime.path, &alias).unwrap();
    assert!(PrivateFrameDirectory::reserve(&alias, &format!("{:032x}", 5)).is_err());
    for nonce in ["0".repeat(32), "A".repeat(32), "1".repeat(31)] {
        assert!(PrivateFrameDirectory::reserve(&runtime.path, &nonce).is_err());
    }
}

#[test]
fn frame_directory_reap_removes_nested_contents_and_retains_a_retryable_owner() {
    let runtime = Runtime::new();
    let mut root = runtime.reserve(6);
    root.create(b"{\"release\":\"retry\"}").unwrap();
    let nested = root.path().join("a").join("b");
    fs::create_dir_all(&nested).unwrap();
    fs::write(nested.join("frame"), [9]).unwrap();
    let outside = runtime.base.join("kept");
    DirBuilder::new().mode(0o700).create(&outside).unwrap();
    fs::write(outside.join("file"), [7]).unwrap();
    symlink(&outside, root.path().join("a").join("escape")).unwrap();

    // A folder this user cannot change fails the first pass, which keeps the owner note.
    if !geteuid().is_root() {
        fs::set_permissions(root.path().join("a"), fs::Permissions::from_mode(0o500)).unwrap();
        assert!(root.reap().is_err());
        assert!(root.path().join(OWNER).exists());
        fs::set_permissions(root.path().join("a"), fs::Permissions::from_mode(0o700)).unwrap();
    }
    root.reap().unwrap();
    assert!(!root.path().exists());
    assert_eq!(fs::read(outside.join("file")).unwrap(), [7]);
    root.reap().unwrap();

    // A different folder now at the leaf name is not this owner's to remove.
    let mut replaced = runtime.reserve(7);
    replaced.create(b"{}").unwrap();
    let moved = runtime.path.join("moved");
    fs::rename(replaced.path(), &moved).unwrap();
    DirBuilder::new()
        .mode(0o700)
        .create(replaced.path())
        .unwrap();
    assert!(replaced.reap().is_err());
    assert!(replaced.path().exists());
    assert!(moved.join(OWNER).exists());
}
