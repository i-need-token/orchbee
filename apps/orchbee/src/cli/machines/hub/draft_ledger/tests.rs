use super::*;
use crate::cli::machines::hub::{
    catalog::Machines,
    lifetime::{Lifetime, Moment},
    state::Hub,
};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

struct TempRoot(PathBuf);

impl TempRoot {
    fn new(name: &str) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        let dir = std::env::temp_dir().join(format!(
            "orchbee-draft-ledger-{name}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        #[cfg(not(windows))]
        {
            use std::os::unix::fs::DirBuilderExt;
            fs::DirBuilder::new()
                .mode(0o700)
                .create(&dir)
                .expect("create private temp dir");
        }
        #[cfg(windows)]
        fs::create_dir(&dir).expect("create temp dir");
        Self(dir)
    }

    fn data_root(&self) -> DataRoot {
        DataRoot::at(self.0.clone())
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        drop(fs::remove_dir_all(&self.0));
    }
}

fn id(hex: &str) -> MachineId {
    MachineId::new(hex).expect("valid hex id")
}

fn shared_with_ledger(root: &DataRoot) -> Arc<Shared> {
    let drafts = DraftLedger::open(root);
    let lifetime = Lifetime::start(Duration::from_secs(600), false, Moment::now());
    Arc::new_cyclic(|this| {
        Shared::new(Hub::new(
            root.clone(),
            lifetime,
            Machines::empty(),
            drafts,
            Duration::from_secs(30),
            this.clone(),
        ))
    })
}

#[test]
fn killed_hub_unsaved_draft_is_swept_on_restart_while_saved_is_kept() {
    let temp = TempRoot::new("sweep");
    let root = temp.data_root();
    let draft_unsaved = id("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    let draft_saved = id("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");

    {
        let mut ledger = DraftLedger::open(&root);
        ledger.record_draft(&draft_unsaved);
        ledger.record_draft(&draft_saved);
        ledger.persister().flush();
        // Simulated hub kill: dropped without shutdown deletion.
    }

    let mut reopened = DraftLedger::open(&root);
    let sweep = reopened.reconcile_catalog(|candidate| candidate == &draft_saved);
    assert_eq!(sweep, vec![draft_unsaved.clone()]);
    for orphan in &sweep {
        reopened.finish_sweep(orphan, true);
    }
    reopened.persister().flush();

    let mut final_open = DraftLedger::open(&root);
    assert!(final_open.reconcile_catalog(|_| false).is_empty());
}

#[test]
fn dropped_draft_stays_on_disk_until_forget_completes() {
    let temp = TempRoot::new("forget-flight");
    let root = temp.data_root();
    let draft = id("cccccccccccccccccccccccccccccccc");

    {
        let mut ledger = DraftLedger::open(&root);
        ledger.record_draft(&draft);
        ledger.begin_forget(&draft);
        ledger.begin_forget(&draft);
        ledger.finish_forget(&draft, true);
        ledger.persister().flush();
        // Killed while second SecretKind deletion is still in flight.
    }

    let mut next = DraftLedger::open(&root);
    assert_eq!(next.reconcile_catalog(|_| false), vec![draft]);
}

#[test]
fn shutdown_keeps_failed_or_timed_out_deletions_in_ledger_and_drops_deleted() {
    let temp = TempRoot::new("bounded-shutdown");
    let root = temp.data_root();
    let draft_deleted = id("dddddddddddddddddddddddddddddddd");
    let draft_failed = id("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee");
    let draft_slow = id("ffffffffffffffffffffffffffffffff");

    {
        let shared = shared_with_ledger(&root);
        {
            let mut hub = shared.lock();
            hub.drafts.record_draft(&draft_deleted);
            hub.drafts.record_draft(&draft_failed);
            hub.drafts.record_draft(&draft_slow);
        }
        shared.lock().drafts.persister().flush();
        let pending = shared.lock().drafts.pending_ids();
        let deleted_clone = draft_deleted.clone();
        let slow_clone = draft_slow.clone();
        forget_bounded_with(
            &shared,
            &pending,
            Duration::from_millis(80),
            move |candidate| {
                if candidate == &deleted_clone {
                    true
                } else if candidate == &slow_clone {
                    thread::sleep(Duration::from_millis(300));
                    true
                } else {
                    false
                }
            },
        );
    }

    let mut next = DraftLedger::open(&root);
    let orphans = next.reconcile_catalog(|_| false);
    assert!(orphans.contains(&draft_failed), "{orphans:?}");
    assert!(orphans.contains(&draft_slow), "{orphans:?}");
    assert!(!orphans.contains(&draft_deleted), "{orphans:?}");
}
