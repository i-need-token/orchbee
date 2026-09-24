use super::*;

fn id(n: u8) -> JobId {
    JobId([n; 16])
}

fn shell(script: &str) -> CommandSpec {
    CommandSpec {
        git_context: None,
        program: std::env::var_os("SystemRoot")
            .map(std::path::PathBuf::from)
            .unwrap()
            .join("System32/WindowsPowerShell/v1.0/powershell.exe")
            .into_os_string(),
        args: [
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            script,
        ]
        .into_iter()
        .map(OsString::from)
        .collect(),
    }
}

fn commands(probe: &str, list: &str) -> JobCommands {
    JobCommands {
        source: None,
        probe: shell(probe),
        list: shell(list),
        test_hooks: None,
    }
}

fn start(slot: &WorktreeDiscoverySlot, job: u8, ttl: Duration, commands: JobCommands) {
    assert_eq!(
        slot.start_with(id(job), &std::env::temp_dir(), ttl, commands),
        StartOutcome::Accepted
    );
}

fn settle(slot: &WorktreeDiscoverySlot, job: u8) -> JobOutcome {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        match slot.poll(id(job)) {
            PollOutcome::Pending if Instant::now() < deadline => thread::sleep(POLL_SLICE),
            PollOutcome::Completed(outcome) => return outcome,
            other => panic!("job did not settle: {other:?}"),
        }
    }
}

#[test]
fn native_pipes_preserve_raw_bytes_and_nonzero_exit() {
    let slot = WorktreeDiscoverySlot::new();
    start(
        &slot,
        1,
        Duration::from_secs(10),
        commands(
            "[Console]::OpenStandardOutput().Write([byte[]](112,0,255),0,3)",
            "[Console]::OpenStandardOutput().Write([byte[]](108,0,254),0,3)",
        ),
    );
    assert_eq!(
        settle(&slot, 1),
        JobOutcome::Completed {
            common_dir: vec![112, 0, 255],
            listing: vec![108, 0, 254],
            stderr: vec![],
        }
    );
    start(
        &slot,
        2,
        Duration::from_secs(10),
        commands("[Console]::Error.Write('failure'); exit 7", "exit 0"),
    );
    assert_eq!(
        settle(&slot, 2),
        JobOutcome::GitFailed {
            stage: Stage::Probe,
            status: Some(7),
            stderr: b"failure".to_vec(),
        }
    );
}

#[test]
fn native_cancel_timeout_and_busy_release_the_exact_children() {
    for cancel in [false, true] {
        let slot = WorktreeDiscoverySlot::new();
        let witness = Arc::new(Mutex::new(JobWitness::default()));
        let mut cmds = commands("Start-Sleep -Seconds 30", "exit 0");
        cmds.test_hooks = Some(TestHooks {
            faults: Faults::default(),
            witness: Arc::clone(&witness),
        });
        start(&slot, 1, Duration::from_millis(1500), cmds);
        let until = Instant::now() + Duration::from_secs(2);
        while witness.lock().unwrap().spawned.is_empty() && Instant::now() < until {
            thread::sleep(POLL_SLICE);
        }
        assert_eq!(
            slot.start_with(
                id(2),
                &std::env::temp_dir(),
                JOB_TTL,
                commands("exit 0", "exit 0")
            ),
            StartOutcome::Busy
        );
        if cancel {
            assert_eq!(slot.cancel(id(1)), CancelOutcome::Registered);
        }
        assert_eq!(
            settle(&slot, 1),
            if cancel {
                JobOutcome::Cancelled
            } else {
                JobOutcome::Timeout
            }
        );
        let witness = witness.lock().unwrap();
        assert_eq!(witness.spawned.len(), 1);
        assert_eq!(witness.spawned, witness.reaped);
        assert!(matches!(
            slot.cancel(id(1)),
            CancelOutcome::AlreadyTerminal(_)
        ));
    }
}

