//! Bounded disk on a scratch store: after a completion moves the pointer, only the versions that are
//! neither active nor previous and from which nothing runs leave the store, and what cannot go now
//! goes with a later completion.
use super::super::{Error, complete, install::Completion, running_programs};
use super::install_fixture::{Fixture, entry_for, pointer};
use crate::release_store::ReleaseStore;
use orchbee_release_records::Pointer;
use std::{
    fs::{self, File, OpenOptions},
    os::windows::fs::OpenOptionsExt,
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

/// `FILE_SHARE_READ | FILE_SHARE_WRITE`: a handle that refuses a rename or deletion.
const SHARE_READ_WRITE: u32 = 3;

/// A stand-in process started from a version folder, stopped through its own handle on drop.
struct Running(Child);

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Maps the library at `path` as an image, as the loader does, and closes the file: like a loaded
/// library, the mapping refuses every writer and any deletion yet lets its folder move.
fn map_image(path: &Path) -> winsafe::guard::CloseHandleGuard<winsafe::HFILEMAP> {
    use winsafe::{HFILE, co};
    let (file, _) = HFILE::CreateFile(
        path.to_str().unwrap(),
        co::GENERIC::READ,
        Some(co::FILE_SHARE::READ | co::FILE_SHARE::DELETE),
        None,
        co::DISPOSITION::OPEN_EXISTING,
        co::FILE_ATTRIBUTE::NORMAL,
        None,
        None,
        None,
    )
    .unwrap();
    file.CreateFileMapping(None, co::PAGE::READONLY, Some(co::SEC::IMAGE), None, None)
        .unwrap()
}

fn hold(path: &Path, share: u32) -> File {
    fs::write(path, b"held").unwrap();
    OpenOptions::new()
        .read(true)
        .share_mode(share)
        .open(path)
        .unwrap()
}

/// The names in `folder`, sorted.
fn names(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(folder)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn activate(root: &Path, version: &str) {
    let completion = Completion {
        path: false,
        allow_downgrade: false,
    };
    let result = complete::activate(root, &entry_for(version, version.as_bytes()), completion);
    assert_eq!(result.err().map(Error::code), None);
}

#[test]
fn completion_removes_only_unused_versions_and_retries_what_stayed() {
    let fixture = Fixture::new();
    let root = fixture.0.join("install");
    let versions = root.join("versions");
    let mut store = ReleaseStore::acquire(&root).unwrap();
    for version in [
        "0.1.0", "0.2.0", "0.3.0", "0.4.0", "0.5.0", "0.6.0", "notes",
    ] {
        fs::create_dir(versions.join(version)).unwrap();
        fs::write(versions.join(version).join("orchbee.txt"), version).unwrap();
    }
    for leftover in [
        ".remove-1-0",
        ".remove-2-0",
        ".stage-1-0",
        ".stage-x",
        "desktop",
    ] {
        fs::create_dir_all(root.join(leftover).join("part")).unwrap();
        fs::write(root.join(leftover).join("part").join("file.txt"), b"x").unwrap();
    }
    let entry = |version: &str| entry_for(version, version.as_bytes());
    let before = Pointer::activate(
        Some(Pointer::activate(None, entry("0.4.0"))),
        entry("0.5.0"),
    );
    store.replace_pointer(&before.encode()).unwrap();
    drop(store);

    // 0.2.0 runs a program, which the process scan finds; 0.3.0 has a library loaded by a process
    // the scan cannot query, which only the write probe finds; 0.4.0, the previous version until
    // this completion, and the leftover `.remove-2-0` each hold a file something keeps open without
    // allowing deletion.
    let system = winsafe::GetSystemDirectory().unwrap();
    let program = versions.join("0.2.0").join("orchbee.exe");
    fs::copy(Path::new(&system).join("cmd.exe"), &program).unwrap();
    let running = Running(
        Command::new(&program)
            .args(["/d", "/q"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let library = versions.join("0.3.0").join("orchbee.dll");
    fs::copy(Path::new(&system).join("version.dll"), &library).unwrap();
    let library = map_image(&library);
    let undeletable = hold(&versions.join("0.4.0").join("open.txt"), SHARE_READ_WRITE);
    let aside = hold(&root.join(".remove-2-0").join("open.txt"), SHARE_READ_WRITE);
    let (found, _) = running_programs::processes_inside(&versions.join("0.2.0"), &[]);
    assert!(found.iter().any(|process| process.pid == running.0.id()));

    activate(&root, "0.6.0");
    let after = pointer(&root).unwrap();
    assert_eq!(
        (after.active, after.previous),
        (entry("0.6.0"), Some(entry("0.5.0")))
    );
    assert_eq!(
        names(&versions),
        ["0.2.0", "0.3.0", "0.4.0", "0.5.0", "0.6.0", "notes"]
    );
    assert!(names(&versions.join("0.4.0")).contains(&"orchbee.txt".to_owned()));
    // Only folders of the store's own grammar go; another installer's folder and a look-alike stay.
    let kept = names(&root);
    for name in [".remove-2-0", ".stage-x", "desktop", "versions"] {
        assert!(kept.contains(&name.to_owned()), "{kept:?}");
    }
    for name in [".remove-1-0", ".stage-1-0"] {
        assert!(!kept.contains(&name.to_owned()), "{kept:?}");
    }

    // Once nothing runs from them, the next completion of the same version removes what stayed.
    // Windows releases an exited program's file shortly after the process ends, so a completion
    // that still finds it in use keeps it once more.
    drop((running, library, undeletable, aside));
    let give_up = Instant::now() + Duration::from_secs(10);
    loop {
        activate(&root, "0.6.0");
        if names(&versions) == ["0.5.0", "0.6.0", "notes"] || Instant::now() > give_up {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert_eq!(names(&versions), ["0.5.0", "0.6.0", "notes"]);
    assert_eq!(
        names(&root),
        [
            ".stage-x",
            "active.json",
            "desktop",
            "install.lock",
            "versions"
        ]
    );
}
