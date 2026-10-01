//! Native handle traces for bounded private copies, exclusive ownership and retryable retirement.
use super::*;
use crate::WindowsPrivateCache;
use std::os::windows::fs::OpenOptionsExt;

struct Runtime {
    path: PathBuf,
    cache: Option<WindowsPrivateCache>,
}
impl Runtime {
    fn new() -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("orchbee-frame-directory-{nonce:032x}"));
        let cache = WindowsPrivateCache::acquire(&path).unwrap();
        Self {
            path,
            cache: Some(cache),
        }
    }
    fn reserve(&self, id: u128) -> WindowsPrivateFrameDirectory {
        WindowsPrivateFrameDirectory::reserve(&self.path, &format!("{id:032x}")).unwrap()
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        self.cache.take();
        let actual = self.path.canonicalize().unwrap();
        assert_eq!(
            actual.parent(),
            Some(std::env::temp_dir().canonicalize().unwrap().as_path())
        );
        assert!(
            actual
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("orchbee-frame-directory-")
        );
        fs::remove_dir_all(actual).unwrap();
    }
}

#[test]
fn frame_directory_private_snapshot_preserves_binary_bytes_and_rejects_paths_links_lengths_and_writers()
 {
    let runtime = Runtime::new();
    let mut root = runtime.reserve(1);
    assert!(!root.path().exists());
    assert!(root.read_frame(&root.path().join("frame"), 4).is_err());
    root.create(br#"{"owner":"test","release":"last reader"}"#)
        .unwrap();
    let path = root.path().join("帧.rgba");
    fs::write(&path, [0, 10, 255, 128]).unwrap();
    assert_eq!(root.read_frame(&path, 4).unwrap(), [0, 10, 255, 128]);
    for length in [0, 3, 5, WINDOWS_FRAME_MAXIMUM_BYTES + 1] {
        assert!(root.read_frame(&path, length).is_err());
    }
    for path in [
        runtime.path.join("outside"),
        root.path().join("nested/../帧.rgba"),
        root.path().join("帧.rgba:stream"),
        root.path().join("CON"),
        root.path().join(".ORCHBEE-FRAME-OWNER.JSON"),
    ] {
        assert!(root.read_frame(&path, 4).is_err());
    }
    let writer = std::fs::OpenOptions::new()
        .write(true)
        .share_mode(3)
        .open(&path)
        .unwrap();
    assert!(root.read_frame(&path, 4).is_err());
    drop(writer);
    assert_eq!(root.read_frame(&path, 4).unwrap(), [0, 10, 255, 128]);
    assert!(fs::rename(root.path(), runtime.path.join("moved")).is_err());
    let link = root.path().join("hard.rgba");
    fs::hard_link(&path, &link).unwrap();
    assert!(root.read_frame(&path, 4).is_err());
    assert!(root.read_frame(&link, 4).is_err());
    root.reap().unwrap();
    assert!(!root.path().exists());
    root.reap().unwrap();
}

#[test]
fn frame_directory_does_not_adopt_existing_roots_and_keeps_failed_delete_retryable() {
    let runtime = Runtime::new();
    let mut root = runtime.reserve(2);
    root.create(b"owner").unwrap();
    let mut contender = runtime.reserve(2);
    assert_eq!(
        contender.create(b"foreign").unwrap_err().kind(),
        io::ErrorKind::AlreadyExists
    );
    contender.reap().unwrap();
    assert!(root.path().exists());
    let path = root.path().join("busy.rgba");
    fs::write(&path, [1, 2, 3, 4]).unwrap();
    let blocker = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(3)
        .open(&path)
        .unwrap();
    assert!(root.reap().is_err());
    assert!(root.path().exists());
    assert!(root.read_frame(&path, 4).is_err());
    assert_eq!(fs::read(root.path().join(OWNER)).unwrap(), b"owner");
    drop(blocker);
    let directory_blocker = permissions::open_directory(root.path(), false).unwrap();
    assert!(root.reap().is_err());
    assert_eq!(fs::read(root.path().join(OWNER)).unwrap(), b"owner");
    drop(directory_blocker);
    root.reap().unwrap();
    assert!(!root.path().exists());
    for nonce in [
        "00000000000000000000000000000000",
        "../bad",
        "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF",
    ] {
        assert!(WindowsPrivateFrameDirectory::reserve(&runtime.path, nonce).is_err());
    }
}
