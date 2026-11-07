//! The record rule under an injected clock: an entry only on a layout change and at least the
//! interval after the newest one, at most the kept maximum, oldest removed first.
use super::*;
use crate::test_support::WorkbenchFixture;
use orchbee_server::WorkbenchAuthority;
use orchbee_workbench_protocol::{PresentationBinding, SessionId, TerminalId, WorkerId};

#[derive(Default)]
struct Memory(Mutex<Vec<(u64, Vec<u8>)>>);

impl LayoutHistoryStore for Memory {
    fn entries(&self) -> io::Result<Vec<(u64, Option<Vec<u8>>)>> {
        let mut entries: Vec<_> = self
            .0
            .lock()
            .unwrap()
            .iter()
            .map(|(at, payload)| (*at, Some(payload.clone())))
            .collect();
        entries.sort_by_key(|entry| std::cmp::Reverse(entry.0));
        Ok(entries)
    }

    fn entry(&self, saved_at: u64) -> io::Result<Option<Vec<u8>>> {
        Ok(self
            .0
            .lock()
            .unwrap()
            .iter()
            .find(|(at, _)| *at == saved_at)
            .map(|(_, payload)| payload.clone()))
    }

    fn record(&self, saved_at: u64, payload: &[u8], keep: usize) -> io::Result<()> {
        let mut entries = self.0.lock().unwrap();
        entries.push((saved_at, payload.to_vec()));
        entries.sort_by_key(|entry| std::cmp::Reverse(entry.0));
        entries.truncate(keep);
        Ok(())
    }
}

/// Checkpoints of one Workspace with 0, 1, 2, ... Tabs; each has a different layout.
fn layouts(count: usize) -> Vec<Vec<u8>> {
    let worker = WorkerId::parse(&"09".repeat(16)).unwrap();
    let mut authority =
        WorkbenchAuthority::workspace_fixture(SessionId::parse(&"11".repeat(16)).unwrap()).unwrap();
    let mut layouts = vec![authority.checkpoint(&worker).unwrap()];
    for tab in 1..count {
        authority
            .append_fixture_tab(PresentationBinding {
                terminal_id: TerminalId::parse(&format!("{:032x}", 0x100 + tab)).unwrap(),
                worker_id: worker.clone(),
            })
            .unwrap();
        layouts.push(authority.checkpoint(&worker).unwrap());
    }
    layouts
}

fn at(minutes: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_800_000_000 + minutes * 60)
}

#[test]
fn entries_follow_layout_changes_spaced_by_the_interval_and_stay_bounded() {
    let store = Arc::new(Memory::default());
    let history = LayoutHistory::load(Arc::clone(&store) as _).unwrap();
    let layouts = layouts(MAXIMUM_LAYOUT_HISTORY_ENTRIES + 3);
    let count = || store.0.lock().unwrap().len();

    // An empty history takes the first save at once.
    assert!(history.after_save(&layouts[0], at(0)).unwrap());
    // A change inside the interval waits; an unchanged layout never records.
    assert!(!history.after_save(&layouts[1], at(14)).unwrap());
    assert!(!history.after_save(&layouts[0], at(30)).unwrap());
    assert_eq!(count(), 1);
    // A change at the interval records; a clock moved back does not.
    assert!(history.after_save(&layouts[1], at(15)).unwrap());
    assert!(!history.after_save(&layouts[2], at(10)).unwrap());

    // The maximum holds, and the oldest go first.
    for (index, layout) in layouts.iter().enumerate().skip(2) {
        let minutes = 15 * u64::try_from(index).unwrap();
        assert!(history.after_save(layout, at(minutes)).unwrap());
    }
    assert_eq!(count(), MAXIMUM_LAYOUT_HISTORY_ENTRIES);
    let saved: Vec<u64> = store.0.lock().unwrap().iter().map(|(at, _)| *at).collect();
    let newest = 15 * u64::try_from(layouts.len() - 1).unwrap();
    assert_eq!(saved[0], unix_milliseconds(at(newest)).unwrap());
    assert!(saved.windows(2).all(|pair| pair[0] > pair[1]));

    // A reload indexes what storage kept, so the next decision continues from the newest.
    let reloaded = LayoutHistory::load(Arc::clone(&store) as _).unwrap();
    let empty = WorkbenchAuthority::new(SessionId::parse(&"11".repeat(16)).unwrap()).unwrap();
    let summaries = reloaded.summaries(&empty.open_layout()).unwrap();
    assert_eq!(summaries.len(), MAXIMUM_LAYOUT_HISTORY_ENTRIES);
    assert!(summaries.iter().all(|entry| entry.workspaces_not_open == 1));
    assert!(!reloaded.after_save(&layouts[0], at(newest + 14)).unwrap());
    assert!(reloaded.entry(saved[0]).unwrap().is_some());
    assert!(reloaded.entry(saved[0] + 1).unwrap().is_none());
}