#[test]
fn explicit_stop_cancels_and_settles_owned_child_and_refuses_unknown_cleanup() {
    let slot = WorktreeDiscoverySlot::new();
    slot.stop_before(Instant::now()).unwrap();
    let witness = Arc::new(Mutex::new(JobWitness::default()));
    let mut cmds = commands("Start-Sleep -Seconds 30", "exit 0");
    cmds.test_hooks = Some(TestHooks {
        faults: Faults::default(),
        witness: Arc::clone(&witness),
    });
    start(&slot, 1, Duration::from_secs(15), cmds);
    let until = Instant::now() + Duration::from_secs(3);
    while witness.lock().unwrap().spawned.is_empty() && Instant::now() < until {
        thread::sleep(POLL_SLICE);
    }
    assert_eq!(witness.lock().unwrap().spawned.len(), 1);
    slot.stop_before(Instant::now() + Duration::from_secs(5))
        .unwrap();
    assert_eq!(
        slot.poll(id(1)),
        PollOutcome::Completed(JobOutcome::Cancelled)
    );
    let witness = witness.lock().unwrap();
    assert_eq!(witness.spawned, witness.reaped);
    slot.inner.lock().unwrap().dead = Some(id(2));
    assert!(
        slot.stop_before(Instant::now() + Duration::from_secs(1))
            .is_err()
    );
}

#[test]
fn native_output_limits_and_failure_cleanup_are_bounded() {
    let slot = WorktreeDiscoverySlot::new();
    start(
        &slot,
        1,
        Duration::from_secs(10),
        commands(
            "[Console]::Write('probe')",
            "[Console]::OpenStandardOutput().Write((New-Object byte[] 200000),0,200000)",
        ),
    );
    assert_eq!(
        settle(&slot, 1),
        JobOutcome::TooLarge { stage: Stage::List }
    );
    start(
        &slot,
        2,
        Duration::from_secs(10),
        commands("[Console]::Error.Write(('e'*200000)); exit 9", "exit 0"),
    );
    assert_eq!(
        settle(&slot, 2),
        JobOutcome::GitFailed {
            stage: Stage::Probe,
            status: Some(9),
            stderr: vec![b'e'; STDERR_RETAIN]
        }
    );
    for faults in [
        Faults {
            nonblock: true,
            ..Faults::default()
        },
        Faults {
            drain: true,
            ..Faults::default()
        },
    ] {
        let witness = Arc::new(Mutex::new(JobWitness::default()));
        let mut cmds = commands(
            "[Console]::Write('bytes'); Start-Sleep -Seconds 30",
            "exit 0",
        );
        cmds.test_hooks = Some(TestHooks {
            faults,
            witness: Arc::clone(&witness),
        });
        start(&slot, 3, Duration::from_secs(10), cmds);
        assert_eq!(
            settle(&slot, 3),
            JobOutcome::OwnerIo {
                stage: Some(Stage::Probe)
            }
        );
        let witness = witness.lock().unwrap();
        assert_eq!(witness.spawned.len(), 1);
        assert_eq!(witness.spawned, witness.reaped);
    }
}

#[test]
fn native_missing_executable_invalid_directory_and_unknown_job_are_typed() {
    let slot = WorktreeDiscoverySlot::new();
    assert_eq!(slot.poll(id(1)), PollOutcome::Unknown);
    assert_eq!(slot.cancel(id(1)), CancelOutcome::Unknown);
    for directory in ["", "relative", "C:relative", "\\rooted"] {
        assert_eq!(
            slot.start_with(
                id(1),
                Path::new(directory),
                JOB_TTL,
                commands("exit 0", "exit 0")
            ),
            StartOutcome::InvalidDirectory
        );
    }
    let mut cmds = commands("exit 0", "exit 0");
    cmds.probe.program = std::env::temp_dir()
        .join(format!("orchbee-no-executable-{}.exe", std::process::id()))
        .into_os_string();
    start(&slot, 1, JOB_TTL, cmds);
    assert_eq!(
        settle(&slot, 1),
        JobOutcome::SpawnUnavailable {
            stage: Stage::Probe
        }
    );
}
