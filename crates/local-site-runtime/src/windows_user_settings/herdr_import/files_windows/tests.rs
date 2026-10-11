use super::*;
use std::{
    fs,
    path::PathBuf,
    sync::{
        Barrier,
        atomic::{AtomicU64, Ordering},
    },
};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "orchbee-herdr-files-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // This unique fixture owns only ordinary leaves and no running processes.
        for entry in fs::read_dir(&self.0).unwrap() {
            fs::remove_file(entry.unwrap().path()).unwrap();
        }
        fs::remove_dir(&self.0).unwrap();
    }
}
#[test]
fn herdr_import_fault_cuts_keep_exact_empty_partial_or_complete_residue() {
    let fixture = Fixture::new();
    for cut in 0..3 {
        let path = fixture.0.join(format!("candidate-{cut}.toml"));
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .share_mode(0)
            .open(&path)
            .unwrap();
        let result = finish_new(
            &mut file,
            b"complete\n",
            |_| {
                if cut == 0 {
                    Err(io::ErrorKind::PermissionDenied.into())
                } else {
                    Ok(())
                }
            },
            |file, bytes| {
                if cut == 1 {
                    file.write_all(&bytes[..3])?;
                    Err(io::ErrorKind::WriteZero.into())
                } else {
                    file.write_all(bytes)
                }
            },
            |_| Err(io::ErrorKind::Other.into()),
        );
        assert!(result.unwrap_err().created());
        assert_eq!(
            result.unwrap_err().stage(),
            ["protect", "write", "synchronize"][cut]
        );
        drop(file);
        assert_eq!(
            fs::read(&path).unwrap(),
            [b"".as_slice(), b"com", b"complete\n"][cut]
        );
    }
}
#[test]
fn herdr_import_competing_creators_never_overwrite_the_winner_or_existing_bytes() {
    let fixture = Fixture::new();
    let path = fixture.0.join("candidate.toml");
    let plan = super::super::prepare_windows_herdr_import(
        "[session]\nresume_agents_on_restore=false\n",
        &fixture.0.join("source.toml"),
        &path,
        |_| None,
    )
    .unwrap();
    let barrier = Barrier::new(2);
    let results = std::thread::scope(|scope| {
        let a = scope.spawn(|| {
            barrier.wait();
            write_windows_herdr_import(&path, &plan)
        });
        let b = scope.spawn(|| {
            barrier.wait();
            write_windows_herdr_import(&path, &plan)
        });
        [a.join().unwrap(), b.join().unwrap()]
    });
    assert_eq!(
        results.iter().filter(|r| r.is_ok()).count(),
        1,
        "{results:?}"
    );
    assert_eq!(fs::read(&path).unwrap(), plan.candidate_toml().as_bytes());
    let original = fs::read(&path).unwrap();
    let rejected = write_windows_herdr_import(&path, &plan).unwrap_err();
    assert!(!rejected.created());
    assert_eq!(rejected.reason(), "already-exists");
    assert_eq!(fs::read(&path).unwrap(), original);
}
#[test]
fn herdr_import_source_is_bounded_regular_single_link_and_excludes_active_writers() {
    let fixture = Fixture::new();
    let path = fixture.0.join("source.toml");
    fs::write(&path, b"[ui]\nwindow_title='plain'\n").unwrap();
    assert!(read_windows_herdr_source(&path).unwrap().contains("plain"));
    let writer = OpenOptions::new()
        .write(true)
        .share_mode(3)
        .open(&path)
        .unwrap();
    assert_eq!(
        read_windows_herdr_source(&path).unwrap_err().reason,
        "unavailable"
    );
    drop(writer);
    let linked = fixture.0.join("linked.toml");
    fs::hard_link(&path, &linked).unwrap();
    assert_eq!(
        read_windows_herdr_source(&path).unwrap_err().reason,
        "not-file"
    );
    fs::remove_file(&linked).unwrap();
    fs::write(&path, [0xff]).unwrap();
    assert_eq!(
        read_windows_herdr_source(&path).unwrap_err().reason,
        "invalid-utf8"
    );
    fs::write(&path, vec![b' '; 64 * 1024 + 1]).unwrap();
    assert_eq!(
        read_windows_herdr_source(&path).unwrap_err().reason,
        "too-large"
    );
    assert!(read_windows_herdr_source(&fixture.0).is_err());
}
