//! Seeded model check of the title change cursor. A reference model of observed titles runs beside
//! the projection while two readers page through its changes: every answer must be a valid page
//! for its request, a reader that reaches the newest change must hold exactly the observed titles,
//! a reader the projection can no longer resume is reset rather than left stale, the retained
//! history stays bounded, and notes follow the one-outstanding rule.
use super::*;
use orchbee_capability_node_protocol::{
    OperationId, TerminalTitleChangesRequest, TerminalTitleChangesResult, WorkerGeneration,
    WorkerId,
};
use orchbee_terminal_protocol::{CreationId, HostEpoch, TerminalId, TerminalIdentity};
use std::collections::BTreeSet;

type Held = BTreeMap<[u8; 16], (HostTerminalIdentity, NonZeroU64, Option<String>)>;

const REQUIRED: &[&str] = &[
    "incomplete-by-rows",
    "incomplete-by-bytes",
    "continued-page",
    "unavailable-row",
    "fresh-skips-removal",
    "reset-below-floor",
    "reset-ahead",
    "identity-replaced",
    "cleared-title",
    "note-written",
    "note-suppressed",
];

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

#[derive(Default)]
struct Reader {
    cursor: u64,
    held: Held,
    complete: bool,
}

struct Run {
    titles: Titles,
    model: Held,
    /// Creation byte of each terminal's current identity; a re-observed terminal gets a new one.
    creation: BTreeMap<u16, u8>,
    revision: BTreeMap<u16, u64>,
    answered: u64,
    note_outstanding: bool,
    coverage: BTreeSet<&'static str>,
    seed: u64,
}

fn key(terminal: u16) -> [u8; 16] {
    (u128::from(terminal) + 1).to_be_bytes()
}

fn event(
    terminal: u16,
    creation: u8,
    revision: u64,
    title: Option<String>,
) -> TerminalMetadataChanged {
    TerminalMetadataChanged {
        identity: TerminalIdentity {
            creation_id: CreationId::from_bytes([creation; 16]).unwrap(),
            host_epoch: HostEpoch::from_bytes([2; 16]).unwrap(),
            terminal_id: TerminalId::from_bytes(key(terminal)).unwrap(),
        },
        revision: NonZeroU64::new(revision).unwrap(),
        title,
        current_directory: None,
    }
}

fn title_bytes(observation: &TerminalTitleObservation) -> usize {
    match observation {
        TerminalTitleObservation::Observed {
            title: Some(title), ..
        } => title.len(),
        _ => 0,
    }
}

impl Run {
    fn observe(&mut self, rng: &mut Rng, terminal: u16, big: bool) {
        let creation = *self.creation.entry(terminal).or_insert(1);
        let current = self.revision.get(&terminal).copied().unwrap_or(0);
        let replay = current > 0 && rng.below(8) == 0;
        let revision = if replay { current } else { current + 1 };
        let title = if replay {
            self.model
                .get(&key(terminal))
                .and_then(|(_, _, t)| t.clone())
        } else {
            match rng.below(10) {
                0 => None,
                _ if big => Some("x".repeat(64 * 1024)),
                n => Some(format!(
                    "\u{280b} {terminal} {n} {}",
                    "\u{6807}\u{9898}".repeat(rng.below(4) as usize)
                )),
            }
        };
        let event = event(terminal, creation, revision, title.clone());
        let identity =
            crate::terminal_host_launch_adapter::capability_identity(event.identity).unwrap();
        let before = self.titles.newest;
        self.titles.observe(event).unwrap();
        if replay {
            assert_eq!(
                self.titles.newest, before,
                "seed {}: a replay is no change",
                self.seed
            );
            return;
        }
        if title.is_none() {
            self.coverage.insert("cleared-title");
        }
        self.revision.insert(terminal, revision);
        self.model.insert(
            key(terminal),
            (identity, NonZeroU64::new(revision).unwrap(), title),
        );
    }

