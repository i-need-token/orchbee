//! The window step's arrangement on a scratch store (AU § Moving a flat layout, § Window cleanup): a
//! running flat Electron window's entry never moves, so its placement waits and the flat files stay
//! intact until that window quits; once it quits, the active version's launcher takes the entry and
//! the flat files go, except a flat bundled release directory a Session still runs from, which goes
//! once that Session ends. A running launcher moves aside as `.remove-flat-*.exe` so the new
//! launcher takes its path at once. Unused window versions, crashed runs' leftovers and an earlier
//! Setup's `desktop.previous` go meanwhile.
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

/// The test store's active window version: this build's own version, so its Client is not older
/// than that version and arranges the store.
const ACTIVE: &str = env!("CARGO_PKG_VERSION");

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
    let version = desktop.join(format!(r"versions\{ACTIVE}"));
    write(
        &version.join(format!(r"resources\cohort\{stem}-desktop-launcher.exe")),
        launcher,
    );
    write(&version.join(&entry_name), b"window");
    write(&desktop.join(r"versions\0.0.1\old.txt"), b"old");
    write(&desktop.join(r".stage-1-0\part.txt"), b"x");
    write(&desktop.join(".new-1-0"), b"x");
    write(
        &root.join(r"desktop.previous\resources\app.asar"),
        b"previous",
    );
    let mut store = ReleaseStore::acquire(&desktop).unwrap();
    store
        .replace_pointer(&Pointer::activate(None, entry_for(ACTIVE, b"list")).encode())
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
    assert!(desktop.join(r"resources\app.asar").is_file());
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
    assert_eq!(names(&desktop.join("versions")), [ACTIVE]);
    assert!(!root.join("desktop.previous").exists());

    drop(window);
    let window_quitted = sorted(&[
        entry_name.as_str(),
        "active.json",
        "install.lock",
        "resources",
        "versions",
    ]);
    tend_until(&root, || names(&desktop) == window_quitted);
    assert_eq!(fs::read(&entry).unwrap(), launcher);
    assert_eq!(names(&desktop), window_quitted);
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

/// A flat Electron window's entry runs from `desktop\` with `resources\app.asar` beside it, so it
/// never moves aside: the launcher waits for that window to quit, and its files stay until then.
#[test]
fn a_running_flat_electron_entry_is_never_moved_aside() {
    let fixture = Fixture::new();
    let root = fixture.0.join("install");
    let desktop = root.join(FOLDER);
    drop(ReleaseStore::acquire(&root).unwrap());
    let stem = environment_stem();
    let entry_name = format!("{stem}-desktop.exe");
    let entry = desktop.join(&entry_name);
    write(
        &desktop.join(format!(
            r"versions\{ACTIVE}\resources\cohort\{stem}-desktop-launcher.exe"
        )),
        b"the window launcher",
    );
    let mut store = ReleaseStore::acquire(&desktop).unwrap();
    store
        .replace_pointer(&Pointer::activate(None, entry_for(ACTIVE, b"list")).encode())
        .unwrap();
    drop(store);
    write(&desktop.join(r"resources\app.asar"), b"page");
    let window = start(&entry);
    let flat_window = fs::read(&entry).unwrap();

    layout::tend(&root);
    assert_eq!(fs::read(&entry).unwrap(), flat_window);
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
    assert_eq!(names(&desktop.join("resources")), ["app.asar"]);
    drop(window);
}

/// A window launcher, which has no `resources\app.asar` beside it, moves aside while it runs so the
/// new launcher takes its path at once; the moved-aside copy goes once its process has exited.
#[test]
fn a_running_launcher_moves_aside_so_the_new_launcher_takes_its_path() {
    let fixture = Fixture::new();
    let root = fixture.0.join("install");
    let desktop = root.join(FOLDER);
    drop(ReleaseStore::acquire(&root).unwrap());
    let stem = environment_stem();
    let entry_name = format!("{stem}-desktop.exe");
    let entry = desktop.join(&entry_name);
    let launcher = b"the new launcher".as_slice();
    write(
        &desktop.join(format!(
            r"versions\{ACTIVE}\resources\cohort\{stem}-desktop-launcher.exe"
        )),
        launcher,
    );
    let mut store = ReleaseStore::acquire(&desktop).unwrap();
    store
        .replace_pointer(&Pointer::activate(None, entry_for(ACTIVE, b"list")).encode())
        .unwrap();
    drop(store);
    let running = start(&entry);
    let old_launcher = fs::read(&entry).unwrap();

    layout::tend(&root);
    assert_eq!(fs::read(&entry).unwrap(), launcher);
    let aside = names(&desktop)
        .into_iter()
        .find(|name| name.starts_with(".remove-flat-") && name.ends_with(".exe"))
        .unwrap();
    assert_eq!(fs::read(desktop.join(&aside)).unwrap(), old_launcher);
    drop(running);
    let settled = sorted(&[
        entry_name.as_str(),
        "active.json",
        "install.lock",
        "versions",
    ]);
    tend_until(&root, || names(&desktop) == settled);
    assert_eq!(names(&desktop), settled);
}

/// A Client older than the store's active window version only retains versions: a flat layout, a
/// stale copy and the root's `desktop.previous` stay, the entry keeps its launcher, and the unused
/// version goes.
#[test]
fn an_older_client_retains_versions_and_arranges_nothing() {
    let fixture = Fixture::new();
    let root = fixture.0.join("install");
    let desktop = root.join(FOLDER);
    drop(ReleaseStore::acquire(&root).unwrap());
    let stem = environment_stem();
    let entry_name = format!("{stem}-desktop.exe");
    let entry = desktop.join(&entry_name);
    let newer = "99.0.0";
    let older_launcher = b"the older launcher".as_slice();
    write(&entry, older_launcher);
    write(
        &desktop.join(format!(
            r"versions\{newer}\resources\cohort\{stem}-desktop-launcher.exe"
        )),
        b"the newer launcher",
    );
    write(&desktop.join(r"versions\0.0.1\old.txt"), b"old");
    write(&desktop.join(".new-1-0"), b"x");
    write(&desktop.join("ffmpeg.dll"), b"flat");
    write(
        &root.join(r"desktop.previous\resources\app.asar"),
        b"previous",
    );
    let mut store = ReleaseStore::acquire(&desktop).unwrap();
    store
        .replace_pointer(&Pointer::activate(None, entry_for(newer, b"list")).encode())
        .unwrap();
    drop(store);

    layout::tend(&root);
    assert_eq!(fs::read(&entry).unwrap(), older_launcher);
    assert!(desktop.join(".new-1-0").is_file());
    assert!(desktop.join("ffmpeg.dll").is_file());
    assert!(root.join("desktop.previous").is_dir());
    assert_eq!(names(&desktop.join("versions")), [newer]);
}
