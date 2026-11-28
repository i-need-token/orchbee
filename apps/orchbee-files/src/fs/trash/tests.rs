//! Where the Recycle Bin is asked to move an entry from.

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use super::located;
use crate::fs::tests::Scratch;

/// Links `link` to the folder `target` with a junction, which needs no privilege.
fn junction(target: &Path, link: &Path) {
    let status = Command::new("cmd")
        .args(["/d", "/c", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .stdout(Stdio::null())
        .status()
        .unwrap();
    assert!(status.success());
}

/// The real location of `folder` in the drive form the shell parses.
fn real(folder: &Path) -> PathBuf {
    let real = fs::canonicalize(folder).unwrap();
    PathBuf::from(real.to_str().unwrap().strip_prefix(r"\\?\").unwrap())
}

/// A junction on the way resolves to the folder it names, as scope checked it, while the entry,
/// itself a junction, stays as named: the Recycle Bin then moves the junction and leaves its
/// target where it is.
#[test]
fn an_entry_is_named_below_its_folders_real_location_and_never_resolved_itself() {
    let scratch = Scratch::new("trash-located");
    let folder = scratch.path().join("folder");
    let target = folder.join("target");
    fs::create_dir_all(&target).unwrap();
    junction(&target, &folder.join("link"));
    junction(&folder, &scratch.path().join("alias"));

    assert_eq!(
        located(&scratch.path().join("alias").join("link")),
        real(&folder).join("link")
    );
}
