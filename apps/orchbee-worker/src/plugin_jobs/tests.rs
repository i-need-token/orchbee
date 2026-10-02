use super::*;
fn id(n: u32) -> OperationId {
    let mut b = [0; 16];
    b[..4].copy_from_slice(&n.to_be_bytes());
    OperationId::from_bytes(b).unwrap()
}
struct ConfiguredFixture {
    jobs: PluginJobs,
    directory: std::path::PathBuf,
    cleaned: bool,
}
impl ConfiguredFixture {
    fn new() -> Self {
        let root = std::env::temp_dir().canonicalize().unwrap();
        let directory = root.join(format!(
            "orchbee-configured-command-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&directory).unwrap();
        Self {
            jobs: PluginJobs::new(),
            directory,
            cleaned: false,
        }
    }
    fn cleanup(&mut self) -> io::Result<()> {
        if self.cleaned {
            return Ok(());
        }
        self.jobs
            .stop_before(Instant::now() + Duration::from_secs(6))?;
        match std::fs::remove_file(self.directory.join("receipt.txt")) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        std::fs::remove_dir(&self.directory)?;
        self.cleaned = true;
        Ok(())
    }
}
impl Drop for ConfiguredFixture {
    fn drop(&mut self) {
        if let Err(error) = self.cleanup() {
            eprintln!(
                "Configured fixture retained at {}: {error}",
                self.directory.display()
            );
        }
    }
}
fn wait_for_configured_receipt(directory: &std::path::Path) {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let incomplete = match std::fs::read_to_string(directory.join("receipt.txt")) {
            Ok(receipt) if receipt.trim() == "cmd_profile_0" => return,
            Ok(receipt) => format!("incomplete command receipt: {receipt:?}"),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                "command receipt absent".into()
            }
            Err(error) => panic!("command receipt read failed: {error}"),
        };
        assert!(Instant::now() < deadline, "{incomplete}");
        thread::sleep(Duration::from_millis(10));
    }
}
#[test]
fn configured_receipt_waits_for_content_after_file_creation() {
    let mut fixture = ConfiguredFixture::new();
    let receipt = fixture.directory.join("receipt.txt");
    std::fs::write(&receipt, b"").unwrap();
    thread::scope(|scope| {
        scope.spawn(|| {
            thread::sleep(Duration::from_millis(100));
            std::fs::write(&receipt, b"cmd_profile_0\r\n").unwrap();
        });
        wait_for_configured_receipt(&fixture.directory);
    });
    fixture.cleanup().unwrap();
}
#[test]
fn configured_fixture_cleanup_failure_preserves_the_original_panic() {
    use std::os::windows::fs::OpenOptionsExt;
    let fixture = ConfiguredFixture::new();
    let directory = fixture.directory.clone();
    // FILE_FLAG_BACKUP_SEMANTICS opens this exact owned directory. Denying
    // FILE_SHARE_DELETE makes cleanup failure deterministic without a child race.
    let consumer = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(3)
        .custom_flags(0x0200_0000)
        .open(&directory)
        .unwrap();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _fixture = fixture;
        panic!("configured fixture original failure");
    }));
    drop(consumer);
    std::fs::remove_dir(&directory).unwrap();
    let panic = outcome.expect_err("the original panic must remain observable");
    assert_eq!(
        panic.downcast_ref::<&str>(),
        Some(&"configured fixture original failure")
    );
}
#[test]
fn configured_shell_is_retained_independently_and_shutdown_joins_its_process() {
    let mut owned = ConfiguredFixture::new();
    let command = orchbee_capability_node_protocol::ConfiguredCommandIntent {
        command_id: "cmd_profile_0".into(), directory: owned.directory.to_str().unwrap().into(),
        command: "echo %ORCHBEE_COMMAND_ID%>receipt.txt & ping -n 20 127.0.0.1 >nul & echo finished>>receipt.txt".into(),
        environment: vec![("ORCHBEE_COMMAND_ID".into(), "cmd_profile_0".into())],
    };
    assert_eq!(
        owned
            .jobs
            .execute(id(800), PluginJobAction::StartConfigured(command.clone())),
        Outcome::ConfiguredStarted {
            job_id: id(800),
            existing: false
        }
    );
    assert_eq!(
        owned
            .jobs
            .execute(id(800), PluginJobAction::StartConfigured(command.clone())),
        Outcome::ConfiguredStarted {
            job_id: id(800),
            existing: true
        }
    );
    let mut other = command.clone();
    other.command = "exit 0".into();
    assert_eq!(
        owned
            .jobs
            .execute(id(800), PluginJobAction::StartConfigured(other)),
        Outcome::Refused(Refusal::Conflict)
    );
    assert_eq!(
        owned.jobs.execute(
            id(800),
            PluginJobAction::Start(intent(&["cmd.exe", "/c", "exit 0"]))
        ),
        Outcome::Refused(Refusal::Conflict)
    );
    wait_for_configured_receipt(&owned.directory);
    assert!(
        matches!(owned.jobs.execute(id(801), PluginJobAction::List {plugin_id:None,before:None,limit:4}), Outcome::Logs {logs,..} if logs.is_empty())
    );
    owned
        .jobs
        .stop_before(Instant::now() + Duration::from_secs(6))
        .unwrap();
    let before = std::fs::read(owned.directory.join("receipt.txt")).unwrap();
    thread::sleep(Duration::from_millis(100));
    assert_eq!(
        std::fs::read(owned.directory.join("receipt.txt")).unwrap(),
        before
    );
    assert!(!String::from_utf8_lossy(&before).contains("finished"));
    let mut bad = command;
    bad.directory = owned.directory.join("missing").to_str().unwrap().into();
    let jobs = PluginJobs::new();
    assert_eq!(
        jobs.execute(id(802), PluginJobAction::StartConfigured(bad.clone())),
        Outcome::Refused(Refusal::Unavailable)
    );
    assert_eq!(
        jobs.execute(id(802), PluginJobAction::StartConfigured(bad)),
        Outcome::Refused(Refusal::Unavailable)
    );
    owned.cleanup().unwrap();
}
fn intent(args: &[&str]) -> PluginCommandIntent {
    PluginCommandIntent {
        plugin_id: "test".into(),
        command_id: "action".into(),
        directory: std::env::current_dir().unwrap().to_str().unwrap().into(),
        argv: args.iter().map(|s| s.to_string()).collect(),
        environment: vec![],
    }
}
fn complete(jobs: &PluginJobs, id: OperationId) -> PluginCommandLog {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let Outcome::Logs { mut logs, .. } = jobs.execute(id, PluginJobAction::Get { job_id: id })
        else {
            panic!("query failed")
        };
        let log = logs.remove(0);
        if log.finished_ms.is_some() {
            log.validate().unwrap();
            return log;
        }
        assert!(Instant::now() < deadline, "command did not finish");
        thread::sleep(Duration::from_millis(10));
    }
}
#[test]
fn plugin_executes_and_retains_exact_receipts_without_replay() {
    let jobs = PluginJobs::new();
    let intent = intent(&[
        "cmd.exe",
        "/d",
        "/c",
        "echo hello & echo error 1>&2 & exit /b 7",
    ]);
    assert!(matches!(
        jobs.execute(id(1), PluginJobAction::Start(intent.clone())),
        Outcome::Started {
            existing: false,
            ..
        }
    ));
    let log = complete(&jobs, id(1));
    assert_eq!(log.exit_code, Some(7));
    assert_eq!(log.failure, Some(Failure::ExitFailure));
    assert!(String::from_utf8_lossy(&log.stdout).contains("hello"));
    assert!(String::from_utf8_lossy(&log.stderr).contains("error"));
    assert_eq!(
        jobs.execute(id(1), PluginJobAction::Start(intent.clone())),
        Outcome::Started {
            log: Box::new(log),
            existing: true
        }
    );
    let mut different = intent;
    different.argv[3] = "echo other".into();
    assert_eq!(
        jobs.execute(id(1), PluginJobAction::Start(different)),
        Outcome::Refused(Refusal::Conflict)
    );
    assert!(matches!(
        jobs.execute(
            id(2),
            PluginJobAction::Start(self::intent(&[
                "orchbee-plugin-missing-executable-test.exe"
            ]))
        ),
        Outcome::Started { .. }
    ));
    assert_eq!(
        complete(&jobs, id(2)).failure,
        Some(Failure::SpawnUnavailable)
    );
    jobs.stop_before(Instant::now() + Duration::from_secs(6))
        .unwrap();
}
#[test]
fn plugin_large_output_is_drained_and_explicitly_truncated() {
    let jobs = PluginJobs::new();
    let command = intent(&[
        "powershell.exe",
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        "[Console]::Out.Write(('x'*100000)); [Console]::Error.Write(('y'*100000)); exit 0",
    ]);
    assert!(matches!(
        jobs.execute(id(3), PluginJobAction::Start(command)),
        Outcome::Started { .. }
    ));
    let log = complete(&jobs, id(3));
    assert_eq!(log.failure, None);
    assert_eq!(log.exit_code, Some(0));
    assert_eq!(log.stdout, vec![b'x'; 64 * 1024]);
    assert_eq!(log.stderr, vec![b'y'; 64 * 1024]);
    assert!(log.stdout_truncated && log.stderr_truncated);
}
fn waiting(_: PluginCommandIntent, log: PluginCommandLog, cancel: &AtomicBool) -> PluginCommandLog {
    while !cancel.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_millis(1));
    }
    finish_failure(log, Failure::Cancelled)
}
#[test]
fn plugin_capacity_fencing_cancellation_and_shutdown_join_owners() {
    let jobs = PluginJobs::with_execute(waiting);
    let intent = intent(&["unused.exe"]);
    let request = orchbee_capability_node_protocol::PluginJobRequest {
        operation_id: id(1),
        worker_id: orchbee_capability_node_protocol::WorkerId::from_bytes([3; 16]).unwrap(),
        worker_generation: orchbee_capability_node_protocol::WorkerGeneration::new(
            std::num::NonZeroU64::MIN,
        ),
        action: PluginJobAction::Start(intent.clone()),
    };
    let refused = crate::plugin_job_operation::execute(
        request,
        orchbee_local_site_protocol::WorkerId::from_bytes([4; 16]),
        std::num::NonZeroU64::MIN,
        &jobs,
        None,
    );
    assert_eq!(refused.outcome, Outcome::Refused(Refusal::RejectedFencing));
    assert!(jobs.inner.lock().unwrap().records.is_empty());
    for n in 1..=32 {
        assert!(matches!(
            jobs.execute(id(n), PluginJobAction::Start(intent.clone())),
            Outcome::Started { .. }
        ));
    }
    assert_eq!(
        jobs.execute(id(33), PluginJobAction::Start(intent.clone())),
        Outcome::Refused(Refusal::Busy)
    );
    assert_eq!(
        jobs.execute(id(34), PluginJobAction::Cancel { job_id: id(1) }),
        Outcome::Cancellation {
            already_complete: false
        }
    );
    assert_eq!(complete(&jobs, id(1)).failure, Some(Failure::Cancelled));
    jobs.stop_before(Instant::now() + Duration::from_secs(6))
        .unwrap();
    assert!(
        jobs.inner
            .lock()
            .unwrap()
            .records
            .iter()
            .all(|r| matches!(r.state, State::Complete))
    );
    assert_eq!(
        jobs.execute(id(35), PluginJobAction::Start(intent)),
        Outcome::Refused(Refusal::Unavailable)
    );
}
fn immediate(
    _: PluginCommandIntent,
    mut log: PluginCommandLog,
    _: &AtomicBool,
) -> PluginCommandLog {
    log.finished_ms = Some(now_ms().max(log.started_ms));
    log.exit_code = Some(0);
    log
}
#[test]
fn plugin_log_ring_pages_without_duplicate_or_reordered_entries() {
    let jobs = PluginJobs::with_execute(immediate);
    for n in 1..=205 {
        assert!(matches!(
            jobs.execute(id(n), PluginJobAction::Start(intent(&["unused.exe"]))),
            Outcome::Started { .. }
        ));
        complete(&jobs, id(n));
    }
    let mut before = None;
    let mut seq = Vec::new();
    loop {
        let Outcome::Logs { logs, next_before } = jobs.execute(
            id(400),
            PluginJobAction::List {
                plugin_id: Some("test".into()),
                before,
                limit: 4,
            },
        ) else {
            panic!()
        };
        seq.extend(logs.iter().map(|l| l.sequence));
        before = next_before;
        if before.is_none() {
            break;
        }
    }
    assert_eq!(seq, (6..=205).rev().collect::<Vec<_>>());
}
