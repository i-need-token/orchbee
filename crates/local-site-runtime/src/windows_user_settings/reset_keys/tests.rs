use super::*;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "orchbee-reset-keys-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn path(&self) -> PathBuf {
        self.0.join("config.toml")
    }
    fn reset(&self) -> WindowsKeysResetReport {
        reset_windows_keybindings(|key| {
            (key == "ORCHBEE_CONFIG_FILE").then(|| self.path().into_os_string())
        })
    }
    fn leaves(&self) -> usize {
        fs::read_dir(&self.0).unwrap().count()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // This fixture creates only files in its unique owned root, never subdirectories.
        for entry in fs::read_dir(&self.0).unwrap() {
            fs::remove_file(entry.unwrap().path()).unwrap();
        }
        fs::remove_dir(&self.0).unwrap();
    }
}

#[test]
fn reset_keys_preserves_other_settings_comments_and_line_endings() {
    for keys in [
        "[keys]\r\nprefix='ctrl+a'\r\n[keys.indexed]\r\nswitch_tab='alt+1..9'\r\n[[keys.command]]\r\ncommand='example'\r\n",
        "[\"keys\"] # trailing comment\r\nprefix='f12'\r\n",
        "keys = {prefix = 'ctrl+a'}\r\n",
        "keys.prefix = 'ctrl+a'\r\n",
    ] {
        let other = "[terminal]\r\nnew_cwd = 'follow' # unchanged\r\n";
        assert_eq!(
            without_keys(&format!("# retained\r\n{keys}{other}")).unwrap(),
            Some(format!("# retained\r\n{other}"))
        );
    }
    assert_eq!(
        without_keys("# no keys\n[terminal]\nnew_cwd='home'").unwrap(),
        None
    );
}

#[test]
fn reset_keys_rejects_invalid_or_unsafe_edits_without_silently_changing_other_data() {
    assert_eq!(
        without_keys("[keys\nDO_NOT_ECHO").unwrap_err().reason,
        "invalid-toml"
    );
    // A line resembling a table inside a retained multiline string is not a keybinding table.
    for source in [
        "description = '''\n[keys]\nliteral\n'''\n[other]\nvalue=1\n[keys]\nprefix='f12'\n",
        "keys = [\n'ctrl+a',\n]\n[terminal]\nnew_cwd='home'\n",
    ] {
        assert_eq!(without_keys(source).unwrap_err().reason, "unsafe-edit");
    }
}

#[test]
fn reset_keys_missing_noop_and_refusals_create_nothing_and_never_echo_configuration() {
    let fixture = Fixture::new();
    assert!(matches!(
        fixture.reset().outcome,
        WindowsKeysResetOutcome::Missing
    ));
    assert_eq!(fixture.leaves(), 0);
    for bytes in [
        b"# only comments".to_vec(),
        vec![0xff],
        vec![b'a'; 65537],
        b"secret='DO_NOT_ECHO'\n[broken".to_vec(),
    ] {
        fs::write(fixture.path(), &bytes).unwrap();
        let outcome = fixture.reset().outcome;
        assert!(!format!("{outcome:?}").contains("DO_NOT_ECHO"));
        assert!(!matches!(outcome, WindowsKeysResetOutcome::Reset { .. }));
        assert_eq!(fs::read(fixture.path()).unwrap(), bytes);
        assert_eq!(fixture.leaves(), 1);
    }
}