    fn remove(&mut self, terminal: u16) {
        let Some((identity, _, _)) = self.model.remove(&key(terminal)) else {
            return;
        };
        self.titles.remove(identity);
        self.revision.remove(&terminal);
        let creation = self.creation.entry(terminal).or_insert(1);
        *creation = creation.wrapping_add(1).max(1);
    }

    fn read(&mut self, reader: &mut Reader) {
        let request = TerminalTitleChangesRequest {
            request_id: OperationId::from_bytes([7; 16]).unwrap(),
            worker_id: WorkerId::from_bytes([8; 16]).unwrap(),
            worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
            after: reader.cursor,
        };
        let floor = self.titles.floor;
        let newest = self.titles.newest;
        let result = TerminalTitleChangesResult {
            request,
            outcome: self.titles.changes(reader.cursor),
        };
        assert!(
            result.answers(&request),
            "seed {}: invalid answer to cursor {}",
            self.seed,
            reader.cursor
        );
        match result.outcome {
            TerminalTitleChangesOutcome::Page(page) => {
                if reader.cursor == 0 && page.rows.is_empty() && page.next > 0 {
                    self.coverage.insert("fresh-skips-removal");
                }
                if !page.complete {
                    let bytes: usize = page
                        .rows
                        .iter()
                        .map(|row| title_bytes(&row.observation))
                        .sum();
                    self.coverage
                        .insert(if page.rows.len() == MAXIMUM_TERMINAL_TITLE_CHANGES {
                            "incomplete-by-rows"
                        } else {
                            assert!(
                                bytes + 64 * 1024 > MAXIMUM_TERMINAL_TITLE_CHANGE_BYTES,
                                "seed {}: a page stopped below both bounds",
                                self.seed
                            );
                            "incomplete-by-bytes"
                        });
                }
                if !reader.complete && reader.cursor != 0 {
                    self.coverage.insert("continued-page");
                }
                for row in page.rows {
                    let key = row.identity.terminal_id().as_bytes();
                    match row.observation {
                        TerminalTitleObservation::Observed { revision, title } => {
                            if reader
                                .held
                                .get(&key)
                                .is_some_and(|(identity, _, _)| *identity != row.identity)
                            {
                                self.coverage.insert("identity-replaced");
                            }
                            reader.held.insert(key, (row.identity, revision, title));
                        }
                        TerminalTitleObservation::Unavailable => {
                            self.coverage.insert("unavailable-row");
                            reader.held.remove(&key);
                        }
                    }
                }
                reader.cursor = page.next;
                reader.complete = page.complete;
                if page.complete {
                    assert_eq!(page.next, newest, "seed {}", self.seed);
                    assert!(
                        reader.held == self.model,
                        "seed {}: a reader at the newest change holds other titles",
                        self.seed
                    );
                }
                self.answered = page.next;
                self.note_outstanding = !page.complete;
            }
            TerminalTitleChangesOutcome::Reset => {
                if reader.cursor > newest {
                    self.coverage.insert("reset-ahead");
                } else {
                    assert!(
                        reader.cursor < floor,
                        "seed {}: resumable cursor {} was reset below floor {floor}",
                        self.seed,
                        reader.cursor
                    );
                    self.coverage.insert("reset-below-floor");
                }
                *reader = Reader {
                    complete: true,
                    ..Reader::default()
                };
                self.answered = 0;
                self.note_outstanding = false;
            }
            TerminalTitleChangesOutcome::RejectedFencing => {
                panic!("seed {}: the projection never rejects fencing", self.seed)
            }
        }
    }

    /// Reads until the reader holds the newest change. Without changes in between, a reset can
    /// happen at most once and every other read returns at least one row or completes.
    fn drain(&mut self, reader: &mut Reader) {
        let limit = 2 * self.titles.changes.len() + 4;
        for _ in 0..limit {
            if reader.complete && reader.cursor >= self.titles.newest {
                return;
            }
            self.read(reader);
        }
        panic!(
            "seed {}: a quiet projection never completed a read",
            self.seed
        );
    }

