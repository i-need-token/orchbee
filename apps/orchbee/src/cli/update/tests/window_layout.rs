//! The window step's arrangement on a scratch store (AU § Moving a flat layout, § Window cleanup): a
//! flat window that runs keeps its entry and every flat file; once it quits, the entry becomes the
//! active version's launcher and the flat files go, except a flat bundled release directory a
//! Session still runs from, which goes once that Session ends. Unused window versions, crashed runs'
//! leftovers and an earlier Setup's `desktop.previous` go meanwhile.
use super::super::{
    running_programs,
    window::{FOLDER, layout},
};
use super::install_fixture::{Fixture, entry_for};
use crate::release_store::ReleaseStore;
use orchbee_local_runtime::environment_stem;
use orchbee_release_records::Pointer;
use std::{
    fs,
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

/// A stand-in process started from a store file, stopped through its own handle on drop.
struct Running(Child);

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Starts a copy of `cmd.exe` at `program`, which then holds its own file.
fn start(program: &Path) -> Running {
    let system = winsafe::GetSystemDirectory().unwrap();
    fs::create_dir_all(program.parent().unwrap()).unwrap();
    fs::copy(Path::new(&system).join("cmd.exe"), program).unwrap();
    Running(
        Command::new(program)
            .args(["/d", "/q"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    )
}

fn write(path: &Path, bytes: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
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

fn sorted(names: &[&str]) -> Vec<String> {
    let mut names: Vec<String> = names.iter().map(|name| (*name).to_owned()).collect();
    names.sort();
    names
}

/// Runs the step until `done` holds, for at most ten seconds: Windows releases an exited program's
/// file shortly after the process ends, so a run that still finds it in use keeps it once more.
fn tend_until(root: &Path, done: impl Fn() -> bool) {
    let give_up = Instant::now() + Duration::from_secs(10);
    loop {
        layout::tend(root);
        if done() || Instant::now() > give_up {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn a_flat_layout_moves_on_once_its_window_quits_and_its_session_ends() {
    let fixture = Fixture::new();
    let root = fixture.0.join("install");
    let desktop = root.join(FOLDER);
    drop(ReleaseStore::acquire(&root).unwrap());
    let stem = environment_stem();
    let entry_name = format!("{stem}-desktop.exe");
    let entry = desktop.join(&entry_name);
    // The first window version beside a flat layout, its pointer moved; an unused older version;
    // what crashed runs left; and the folder an earlier Setup moved its previous window into.
    let launcher = b"the window launcher".as_slice();
    let version = desktop.join(r"versions\0.2.0");
    write(
        &version.join(format!(r"resources\cohort\{stem}-desktop-launcher.exe")),
        launcher,
    );
    write(&version.join(&entry_name), b"window");
    write(&desktop.join(r"versions\0.1.0\old.txt"), b"old");
    write(&desktop.join(r".stage-1-0\part.txt"), b"x");
    write(&desktop.join(".new-1-0"), b"x");
    write(
        &root.join(r"desktop.previous\resources\app.asar"),
        b"previous",
    );
    let mut store = ReleaseStore::acquire(&desktop).unwrap();
    store
        .replace_pointer(&Pointer::activate(None, entry_for("0.2.0", b"list")).encode())
        .unwrap();
    drop(store);
    // The flat window runs from the entry, and a Session its bundled Client started still runs.
    write(&desktop.join("ffmpeg.dll"), b"flat");
    write(&desktop.join(r"resources\app.asar"), b"page");
    let window = start(&entry);
    let session = start(&desktop.join(format!(r"resources\cohort\{stem}.exe")));
    let flat_window = fs::read(&entry).unwrap();
    let (found, _) = running_programs::processes_inside(&desktop, &[]);
    assert!(found.iter().any(|process| process.pid == window.0.id()));

    layout::tend(&root);
    assert_eq!(fs::read(&entry).unwrap(), flat_window);
    assert_eq!(
        names(&desktop),
        sorted(&[
            entry_name.as_str(),
            "active.json",
            "ffmpeg.dll",
            "install.lock",
            "resources",
            "versions"
        ])
    );
    assert_eq!(names(&desktop.join("versions")), ["0.2.0"]);
    assert!(!root.join("desktop.previous").exists());

    drop(window);
    tend_until(&root, || {
        fs::read(&entry).is_ok_and(|bytes| bytes == launcher)
    });
    assert_eq!(fs::read(&entry).unwrap(), launcher);
    assert_eq!(
        names(&desktop),
        sorted(&[
            entry_name.as_str(),
            "active.json",
            "install.lock",
            "resources",
            "versions"
        ])
    );
    assert_eq!(names(&desktop.join("resources")), ["cohort"]);

    drop(session);
    let arranged = sorted(&[
        entry_name.as_str(),
        "active.json",
        "install.lock",
        "versions",
    ]);
    tend_until(&root, || names(&desktop) == arranged);
    assert_eq!(names(&desktop), arranged);
    assert_eq!(fs::read(version.join(&entry_name)).unwrap(), b"window");
}
