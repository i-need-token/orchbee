//! Windows adapter witnesses. Every fixture owns its directory and every
//! provider tree is owned by the production Job runner under test.
use super::{exec, files};
use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

struct Fixture(PathBuf);

#[test]
fn capsule_copy_budget_includes_the_manifest_beside_the_maximum_payload() {
    let fixture = Fixture::new();
    let source = fixture.0.join("source");
    fs::create_dir(&source).unwrap();
    for index in 0..4 {
        fs::write(
            source.join(format!("payload-{index}")),
            vec![b'x'; 512 * 1024],
        )
        .unwrap();
    }
    fs::write(source.join("capsule.json"), vec![b' '; 1024]).unwrap();
    files::copy_tree(&source, &fixture.0.join("fits")).unwrap();
    fs::write(source.join("one-byte-too-many"), b"x").unwrap();
    assert!(files::copy_tree(&source, &fixture.0.join("over-budget")).is_err());
}
impl Fixture {
    fn new() -> Self {
        let mut nonce = [0; 16];
        getrandom::fill(&mut nonce).unwrap();
        let path = std::env::temp_dir().join(format!(
            "orchbee-integration-windows-{:032x}",
            u128::from_le_bytes(nonce)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn node(&self) -> PathBuf {
        exec::resolve_executable("node", std::env::var_os("PATH").as_deref())
            .expect("repository Node toolchain")
    }
    fn run(
        &self,
        script: &str,
        cancel: Option<&dyn crate::termination_signal::TerminationSource>,
    ) -> exec::CommandResult {
        exec::run_command(
            &self.node(),
            &["-e".into(), script.into()],
            &self.0,
            Instant::now() + exec::SLOT,
            cancel,
        )
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let root = fs::canonicalize(std::env::temp_dir()).unwrap();
        let target = fs::canonicalize(&self.0).unwrap();
        assert!(target.starts_with(root));
        assert!(
            self.0
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("orchbee-integration-windows-")
        );
        fs::remove_dir_all(target).unwrap();
    }
}

#[test]
fn publication_conflict_and_restore_preserve_foreign_bytes() {
    let fixture = Fixture::new();
    let directory = fixture.0.join("中文 & spaces");
    let stage = files::stage_directory(&directory, "entry").unwrap();
    let source = stage.join("payload");
    files::write_file_exclusive(&source, b"owned").unwrap();
    let target = directory.join("entry");
    fs::write(&target, b"foreign").unwrap();
    assert_eq!(
        files::publish_no_replace(&source, &target),
        files::FsStep::Conflict
    );
    assert_eq!(fs::read(&target).unwrap(), b"foreign");
    assert_eq!(fs::read(&source).unwrap(), b"owned");
    fs::remove_file(&target).unwrap();
    assert_eq!(
        files::publish_no_replace(&source, &target),
        files::FsStep::Done
    );
    let backup = files::move_aside(&target).unwrap().unwrap();
    fs::write(&target, b"arrival").unwrap();
    assert_eq!(
        files::restore_backup(&backup, &target),
        files::FsStep::Conflict
    );
    assert_eq!(fs::read(&backup).unwrap(), b"owned");
    assert_eq!(fs::read(&target).unwrap(), b"arrival");
    assert_eq!(files::remove_owned(&stage, true), files::FsStep::Done);
}

#[test]
fn ambiguous_and_device_paths_never_admit_effects() {
    for path in [
        r"C:\a\..\b",
        r"C:\a\.\b",
        r"C:\a\file:stream",
        r"C:\a\NUL.txt",
        r"C:\a\CON",
        r"C:\a\COM1",
        r"C:\a\x.",
        r"\\server\share\x",
        r"relative\x",
        r"\\.\pipe\x",
    ] {
        assert!(!files::path_admitted(std::path::Path::new(path)), "{path}");
    }
}

#[test]
fn junctions_cannot_redirect_copy_or_cleanup_and_pins_block_ancestor_moves() {
    use std::os::windows::process::CommandExt;
    let fixture = Fixture::new();
    let source = fixture.0.join("source");
    let link = fixture.0.join("junction");
    fs::create_dir(&source).unwrap();
    fs::write(source.join("keep"), b"preserve").unwrap();
    let powershell = PathBuf::from(winsafe::GetSystemDirectory().unwrap())
        .join("WindowsPowerShell/v1.0/powershell.exe");
    let output = std::process::Command::new(powershell)
        .args(["-NoProfile", "-NonInteractive", "-Command", "New-Item -ItemType Junction -Path $env:INTEGRATION_TEST_LINK -Target $env:INTEGRATION_TEST_SOURCE -ErrorAction Stop | Out-Null"])
        .env("INTEGRATION_TEST_LINK", &link).env("INTEGRATION_TEST_SOURCE", &source)
        .creation_flags(0x0800_0000).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(files::stage_directory(&link, "entry").is_err());
    assert!(files::read_bounded(&link.join("keep"), 1024).is_err());
    assert!(files::copy_tree(&link, &fixture.0.join("copy")).is_err());
    assert_eq!(files::remove_owned(&link, true), files::FsStep::Conflict);
    assert_eq!(fs::read(source.join("keep")).unwrap(), b"preserve");
    let pins = files::pin_tree(&source, false).unwrap();
    assert!(fs::rename(&source, fixture.0.join("moved")).is_err());
    drop(pins);
    fs::remove_dir(link).unwrap();
}

#[test]
fn job_runner_captures_output_and_enforces_limit() {
    let fixture = Fixture::new();
    let exec::CommandResult::Completed { stdout } =
        fixture.run("process.stdout.write('中文 output')", None)
    else {
        panic!("command failed");
    };
    assert_eq!(String::from_utf8(stdout).unwrap(), "中文 output");
    assert!(matches!(
        fixture.run("process.stdout.write('x'.repeat(300000))", None),
        exec::CommandResult::OutputTooLarge
    ));
    assert!(matches!(
        fixture.run("process.exitCode = 17", None),
        exec::CommandResult::Failed
    ));
}

struct CancelWhenChildReady {
    ready: PathBuf,
    child: std::cell::RefCell<Option<winsafe::guard::CloseHandleGuard<winsafe::HPROCESS>>>,
}
impl crate::termination_signal::TerminationSource for CancelWhenChildReady {
    fn requested(&self) -> bool {
        if self.child.borrow().is_some() {
            return true;
        }
        let pid = match fs::read_to_string(&self.ready) {
            Ok(pid) => pid.parse::<u32>().expect("atomically published child PID"),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return false,
            Err(error) => panic!("read child readiness: {error}"),
        };
        // Hold the exact process object before signalling cancellation, avoiding PID reuse.
        let handle = winsafe::HPROCESS::OpenProcess(winsafe::co::PROCESS::SYNCHRONIZE, false, pid)
            .expect("open live fixture descendant");
        self.child.replace(Some(handle));
        true
    }
}

#[test]
fn cancellation_and_timeout_kill_descendants() {
    let fixture = Fixture::new();
    let script = "const {spawn}=require('node:child_process'); const child=spawn(process.execPath,['-e',`const fs=require('node:fs');fs.writeFileSync('child-starting',String(process.pid));fs.renameSync('child-starting','child-ready');setTimeout(()=>fs.writeFileSync('escaped','bad'),1800);setInterval(()=>{},1000);`],{stdio:'ignore'});setInterval(()=>{},1000);";
    let cancel = CancelWhenChildReady {
        ready: fixture.0.join("child-ready"),
        child: std::cell::RefCell::new(None),
    };
    assert!(matches!(
        fixture.run(script, Some(&cancel)),
        exec::CommandResult::Cancelled
    ));
    assert!(
        fixture.0.join("child-ready").exists(),
        "descendant must have started"
    );
    let child = cancel.child.borrow();
    let handle = child
        .as_ref()
        .expect("descendant observed before cancellation");
    assert_eq!(
        handle
            .WaitForSingleObject(Some(1000))
            .expect("wait for descendant"),
        winsafe::co::WAIT::OBJECT_0,
        "exact descendant must exit after Job cancellation"
    );
    assert!(
        !fixture.0.join("escaped").exists(),
        "descendant survived cancellation"
    );
    let start = Instant::now();
    assert!(matches!(
        fixture.run("setInterval(()=>{},1000)", None),
        exec::CommandResult::TimedOut
    ));
    assert!(start.elapsed() < Duration::from_secs(5));
}

#[test]
fn normal_exit_also_terminates_retained_descendants() {
    let fixture = Fixture::new();
    let script = "const fs=require('node:fs');const child=require('node:child_process').spawn(process.execPath,['-e',`const fs=require('node:fs');fs.writeFileSync('ready','yes');setTimeout(()=>fs.writeFileSync('escaped','bad'),1200);setInterval(()=>{},1000);`],{stdio:'ignore'});child.unref();const timer=setInterval(()=>{if(fs.existsSync('ready'))clearInterval(timer)},10);";
    assert!(matches!(
        fixture.run(script, None),
        exec::CommandResult::Completed { .. }
    ));
    assert!(fixture.0.join("ready").exists());
    std::thread::sleep(Duration::from_millis(1500));
    assert!(!fixture.0.join("escaped").exists());
}

#[test]
fn batch_shim_preserves_arguments_without_shell_interpolation() {
    let fixture = Fixture::new();
    let script = fixture.0.join("echo.cjs");
    fs::write(
        &script,
        "process.stdout.write(JSON.stringify(process.argv.slice(2)))",
    )
    .unwrap();
    let shim = fixture.0.join("codex.cmd");
    fs::write(
        &shim,
        format!(
            "@echo off\r\n\"{}\" \"{}\" %*\r\n",
            fixture.node().display(),
            script.display()
        ),
    )
    .unwrap();
    let arguments: Vec<String> = [
        "plain",
        "中文 space",
        "x&echo injected",
        "%PATH%",
        "bang!",
        "",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    let result = exec::run_command(
        &shim,
        &arguments,
        &fixture.0,
        Instant::now() + exec::SLOT,
        None,
    );
    let exec::CommandResult::Completed { stdout } = result else {
        panic!("{result:?}");
    };
    assert_eq!(
        serde_json::from_slice::<Vec<String>>(&stdout).unwrap(),
        arguments
    );
}