    fn idle(&mut self) {
        match self.titles.take_note() {
            Some(note) => {
                assert!(!self.note_outstanding, "seed {}: a second note", self.seed);
                assert_eq!(note.through.get(), self.titles.newest, "seed {}", self.seed);
                assert!(note.through.get() > self.answered, "seed {}", self.seed);
                self.note_outstanding = true;
                self.coverage.insert("note-written");
            }
            None => {
                assert!(
                    self.note_outstanding || self.titles.newest <= self.answered,
                    "seed {}: a due note was withheld",
                    self.seed
                );
                if self.note_outstanding && self.titles.newest > self.answered {
                    self.coverage.insert("note-suppressed");
                }
            }
        }
    }

    fn check_bounds(&self) {
        let titles = &self.titles;
        assert_eq!(
            titles.changes.len(),
            titles.rows.len() + titles.removed.len()
        );
        assert!(titles.removed.len() <= MAXIMUM_REMOVED);
        assert!(titles.rows.keys().all(|k| !titles.removed.contains_key(k)));
        // A fresh read's next page must resume at or above the floor, or it would reset forever.
        assert!(
            titles
                .changes
                .keys()
                .next()
                .is_none_or(|first| *first > titles.floor),
            "seed {}: a change at or below the floor",
            self.seed
        );
        assert_eq!(
            titles.bytes,
            titles
                .rows
                .values()
                .map(|row| row.title.as_ref().map_or(0, String::len))
                .sum::<usize>()
        );
    }
}

#[test]
fn change_cursor_pages_resets_and_notes_follow_the_model() {
    let mut coverage = BTreeSet::new();
    for seed in 1..=64_u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let mut run = Run {
            titles: Titles::default(),
            model: Held::new(),
            creation: BTreeMap::new(),
            revision: BTreeMap::new(),
            answered: 0,
            note_outstanding: false,
            coverage: BTreeSet::new(),
            seed,
        };
        let mut primary = Reader {
            complete: true,
            ..Reader::default()
        };
        let mut stale = Reader {
            complete: true,
            ..Reader::default()
        };
        let terminals = 200 + rng.below(200) as u16;
        let big = seed % 2 == 0;
        // A burst wider than one page, before any read.
        for terminal in 0..terminals.min(300) {
            let large = big && rng.below(10) == 0;
            run.observe(&mut rng, terminal, large);
        }
        run.idle();
        run.read(&mut stale);
        let churn = seed % 8 == 0;
        if seed % 16 == 4 {
            // Every terminal leaves before the first fresh read.
            for terminal in 0..terminals {
                run.remove(terminal);
            }
            let mut fresh = Reader {
                complete: true,
                ..Reader::default()
            };
            run.read(&mut fresh);
        }
        let steps = if churn { 2_400 } else { 900 };
        for step in 0..steps {
            if churn {
                // Fresh terminals come and go, so removals outgrow the retained history.
                let terminal = 400 + (step / 2) as u16;
                if step % 2 == 0 {
                    run.observe(&mut rng, terminal, false);
                } else {
                    run.remove(terminal);
                }
            } else {
                match rng.below(20) {
                    0..=9 => {
                        let terminal = rng.below(u64::from(terminals)) as u16;
                        let large = big && rng.below(30) == 0;
                        run.observe(&mut rng, terminal, large);
                    }
                    10..=12 => run.remove(rng.below(u64::from(terminals)) as u16),
                    13..=15 => run.idle(),
                    _ => {}
                }
            }
            // The Server continues an incomplete page at once; otherwise it reads now and then.
            if !primary.complete || rng.below(12) == 0 {
                run.read(&mut primary);
            }
            run.check_bounds();
        }
        run.drain(&mut primary);
        // The stale reader returns once, possibly after its cursor fell below the floor.
        run.read(&mut stale);
        run.drain(&mut stale);
        // A cursor ahead of the newest change belongs to no history this projection kept.
        let mut ahead = Reader {
            cursor: run.titles.newest + 1,
            complete: true,
            ..Reader::default()
        };
        run.read(&mut ahead);
        coverage.extend(run.coverage);
    }
    for class in REQUIRED {
        assert!(coverage.contains(class), "uncovered class {class}");
    }
}
