//! Safe boundary tests of the Recycle Bin adapter through the real shell. None of them moves
//! anything into the Recycle Bin: each item stays in a scratch folder the test removes.

use std::{
    ffi::OsString,
    fs,
    os::windows::fs::OpenOptionsExt,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
use windows::Win32::UI::Shell::FOFX_RECYCLEONDELETE;

use super::{NotRecycled, RECYCLE, delete, recycle};

/// `FILE_SHARE_READ` alone: no other handle may delete or rename the file.
const SHARE_READ: u32 = 0x1;

/// A folder of the test's own under the temporary folder, removed with what is left in it.
struct Scratch(PathBuf);

impl Scratch {
    fn new(purpose: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "orchbee-files-recycle-bin-{purpose}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Without `FOFX_RECYCLEONDELETE` the shell deletes permanently, and its `PreDeleteItem` then lacks
/// `TSF_DELETE_RECYCLE_IF_POSSIBLE`, as when it cannot recycle. The refusal keeps a file and a
/// folder with its content, and answers what a volume without a Recycle Bin answers.
#[test]
fn a_deletion_the_shell_would_make_permanent_is_refused_and_keeps_the_item() {
    let scratch = Scratch::new("permanent");
    let file = scratch.0.join("file.txt");
    fs::write(&file, b"kept").unwrap();
    let folder = scratch.0.join("folder");
    fs::create_dir(&folder).unwrap();
    fs::write(folder.join("inner.txt"), b"kept").unwrap();

    for item in [&file, &folder] {
        assert_eq!(
            delete(item, RECYCLE & !FOFX_RECYCLEONDELETE),
            Err(NotRecycled::Unavailable)
        );
    }
    assert_eq!(fs::read(&file).unwrap(), b"kept");
    assert_eq!(fs::read(folder.join("inner.txt")).unwrap(), b"kept");
}

/// The shell cannot move a file another handle holds without delete sharing: the move fails,
/// without a window, and the file stays.
#[test]
fn a_move_the_shell_fails_answers_failed_and_keeps_the_item() {
    let scratch = Scratch::new("held");
    let file = scratch.0.join("held.txt");
    fs::write(&file, b"kept").unwrap();
    let held = fs::OpenOptions::new()
        .read(true)
        .share_mode(SHARE_READ)
        .open(&file)
        .unwrap();

    assert_eq!(recycle(&file), Err(NotRecycled::Failed));
    drop(held);
    assert_eq!(fs::read(&file).unwrap(), b"kept");
}

/// A NUL would end the path the shell parses at the scratch folder, so such a path never reaches
/// the shell.
#[test]
fn a_path_with_a_nul_never_reaches_the_shell() {
    let scratch = Scratch::new("nul");
    fs::write(scratch.0.join("inner.txt"), b"kept").unwrap();
    let mut cut = OsString::from(&scratch.0);
    cut.push("\0inner.txt");

    assert_eq!(recycle(&PathBuf::from(cut)), Err(NotRecycled::Failed));
    assert_eq!(fs::read(scratch.0.join("inner.txt")).unwrap(), b"kept");
}
