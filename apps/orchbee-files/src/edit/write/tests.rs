//! The commit boundary under a fault injected at each filesystem call, the precondition's rule,
//! the in-place rewrite, and a save that another writer cuts.

use super::*;
use crate::fs::tests::Scratch;

/// This machine's filesystem, failing the call numbered `fail_at` and recording every call; with
/// `cut`, another writer cuts that file after each piece the save writes.
#[derive(Clone)]
struct Faulty {
    fail_at: usize,
    calls: Arc<Mutex<Vec<&'static str>>>,
    cut: Option<PathBuf>,
}

impl Faulty {
    fn new(fail_at: usize) -> Self {
        Self {
            fail_at,
            calls: Arc::default(),
            cut: None,
        }
    }

    fn step(&self, name: &'static str) -> io::Result<()> {
        let mut calls = self.calls.lock().unwrap();
        calls.push(name);
        if calls.len() - 1 == self.fail_at {
            Err(io::Error::other("injected"))
        } else {
            Ok(())
        }
    }

    fn failed(&self) -> Option<&'static str> {
        self.calls.lock().unwrap().get(self.fail_at).copied()
    }
}

impl Disk for Faulty {
    type File = File;

    fn inspect(&self, path: &Path) -> io::Result<Option<Found>> {
        self.step("inspect")?;
        System.inspect(path)
    }

    fn open_read(&self, path: &Path) -> io::Result<File> {
        self.step("open_read")?;
        System.open_read(path)
    }

    fn read(&self, file: &mut File, buffer: &mut [u8]) -> io::Result<usize> {
        self.step("read")?;
        System.read(file, buffer)
    }

    fn set_permissions(&self, path: &Path, permissions: Permissions) -> io::Result<()> {
        self.step("set_permissions")?;
        System.set_permissions(path, permissions)
    }

    fn create_folders(&self, path: &Path) -> io::Result<()> {
        self.step("create_folders")?;
        System.create_folders(path)
    }

    fn open_write(&self, path: &Path, open: Open) -> io::Result<File> {
        self.step("open_write")?;
        System.open_write(path, open)
    }

    fn truncate(&self, file: &mut File) -> io::Result<()> {
        self.step("truncate")?;
        System.truncate(file)
    }

    fn write(&self, file: &mut File, bytes: &[u8]) -> io::Result<()> {
        self.step("write")?;
        System.write(file, bytes)?;
        match &self.cut {
            Some(path) => OpenOptions::new().write(true).open(path)?.set_len(0),
            None => Ok(()),
        }
    }

    fn close(&self, file: File) -> io::Result<()> {
        self.step("close")?;
        System.close(file)
    }

    fn written(&self, path: &Path) -> io::Result<(u64, Signed)> {
        self.step("written")?;
        System.written(path)
    }
}

fn digest(bytes: &[u8]) -> Sha256Digest {
    Sha256Digest::from_bytes(Sha256::digest(bytes).into())
}

/// The precondition of a file read with `bytes`; its time does not count.
fn version(bytes: &[u8]) -> WritePrecondition {
    WritePrecondition::Version {
        mtime_ms: Signed::default(),
        size: SafeInteger::new(bytes.len() as u64).unwrap(),
        sha256: digest(bytes),
    }
}

fn utf8(path: &Path, text: &str, precondition: WritePrecondition) -> Write {
    Write {
        path: path.to_path_buf(),
        text: text.to_owned(),
        target: Target::new("utf8", false),
        precondition,
        unlock: false,
    }
}

fn set_readonly(path: &Path, readonly: bool) {
    let mut permissions = fs::metadata(path).unwrap().permissions();
    if readonly {
        permissions.set_readonly(true);
    } else {
        make_writable(&mut permissions);
    }
    fs::set_permissions(path, permissions).unwrap();
}

fn is_readonly(path: &Path) -> bool {
    readonly(&fs::metadata(path).unwrap().permissions())
}

#[test]
fn a_failure_before_the_commit_point_leaves_the_file_as_it_was() {
    let scratch = Scratch::new("write-commit");
    let path = scratch.path().join("locked.txt");
    let original = b"original content\r\n";
    let text = "new content, longer than the original: \u{e9}\u{4e2d}\n";
    let mut expected = vec![0xFF, 0xFE];
    expected.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
    let helpers = Helpers::new();
    let (mut unchanged, mut incomplete) = (0, 0);
    for fail_at in 0.. {
        if path.exists() {
            set_readonly(&path, false);
        }
        fs::write(&path, original).unwrap();
        set_readonly(&path, true);
        let disk = Faulty::new(fail_at);
        let write = Write {
            target: Target::new("utf16le", false),
            unlock: true,
            ..utf8(&path, text, version(original))
        };
        let result = run(&helpers, &disk, &write, &|| false);
        let bytes = fs::read(&path).unwrap();
        let step = disk.failed();
        match result {
            // No fault, or one after the content was written, which reports it written.
            Ok(written) => {
                assert_eq!(bytes, expected, "{step:?}");
                assert_eq!(written.sha256, digest(&expected), "{step:?}");
                assert!(!is_readonly(&path), "a save leaves the file writable");
                if step.is_none() {
                    assert_eq!(written.size.get(), expected.len() as u64);
                    break;
                }
            }
            Err(error) => {
                let step = step.expect("only an injected fault fails the write");
                if error.code == ErrorCode::WriteIncomplete {
                    incomplete += 1;
                } else {
                    // Any other failure changed nothing, and locks an unlocked file again.
                    assert_eq!(bytes, original, "{step}");
                    assert!(is_readonly(&path), "{step} relocks");
                    unchanged += 1;
                }
            }
        }
    }
    set_readonly(&path, false);
    // The faults fell on both sides of the commit point.
    assert!(unchanged > 0 && incomplete > 0, "{unchanged} {incomplete}");
}

