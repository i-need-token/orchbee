// Removal deletes files in a folder the Terminal Host shares, so these tests prove its boundaries on
// real files: the exact name scheme, the strict one-day age, and that no link leads it elsewhere.
use super::{RETENTION, remove_stale_editor_documents};
use crate::private_filesystem as permissions;
use std::{
    fs,
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

struct Folder(PathBuf);
impl Folder {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "orchbee-editor-documents-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir(&path).expect("create the test folder");
        let mut directory = permissions::open_directory(&path, true).expect("open the test folder");
        permissions::protect_new(&mut directory).expect("protect the test folder");
        Self(path)
    }

    fn names(&self) -> Vec<String> {
        let mut names: Vec<_> = fs::read_dir(&self.0)
            .expect("list the test folder")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .into_string()
                    .expect("name")
            })
            .collect();
        names.sort();
        names
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn written(path: &Path, modified: SystemTime) {
    fs::write(path, b"scrollback").expect("write a document");
    fs::File::options()
        .write(true)
        .open(path)
        .and_then(|file| file.set_modified(modified))
        .expect("set the modified time");
}

/// A junction at `link` to `target`, which needs no symbolic-link privilege.
fn junction(link: &Path, target: &Path) {
    let status = std::process::Command::new("powershell.exe")
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "New-Item -ItemType Junction -Path $env:ORCHBEE_FIXTURE_LINK -Value $env:ORCHBEE_FIXTURE_TARGET -ErrorAction Stop | Out-Null",
        ])
        .env("ORCHBEE_FIXTURE_LINK", link)
        .env("ORCHBEE_FIXTURE_TARGET", target)
        .creation_flags(0x0800_0000)
        .status()
        .expect("start PowerShell");
    assert!(status.success(), "create the junction");
}

#[test]
fn removal_takes_only_documents_older_than_a_day_and_follows_no_link() {
    let folder = Folder::new("rule");
    let outside = Folder::new("outside");
    let now = SystemTime::now();
    let day = RETENTION;
    let old = now - day * 2;
    for name in [
        "scrollback-2026-11-01T10-00-00Z.txt",
        "scrollback-2026-11-01T10-00-00Z-2.txt",
        "scrollback-2026-11-01T10-00-00Z-17.txt",
    ] {
        written(&folder.0.join(name), old);
    }
    written(
        &folder.0.join("scrollback-2026-11-06T09-00-00Z.txt"),
        now - day - Duration::from_secs(1),
    );
    written(
        &folder.0.join("scrollback-2026-11-06T10-00-00Z.txt"),
        now - day,
    );
    written(
        &folder.0.join("scrollback-2026-11-08T09-00-00Z.txt"),
        now - Duration::from_secs(3600),
    );
    written(
        &folder.0.join("scrollback-2026-11-09T09-00-00Z.txt"),
        now + Duration::from_secs(3600),
    );
    let foreign = [
        "scrollback-2026-11-01T10-00-00Z-1.txt",
        "scrollback-2026-11-01T10-00-00Z-02.txt",
        "scrollback-2026-02-30T10-00-00Z.txt",
        "scrollback-2026-11-01T10-00-00Z.txt.swp",
        ".scrollback-2026-11-01T10-00-00Z.txt.swp",
        "Scrollback-2026-10-01T10-00-00Z.TXT",
        "scrollback-2026-11-01T10-00-00Z",
        "notes.txt",
    ];
    for name in foreign {
        written(&folder.0.join(name), old);
    }
    fs::create_dir(folder.0.join("scrollback-2026-11-02T10-00-00Z.txt")).expect("directory");
    let target = outside.0.join("scrollback-2026-11-03T10-00-00Z.txt");
    fs::create_dir(&target).expect("link target");
    written(&target.join("scrollback-2026-11-03T10-00-00Z.txt"), old);
    junction(
        &folder.0.join("scrollback-2026-11-03T10-00-00Z.txt"),
        &target,
    );

    let removal = remove_stale_editor_documents(&folder.0, now).expect("remove");

    assert_eq!((removal.removed, removal.failed), (4, 0));
    let mut kept: Vec<_> = foreign.iter().map(|name| (*name).to_owned()).collect();
    kept.extend(
        [
            "scrollback-2026-11-02T10-00-00Z.txt",
            "scrollback-2026-11-03T10-00-00Z.txt",
            "scrollback-2026-11-06T10-00-00Z.txt",
            "scrollback-2026-11-08T09-00-00Z.txt",
            "scrollback-2026-11-09T09-00-00Z.txt",
        ]
        .map(str::to_owned),
    );
    kept.sort();
    assert_eq!(folder.names(), kept);
    assert!(target.join("scrollback-2026-11-03T10-00-00Z.txt").is_file());
    // Remove the junction itself before recursive fixture cleanup.
    fs::remove_dir(folder.0.join("scrollback-2026-11-03T10-00-00Z.txt")).expect("unlink");
}

#[test]
fn removal_refuses_a_linked_folder_and_never_creates_a_missing_one() {
    let root = Folder::new("folder");
    let missing = root.0.join("tmp");
    let removal = remove_stale_editor_documents(&missing, SystemTime::now()).expect("missing");
    assert_eq!(removal, super::EditorDocumentRemoval::default());
    assert!(!missing.exists());

    let target = root.0.join("target");
    fs::create_dir(&target).expect("link target");
    let document = target.join("scrollback-2026-11-01T10-00-00Z.txt");
    written(&document, SystemTime::now() - RETENTION * 2);
    junction(&missing, &target);
    assert!(remove_stale_editor_documents(&missing, SystemTime::now()).is_err());
    assert!(document.is_file());
    fs::remove_dir(&missing).expect("unlink");
}