#[test]
fn reset_keys_backs_up_exact_bytes_preserves_file_identity_and_never_repeats_a_write() {
    let fixture = Fixture::new();
    let source = b"# hello\r\n[terminal]\r\nnew_cwd='home'\r\n[keys]\r\nprefix='f12'\r\n";
    fs::write(fixture.path(), source).unwrap();
    let identity = winapi_util::file::information(File::open(fixture.path()).unwrap())
        .unwrap()
        .file_index();
    let WindowsKeysResetOutcome::Reset { backup_path } = fixture.reset().outcome else {
        panic!("reset")
    };
    assert_eq!(fs::read(&backup_path).unwrap(), source);
    permissions::require_private(&File::open(&backup_path).unwrap()).unwrap();
    assert_eq!(
        fs::read(fixture.path()).unwrap(),
        b"# hello\r\n[terminal]\r\nnew_cwd='home'\r\n"
    );
    assert_eq!(
        winapi_util::file::information(File::open(fixture.path()).unwrap())
            .unwrap()
            .file_index(),
        identity
    );
    assert!(matches!(
        fixture.reset().outcome,
        WindowsKeysResetOutcome::NoKeys
    ));
    assert_eq!(fixture.leaves(), 2);
    // A later reset produces another backup and cannot overwrite the earlier recovery artifact.
    fs::write(fixture.path(), "[keys]\nprefix='ctrl+b'\n").unwrap();
    let WindowsKeysResetOutcome::Reset {
        backup_path: second,
    } = fixture.reset().outcome
    else {
        panic!("second reset")
    };
    assert_ne!(backup_path, second);
    assert_eq!(fs::read(backup_path).unwrap(), source);
}

#[test]
fn reset_keys_refuses_an_open_file_or_a_hard_link_before_creating_backup() {
    let fixture = Fixture::new();
    let source = b"[keys]\nprefix='f12'\n";
    fs::write(fixture.path(), source).unwrap();
    let held = File::open(fixture.path()).unwrap();
    assert!(matches!(
        fixture.reset().outcome,
        WindowsKeysResetOutcome::Failed {
            recovery_required: false,
            ..
        }
    ));
    assert_eq!(fixture.leaves(), 1);
    drop(held);
    fs::hard_link(fixture.path(), fixture.0.join("alias")).unwrap();
    assert!(matches!(
        fixture.reset().outcome,
        WindowsKeysResetOutcome::Failed {
            recovery_required: false,
            ..
        }
    ));
    assert_eq!(fixture.leaves(), 2);
    assert_eq!(fs::read(fixture.path()).unwrap(), source);
}

#[test]
fn reset_keys_write_failure_restores_or_retains_a_complete_recovery_backup() {
    for recovery_fails in [false, true] {
        let fixture = Fixture::new();
        let before = b"[keys]\nprefix='f12'\n";
        fs::write(fixture.path(), before).unwrap();
        let mut file = open(&fixture.path(), true).unwrap();
        let mut writes = 0;
        let result = commit(&fixture.path(), &mut file, before, b"", |file, bytes| {
            writes += 1;
            if writes == 1 {
                replace(file, b"partial")?;
                Err(io::ErrorKind::Other.into())
            } else if recovery_fails {
                Err(io::ErrorKind::Other.into())
            } else {
                replace(file, bytes)
            }
        });
        drop(file);
        let WindowsKeysResetOutcome::Failed {
            backup_path,
            recovery_required,
            ..
        } = result
        else {
            panic!("failure")
        };
        assert_eq!(writes, 2);
        assert_eq!(recovery_required, recovery_fails);
        if recovery_fails {
            assert_eq!(fs::read(backup_path.unwrap()).unwrap(), before);
            assert_eq!(fs::read(fixture.path()).unwrap(), b"partial");
        } else {
            assert!(backup_path.is_none());
            assert_eq!(fixture.leaves(), 1);
            assert_eq!(fs::read(fixture.path()).unwrap(), before);
        }
    }
}

#[test]
fn reset_keys_changed_input_refuses_before_creating_any_backup() {
    let fixture = Fixture::new();
    let current = "[keys]\nprefix='ctrl+a'\n[terminal]\nnew_cwd='home'\n";
    fs::write(fixture.path(), current).unwrap();
    let read = open(&fixture.path(), false).unwrap();
    let result = apply_prepared(&fixture.path(), read, "[keys]\nprefix='f12'\n", "");
    let WindowsKeysResetOutcome::Failed {
        error,
        backup_path,
        recovery_required,
    } = result
    else {
        panic!("changed")
    };
    assert_eq!(error.reason, "changed");
    assert!(backup_path.is_none());
    assert!(!recovery_required);
    assert_eq!(fixture.leaves(), 1);
    assert_eq!(fs::read_to_string(fixture.path()).unwrap(), current);
}