#[test]
fn the_digest_decides_whether_the_file_changed() {
    let scratch = Scratch::new("write-precondition");
    let helpers = Helpers::new();
    let path = scratch.path().join("file.txt");
    let write = |text: &str, precondition| {
        run(&helpers, &System, &utf8(&path, text, precondition), &|| {
            false
        })
    };
    let code = |result: Result<FsWriteResult, FilesError>| result.err().map(|error| error.code);

    fs::write(&path, "read").unwrap();
    assert_eq!(code(write("saved", version(b"read"))), None);
    assert_eq!(fs::read(&path).unwrap(), b"saved");

    // Changed on disk, to the same size and to another.
    fs::write(&path, "READ").unwrap();
    assert_eq!(
        code(write("saved", version(b"read"))),
        Some(ErrorCode::ContentNewer)
    );
    fs::write(&path, "changed").unwrap();
    assert_eq!(
        code(write("saved", version(b"read"))),
        Some(ErrorCode::ContentNewer)
    );
    assert_eq!(fs::read(&path).unwrap(), b"changed");

    // Changed on disk to the content being saved.
    fs::write(&path, "saved").unwrap();
    assert_eq!(code(write("saved", version(b"read"))), None);
    fs::write(&path, "same").unwrap();
    assert_eq!(code(write("same", version(b"else"))), None);

    // `none` replaces whatever is there; `create` needs nothing there.
    assert_eq!(code(write("anything", WritePrecondition::None)), None);
    assert_eq!(fs::read(&path).unwrap(), b"anything");
    assert_eq!(
        code(write("new", WritePrecondition::Create)),
        Some(ErrorCode::Exists)
    );

    // A missing file is created, with the folders above it, whatever the precondition.
    let nested = scratch.path().join("a").join("b").join("new.txt");
    let created = run(
        &helpers,
        &System,
        &utf8(&nested, "made", WritePrecondition::Create),
        &|| false,
    );
    assert_eq!(code(created), None);
    assert_eq!(fs::read(&nested).unwrap(), b"made");
    fs::remove_file(&path).unwrap();
    assert_eq!(code(write("again", version(b"anything"))), None);
    assert_eq!(fs::read(&path).unwrap(), b"again");

    fs::create_dir(scratch.path().join("folder")).unwrap();
    let folder = scratch.path().join("folder");
    let into_folder = run(
        &helpers,
        &System,
        &utf8(&folder, "x", WritePrecondition::None),
        &|| false,
    );
    assert_eq!(code(into_folder), Some(ErrorCode::IsDirectory));
}

#[test]
fn a_read_only_file_needs_unlock_and_a_cancelled_write_changes_nothing() {
    let scratch = Scratch::new("write-locked");
    let helpers = Helpers::new();
    let path = scratch.path().join("locked.txt");
    fs::write(&path, "locked").unwrap();
    set_readonly(&path, true);
    let locked = run(
        &helpers,
        &System,
        &utf8(&path, "new", version(b"locked")),
        &|| false,
    );
    assert_eq!(
        locked.err().map(|error| error.code),
        Some(ErrorCode::ReadOnly)
    );
    let unlocked = Write {
        unlock: true,
        ..utf8(&path, "new", version(b"locked"))
    };
    let cancelled = run(&helpers, &System, &unlocked, &|| true);
    assert_eq!(
        cancelled.err().map(|error| error.code),
        Some(ErrorCode::Cancelled)
    );
    assert_eq!(fs::read(&path).unwrap(), b"locked");
    assert!(is_readonly(&path));
    set_readonly(&path, false);
}

#[test]
fn a_save_rewrites_the_file_in_place() {
    let scratch = Scratch::new("write-in-place");
    let path = scratch.path().join("file.txt");
    let link = scratch.path().join("hard-link.txt");
    fs::write(&path, "a longer original text").unwrap();
    fs::hard_link(&path, &link).unwrap();
    let write = Write {
        target: Target::new("windows1252", false),
        ..utf8(&path, "caf\u{e9} \u{4e2d}", WritePrecondition::None)
    };
    let written = run(&Helpers::new(), &System, &write, &|| false).unwrap();
    assert_eq!(written.replaced.get(), 1);
    assert_eq!(fs::read(&link).unwrap(), b"caf\xE9 ?");
}

#[test]
fn a_save_never_fills_with_zeros_what_another_writer_cut() {
    let scratch = Scratch::new("write-cut");
    let path = scratch.path().join("file.txt");
    fs::write(&path, "the original").unwrap();
    let disk = Faulty {
        cut: Some(path.clone()),
        ..Faulty::new(usize::MAX)
    };
    let save = utf8(&path, "the save", version(b"the original"));
    run(&Helpers::new(), &disk, &save, &|| false).unwrap();
    // The other writer cut the file last, and the save leaves it as that writer did.
    assert_eq!(fs::read(&path).unwrap(), b"");
}
