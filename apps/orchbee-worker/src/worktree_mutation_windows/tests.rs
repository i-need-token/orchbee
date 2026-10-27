use super::*;
use std::{
    fs,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    thread,
    time::Duration,
};

static SERIAL: AtomicU64 = AtomicU64::new(0);

#[test]
fn git_diagnostics_keep_the_error_after_excessive_progress() {
    let fixture = Fixture::new();
    let cancel = AtomicBool::new(false);
    let mut context = Context {
        cancel: &cancel,
        deadline: Instant::now() + Duration::from_secs(10),
        trust: false,
        changed: false,
        stage: Stage::Probe,
    };
    let output = context.git(fixture.source.as_str(), &[
        "-c", "alias.diagnostic=!printf '%05000d' 0 >&2; printf 'fatal: fixture checkout failed\\n' >&2; exit 128",
        "diagnostic",
    ], Stage::Probe, false).unwrap_or_else(|_| panic!("settled Git fixture"));
    assert_eq!(output.status.code(), Some(128));
    assert_eq!(output.stderr.len(), 2048);
    assert!(output.stderr.ends_with(b"fatal: fixture checkout failed\n"));
}

struct Fixture {
    root: PathBuf,
    source: AbsoluteCwd,
    uncertain: std::cell::Cell<bool>,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "orchbee-worktree-mutation-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("repo")).unwrap();
        fs::create_dir(root.join("hooks")).unwrap();
        let fixture = Self {
            source: canonical_path(&root.join("repo")).unwrap(),
            root,
            uncertain: std::cell::Cell::new(false),
        };
        fixture.git(&["init", "-b", "main"]);
        fixture.git(&["config", "user.name", "Orchbee Fixture"]);
        fixture.git(&["config", "user.email", "fixture@example.invalid"]);
        fixture.git(&["config", "commit.gpgsign", "false"]);
        fixture.git(&[
            "config",
            "core.hooksPath",
            canonical_path(&fixture.root.join("hooks"))
                .unwrap()
                .as_str(),
        ]);
        fs::write(fixture.root.join("repo/seed.txt"), b"seed\n").unwrap();
        fixture.git(&["add", "seed.txt"]);
        fixture.git(&["commit", "-m", "seed"]);
        fixture
    }
    fn git(&self, args: &[&str]) -> Vec<u8> {
        let cancel = AtomicBool::new(false);
        let mut context = Context {
            cancel: &cancel,
            deadline: Instant::now() + Duration::from_secs(10),
            trust: false,
            changed: false,
            stage: Stage::Probe,
        };
        context
            .required(self.source.as_str(), args, Stage::Mutate, true)
            .unwrap_or_else(|error| match error {
                Abort::Known(failure) => panic!("fixture Git failed: {failure:?}"),
                Abort::PhysicalUnknown => {
                    self.uncertain.set(true);
                    panic!("fixture Git settlement unknown");
                }
            })
    }
    fn path(&self, name: &str) -> AbsoluteCwd {
        AbsoluteCwd::new(self.root.join(name).to_str().unwrap()).unwrap()
    }
    fn intent(&self, mutation: WorktreeMutation) -> WorktreeMutationIntent {
        WorktreeMutationIntent::new(self.source.clone(), false, mutation).unwrap()
    }
    fn execute(&self, mutation: WorktreeMutation) -> Completion {
        run(
            self.intent(mutation),
            &AtomicBool::new(false),
            Instant::now() + Duration::from_secs(15),
        )
        .unwrap_or_else(|()| {
            self.uncertain.set(true);
            panic!("fixture mutation settlement unknown");
        })
    }
    fn create(&self, name: &str, branch: &str, base: &str) -> WorktreeCheckout {
        let result = self.execute(WorktreeMutation::Create {
            path: self.path(name),
            branch: GitReference::new(branch).unwrap(),
            base: GitReference::new(base).unwrap(),
        });
        let Completion::Checkout(checkout) = result else {
            panic!("create failed: {result:?}")
        };
        checkout
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if self.uncertain.get() {
            eprintln!(
                "Retained owned Git fixture with unproven cleanup: {}",
                self.root.display()
            );
            return;
        }
        // Every runner above returns only after Job settlement. Never touch a
        // pre-existing name or a reparse-point replacement during fixture cleanup.
        use std::os::windows::fs::MetadataExt;
        let temp = fs::canonicalize(std::env::temp_dir()).unwrap();
        let root = fs::canonicalize(&self.root).unwrap();
        assert!(root.starts_with(&temp) && root != temp);
        assert_eq!(
            fs::symlink_metadata(&self.root).unwrap().file_attributes() & 0x400,
            0
        );
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn native_create_open_dirty_refusal_force_remove_and_branch_preservation() {
    let fixture = Fixture::new();
    let checkout = fixture.create("checkout one", "topic/one", "HEAD");
    assert_eq!(checkout.repository_root(), &fixture.source);
    assert!(!checkout.dirty());
    assert!(fixture.root.join("checkout one/seed.txt").is_file());
    let opened = fixture.execute(WorktreeMutation::Open(WorktreeCheckoutSelector::Branch(
        GitReference::new("topic/one").unwrap(),
    )));
    assert_eq!(opened, Completion::Checkout(checkout.clone()));
    fs::write(fixture.root.join("checkout one/untracked.txt"), b"keep me").unwrap();
    let observed = fixture.execute(WorktreeMutation::InspectRemove {
        path: checkout.path().clone(),
    });
    assert!(matches!(observed, Completion::Checkout(value) if value.dirty()));
    let remove = |force| WorktreeMutation::Remove {
        path: checkout.path().clone(),
        expected_common_directory: checkout.common_directory().clone(),
        force,
    };
    let refused = fixture.execute(remove(false));
    assert!(
        matches!(refused, Completion::Failed(failure) if failure.error == Error::Dirty && failure.effect == WorktreeMutationEffect::None)
    );
    assert_eq!(
        fs::read(fixture.root.join("checkout one/untracked.txt")).unwrap(),
        b"keep me"
    );
    assert_eq!(
        fixture.execute(remove(true)),
        Completion::Removed {
            path: checkout.path().clone()
        }
    );
    assert!(!fixture.root.join("checkout one").exists());
    assert!(
        !fixture
            .git(&["show-ref", "--verify", "refs/heads/topic/one"])
            .is_empty()
    );
    assert!(
        matches!(fixture.execute(remove(true)), Completion::Failed(failure) if failure.error == Error::CheckoutNotFound)
    );
}

#[test]
fn existing_branch_ignores_base_and_wrong_repository_or_main_is_never_removed() {
    let fixture = Fixture::new();
    fixture.git(&["branch", "existing"]);
    let checkout = fixture.create("existing", "existing", "missing-base");
    assert_eq!(checkout.branch().unwrap().as_str(), "existing");
    assert!(
        matches!(fixture.execute(WorktreeMutation::Remove { path: checkout.path().clone(), expected_common_directory: fixture.source.clone(), force: true }),
        Completion::Failed(failure) if failure.error == Error::RepositoryChanged && failure.effect == WorktreeMutationEffect::None)
    );
    assert!(
        matches!(fixture.execute(WorktreeMutation::Remove { path: fixture.source.clone(), expected_common_directory: checkout.common_directory().clone(), force: true }),
        Completion::Failed(failure) if failure.error == Error::MainCheckout)
    );
    assert!(fixture.root.join("repo/seed.txt").exists());
    assert!(fixture.root.join("existing/seed.txt").exists());
}

#[test]
fn cancellation_before_effect_and_output_overflow_have_known_cleanup() {
    let fixture = Fixture::new();
    let intent = fixture.intent(WorktreeMutation::Create {
        path: fixture.path("cancelled"),
        branch: GitReference::new("cancelled").unwrap(),
        base: GitReference::new("HEAD").unwrap(),
    });
    let result = run(
        intent,
        &AtomicBool::new(true),
        Instant::now() + Duration::from_secs(5),
    )
    .unwrap();
    assert!(
        matches!(result, Completion::Failed(failure) if failure.error == Error::Cancelled && failure.effect == WorktreeMutationEffect::None)
    );
    assert!(!fixture.root.join("cancelled").exists());
    use std::io::Write;
    let mut config = fs::OpenOptions::new()
        .append(true)
        .open(fixture.root.join("repo/.git/config"))
        .unwrap();
    writeln!(config, "\n[test]\nlarge = {}", "x".repeat(70_000)).unwrap();
    drop(config);
    let cancel = AtomicBool::new(false);
    let mut context = Context {
        cancel: &cancel,
        deadline: Instant::now() + Duration::from_secs(5),
        trust: false,
        changed: false,
        stage: Stage::Probe,
    };
    assert!(
        matches!(context.git(fixture.source.as_str(), &["config", "--local", "--get", "test.large"], Stage::Probe, false), Err(Abort::Known(failure)) if failure.error == Error::TooLarge && failure.effect == WorktreeMutationEffect::None)
    );
}

#[test]
fn cancellation_during_post_checkout_hook_preserves_possible_effects() {
    let fixture = Fixture::new();
    fs::write(
        fixture.root.join("hooks/post-checkout"),
        b"#!/bin/sh\nprintf ready > ../mutation-ready\nsleep 30\n",
    )
    .unwrap();
    let cancel = Arc::new(AtomicBool::new(false));
    let owner_cancel = Arc::clone(&cancel);
    let intent = fixture.intent(WorktreeMutation::Create {
        path: fixture.path("hooked"),
        branch: GitReference::new("hooked").unwrap(),
        base: GitReference::new("HEAD").unwrap(),
    });
    let owner = thread::spawn(move || {
        run(
            intent,
            &owner_cancel,
            Instant::now() + Duration::from_secs(20),
        )
    });
    let deadline = Instant::now() + Duration::from_secs(8);
    while !fixture.root.join("mutation-ready").exists()
        && !owner.is_finished()
        && Instant::now() < deadline
    {
        thread::sleep(Duration::from_millis(10));
    }
    let reached = fixture.root.join("mutation-ready").exists();
    cancel.store(true, Ordering::Relaxed);
    let result = match owner.join() {
        Ok(Ok(result)) => result,
        _ => {
            fixture.uncertain.set(true);
            panic!("hook owner cleanup unknown");
        }
    };
    assert!(
        reached,
        "Git did not enter the controlled checkout hook: {result:?}"
    );
    assert!(
        matches!(result, Completion::Failed(failure) if failure.error == Error::Cancelled && failure.effect == WorktreeMutationEffect::MayHaveChanged)
    );
}

#[test]
fn ambiguous_branch_requires_path_and_trust_accepts_a_source_subdirectory() {
    let fixture = Fixture::new();
    let first = fixture.create("first", "duplicate", "HEAD");
    fixture.git(&[
        "worktree",
        "add",
        "--force",
        fixture.path("second").as_str(),
        "duplicate",
    ]);
    assert!(
        matches!(fixture.execute(WorktreeMutation::Open(WorktreeCheckoutSelector::Branch(GitReference::new("duplicate").unwrap()))),
        Completion::Failed(failure) if failure.error == Error::AmbiguousCheckout && failure.effect == WorktreeMutationEffect::None)
    );
    fs::create_dir(fixture.root.join("repo/subdirectory")).unwrap();
    let intent = WorktreeMutationIntent::new(
        canonical_path(&fixture.root.join("repo/subdirectory")).unwrap(),
        true,
        WorktreeMutation::Open(WorktreeCheckoutSelector::Path(first.path().clone())),
    )
    .unwrap();
    let result = run(
        intent,
        &AtomicBool::new(false),
        Instant::now() + Duration::from_secs(10),
    )
    .unwrap_or_else(|()| {
        fixture.uncertain.set(true);
        panic!("trusted open cleanup unknown");
    });
    assert_eq!(result, Completion::Checkout(first));
}

#[test]
fn malformed_listing_never_selects_an_unproven_checkout() {
    let head = "a".repeat(40);
    for text in [
        "".to_owned(),
        "worktree C:/repo\0".to_owned(),
        format!("worktree relative\0HEAD {head}\0branch refs/heads/main\0\0"),
        format!("worktree C:/repo\0HEAD {head}\0HEAD {head}\0branch refs/heads/main\0\0"),
        format!("worktree C:/repo\0HEAD {head}\0branch refs/heads/main\0detached\0\0"),
        "worktree C:/repo\0bare\0branch refs/heads/main\0\0".to_owned(),
    ] {
        assert!(listing::parse(text.as_bytes()).is_err());
    }
    assert!(
        listing::parse(format!("worktree C:/repo\0HEAD {head}\0detached\0\0").as_bytes()).is_ok()
    );
    assert!(listing::parse(b"worktree C:/repo\0bare\0\0").is_ok());
}

#[test]
fn pre_host_mutation_is_joined_on_explicit_stop_and_server_disconnect() {
    use orchbee_capability_node_protocol::{
        CapabilityNodeMessage as Message, OperationId, ShutdownBudget, WorkerGeneration, WorkerId,
        WorkerShutdownOutcome, WorkerShutdownRequest, WorktreeMutationJobAction,
        WorktreeMutationOutcome, WorktreeMutationRequest, WorktreeMutationStartOutcome,
        read_message, write_message,
    };
    use orchbee_local_ipc::LocalStream;
    use orchbee_local_runtime::{Cohort, ModeCheckedLocalSessionRendezvous};
    use std::num::NonZeroU64;
    for explicit_stop in [true, false] {
        let fixture = Fixture::new();
        fs::write(
            fixture.root.join("hooks/post-checkout"),
            b"#!/bin/sh\nprintf ready > ../mutation-ready\nsleep 30\n",
        )
        .unwrap();
        let cohort_path = fixture.root.join("cohort");
        fs::create_dir(&cohort_path).unwrap();
        for name in [
            "orchbee",
            "orchbee-server",
            "orchbee-worker",
            "orchbee-terminal-host",
        ] {
            fs::write(cohort_path.join(format!("{name}.exe")), b"native fixture\n").unwrap();
        }
        let digest = "396cfec2d1c795ce71dce2d9c92f29cddec638632050c17861d771936a99282d";
        fs::write(cohort_path.join("cohort.json"), format!(r#"{{"schemaVersion":1,"artifacts":{{"orchbee":"{digest}","orchbee-server":"{digest}","orchbee-worker":"{digest}","orchbee-terminal-host":"{digest}","agent-observation-adapters":"{digest}"}}}}"#)).unwrap();
        let cohort = Cohort::load(&cohort_path, Instant::now() + Duration::from_secs(3)).unwrap();
        let runtime = fixture.root.join("runtime");
        fs::create_dir(&runtime).unwrap();
        let worker_runtime = runtime.clone();
        let (mut server, worker) = LocalStream::pair().unwrap();
        server
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        server
            .set_write_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let owner = thread::spawn(move || {
            crate::terminal_operations::run_lazy(
                worker,
                orchbee_local_worker_protocol::WorkerId::from_bytes([2; 16]),
                NonZeroU64::new(3).unwrap(),
                &cohort,
                &worker_runtime,
                None,
            )
            .is_ok()
        });
        // Retain the fixture on any panic before joining this physical consumer.
        fixture.uncertain.set(true);
        let start = WorktreeMutationRequest {
            operation_id: OperationId::from_bytes([1; 16]).unwrap(),
            worker_id: WorkerId::from_bytes([2; 16]).unwrap(),
            worker_generation: WorkerGeneration::new(NonZeroU64::new(3).unwrap()),
            action: WorktreeMutationJobAction::Start(fixture.intent(WorktreeMutation::Create {
                path: fixture.path("hooked"),
                branch: GitReference::new("hooked").unwrap(),
                base: GitReference::new("HEAD").unwrap(),
            })),
        };
        for expected in [
            WorktreeMutationStartOutcome::Accepted,
            WorktreeMutationStartOutcome::Existing,
        ] {
            write_message(
                &mut server,
                &Message::WorktreeMutationRequest(start.clone()),
            )
            .unwrap();
            let Message::WorktreeMutationResult(result) = read_message(&mut server).unwrap() else {
                panic!("wrong reply")
            };
            assert_eq!(result.operation_id, start.operation_id);
            assert_eq!(result.job_id, start.operation_id);
            assert_eq!(result.outcome, WorktreeMutationOutcome::Start(expected));
        }
        let deadline = Instant::now() + Duration::from_secs(8);
        while !fixture.root.join("mutation-ready").exists()
            && !owner.is_finished()
            && Instant::now() < deadline
        {
            thread::sleep(Duration::from_millis(10));
        }
        let reached = fixture.root.join("mutation-ready").exists();
        if explicit_stop {
            write_message(
                &mut server,
                &Message::WorkerShutdownRequest(WorkerShutdownRequest {
                    operation_id: OperationId::from_bytes([5; 16]).unwrap(),
                    worker_id: start.worker_id,
                    worker_generation: start.worker_generation,
                    budget: ShutdownBudget::new(8_000).unwrap(),
                }),
            )
            .unwrap();
            let Message::WorkerShutdownResult(result) = read_message(&mut server).unwrap() else {
                panic!("wrong shutdown reply")
            };
            assert_eq!(result.outcome, WorkerShutdownOutcome::NoHost);
        }
        drop(server);
        let deadline = Instant::now() + Duration::from_secs(8);
        while !owner.is_finished() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(
            owner.is_finished(),
            "Worker did not settle the mutation on exit"
        );
        let stopped = owner.join().unwrap();
        fixture.uncertain.set(false);
        assert!(reached, "Git did not enter the controlled hook");
        assert_eq!(stopped, explicit_stop);
        assert!(!ModeCheckedLocalSessionRendezvous::host_runtime_directory(&runtime).exists());
    }
}

#[test]
fn source_probe_resolves_subdirectories_and_open_includes_main_checkout() {
    let fixture = Fixture::new();
    for selector in [
        WorktreeCheckoutSelector::Path(fixture.source.clone()),
        WorktreeCheckoutSelector::Branch(GitReference::new("main").unwrap()),
    ] {
        let Completion::Checkout(checkout) = fixture.execute(WorktreeMutation::Open(selector))
        else {
            panic!("main checkout must open")
        };
        assert_eq!(checkout.path(), &fixture.source);
        assert_eq!(checkout.repository_root(), &fixture.source);
    }
    let linked = fixture.create("probe", "probe", "HEAD");
    for checkout in [&fixture.source, linked.path()] {
        let subdirectory = Path::new(checkout.as_str()).join("nested");
        std::fs::create_dir(&subdirectory).unwrap();
        let intent = WorktreeMutationIntent::new(
            canonical_path(&subdirectory).unwrap(),
            false,
            WorktreeMutation::InspectSource,
        )
        .unwrap();
        let Completion::Checkout(observed) = run(
            intent,
            &AtomicBool::new(false),
            Instant::now() + Duration::from_secs(30),
        )
        .unwrap() else {
            panic!("source checkout must resolve")
        };
        assert_eq!(observed.path(), checkout);
        assert_eq!(observed.repository_root(), &fixture.source);
        assert_eq!(observed.common_directory(), linked.common_directory());
    }
}

#[test]
fn successful_hook_cannot_hide_a_different_checked_out_branch() {
    let fixture = Fixture::new();
    fixture.git(&["branch", "other"]);
    std::fs::write(
        fixture.root.join("hooks/post-checkout"),
        format!(
            "#!/bin/sh\nunset GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR\ngit -C '{}' symbolic-ref HEAD refs/heads/other\n",
            fixture.path("changed").as_str().replace('\'', "'\\''")
        ),
    )
    .unwrap();
    let result = fixture.execute(WorktreeMutation::Create {
        path: fixture.path("changed"),
        branch: GitReference::new("requested").unwrap(),
        base: GitReference::new("HEAD").unwrap(),
    });
    assert!(
        matches!(&result, Completion::Failed(failure) if failure.error == Error::RepositoryChanged && failure.effect == WorktreeMutationEffect::MayHaveChanged),
        "unexpected completion: {result:?}"
    );
    assert!(fixture.root.join("changed/seed.txt").exists());
}
