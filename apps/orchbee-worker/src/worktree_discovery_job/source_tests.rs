use super::*;
use crate::{git_repository_windows::canonical_path, windows_command_child::WindowsCommandChild};
use std::{fs, path::PathBuf, sync::atomic::AtomicU64};

struct Fixture {
    root: PathBuf,
    slot: WorktreeDiscoverySlot,
}
impl Fixture {
    fn new() -> Self {
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "orchbee-worktree-source-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("repo")).unwrap();
        fs::create_dir(root.join("hooks")).unwrap();
        let fixture = Self {
            root,
            slot: WorktreeDiscoverySlot::new(),
        };
        fixture.git(&["init", "-b", "main"]);
        fixture.git(&["commit", "--allow-empty", "-m", "seed"]);
        fixture
    }
    fn git(&self, args: &[&str]) {
        let mut command = Command::new("git.exe");
        crate::git_repository_windows::configure(&mut command, &self.root.join("repo"), false)
            .unwrap();
        command
            .args([
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "-c",
            ])
            .arg(format!(
                "core.hooksPath={}",
                self.root.join("hooks").display()
            ))
            .arg("-C")
            .arg(self.root.join("repo"))
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit());
        let mut child =
            WindowsCommandChild::spawn(command).unwrap_or_else(|_| panic!("spawn fixture Git"));
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(status.success(), "Git fixture {args:?}");
                break;
            }
            if Instant::now() >= deadline {
                child.settle().unwrap();
                panic!("fixture Git timed out");
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
    fn observe(&self, job: u8, path: &Path, trust: bool) -> JobOutcome {
        assert_eq!(
            self.slot.start_source(JobId([job; 16]), path, trust),
            StartOutcome::Accepted
        );
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            match self.slot.poll(JobId([job; 16])) {
                PollOutcome::Pending if Instant::now() < deadline => thread::sleep(POLL_SLICE),
                PollOutcome::Completed(outcome) => return outcome,
                other => panic!("source query did not settle: {other:?}"),
            }
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if self
            .slot
            .stop_before(Instant::now() + Duration::from_secs(5))
            .is_err()
        {
            eprintln!(
                "source fixture retained for unknown physical settlement: {}",
                self.root.display()
            );
            return;
        }
        let resolved = fs::canonicalize(&self.root).unwrap();
        let temporary = fs::canonicalize(std::env::temp_dir()).unwrap();
        assert_eq!(resolved.parent(), Some(temporary.as_path()));
        fs::remove_dir_all(resolved).unwrap();
    }
}

#[test]
fn main_and_linked_subdirectories_report_checkout_and_common_directory() {
    let fixture = Fixture::new();
    let linked = fixture.root.join("linked");
    fixture.git(&["worktree", "add", "-b", "feature", linked.to_str().unwrap()]);
    fs::create_dir(fixture.root.join("repo/sub")).unwrap();
    fs::create_dir(linked.join("sub")).unwrap();
    // An untracked file is irrelevant to read-only discovery and must survive it.
    fs::write(linked.join("untracked.txt"), "keep").unwrap();
    let before = fs::read(fixture.root.join("repo/.git/config")).unwrap();
    for (job, checkout, trust) in [
        (1, fixture.root.join("repo"), false),
        (2, linked.clone(), true),
    ] {
        let JobOutcome::SourceCompleted {
            common_dir,
            source_checkout,
            listing,
            ..
        } = fixture.observe(job, &checkout.join("sub"), trust)
        else {
            panic!("source result")
        };
        let text = |value: Vec<u8>| {
            String::from_utf8(value)
                .unwrap()
                .trim_end_matches(['\r', '\n'])
                .to_owned()
        };
        assert_eq!(
            text(common_dir),
            canonical_path(&fixture.root.join("repo/.git"))
                .unwrap()
                .as_str()
        );
        assert_eq!(
            text(source_checkout),
            canonical_path(&checkout).unwrap().as_str()
        );
        let listing = String::from_utf8(listing).unwrap();
        assert!(listing.contains("branch refs/heads/main\0"));
        assert!(listing.contains("branch refs/heads/feature\0"));
    }
    assert_eq!(
        fs::read(fixture.root.join("repo/.git/config")).unwrap(),
        before
    );
    assert_eq!(
        fs::read_to_string(linked.join("untracked.txt")).unwrap(),
        "keep"
    );
}

#[test]
fn source_command_trust_is_an_exact_argument_and_invalid_source_is_typed() {
    let fixture = Fixture::new();
    let mut command = Command::new("git.exe");
    crate::git_repository_windows::configure(&mut command, &fixture.root.join("repo"), true)
        .unwrap();
    let args: Vec<_> = command
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        args,
        vec![
            "-c".to_owned(),
            format!(
                "safe.directory={}",
                canonical_path(&fixture.root.join("repo")).unwrap().as_str()
            )
        ]
    );
    assert!(matches!(
        fixture.observe(3, &fixture.root, false),
        JobOutcome::GitFailed {
            stage: Stage::Probe,
            ..
        }
    ));
    assert_eq!(
        fixture
            .slot
            .start_source(JobId([4; 16]), Path::new("relative"), true),
        StartOutcome::InvalidDirectory
    );
}

#[test]
fn unavailable_directory_is_prepared_by_the_owner_and_late_cancellation_never_spawns() {
    let fixture = Fixture::new();
    assert!(matches!(
        fixture.observe(8, &fixture.root.join("missing"), false),
        JobOutcome::SpawnFailed {
            stage: Stage::Probe
        }
    ));
    // An owned preparation failure leaves the physical slot reusable.
    assert!(matches!(
        fixture.observe(9, &fixture.root.join("repo"), false),
        JobOutcome::SourceCompleted { .. }
    ));
    let witness = Arc::new(Mutex::new(JobWitness::default()));
    let mut commands = source_commands(&fixture.root.join("repo"), true);
    commands.test_hooks = Some(TestHooks {
        faults: Faults {
            cancel_after_prepare: true,
            ..Faults::default()
        },
        witness: Arc::clone(&witness),
    });
    let report = run_owner(
        commands,
        Arc::new(AtomicBool::new(false)),
        Instant::now() + Duration::from_secs(5),
    );
    assert_eq!(report.outcome, JobOutcome::Cancelled);
    assert!(report.child_reaped && report.pipes_closed);
    assert!(witness.lock().unwrap().spawned.is_empty());
}
