//! What a Workspace's row paints, name and location line together, while its first terminal changes
//! directory, step by step through the Client's own projection, retention and paint: a committed
//! snapshot, the metadata read the commit triggers, and the Git answer that follows.
use super::*;
use crate::machine_picker::SPACE_HOLD_BOUND;
use crate::machine_spaces::checkpoint::Observation;
use crate::machine_spaces::navigation_projection::SidebarNavigationProjection;
use crate::machine_spaces::{Checkpoint, Git};
use std::time::{Duration, Instant};

const OUTSIDE: &str = "C:/code";

fn on_main() -> Option<Git> {
    Some(Git {
        branch: Some("main".into()),
        divergence: None,
    })
}
fn no_repository() -> Option<Git> {
    Some(Git {
        branch: None,
        divergence: None,
    })
}
fn main_ahead_of_upstream() -> Option<Git> {
    Some(Git {
        branch: Some("main".into()),
        divergence: Some(GitDivergence {
            ahead: 18,
            behind: 0,
        }),
    })
}

/// A Client showing two Workspaces of one repository, of which only `moved` changes directory.
struct Trace {
    view: ClientView,
    snapshot: Snapshot,
    now: Instant,
    /// Each row's painted `name / location line`, one entry per change.
    moved: Vec<String>,
    other: Vec<String>,
}
impl Trace {
    fn new() -> Self {
        Self::after_first_read([on_main(), on_main()])
    }
    fn after_first_read(git: [Option<Git>; 2]) -> Self {
        let directory_id = WorkbenchDirectoryId::parse(&id(200)).unwrap();
        let mut snapshot = crate::machine_agents::test_support::fixture();
        snapshot.revision = WorkbenchRevision::new(2);
        snapshot.navigation.projects = vec![Project {
            project_id: WorkbenchProjectId::parse(&id(70)).unwrap(),
            label: "Repo".into(),
        }];
        snapshot.navigation.directories = vec![Directory {
            directory_id: directory_id.clone(),
            worker_id: WorkerId::parse(&id(201)).unwrap(),
            path: WorkbenchDirectoryPath::parse("C:/repo").unwrap(),
        }];
        snapshot.navigation.workspaces = [(0, "moved"), (1, "other")]
            .into_iter()
            .map(|(n, label)| Workspace {
                terminal_directory: None,
                automatic_label: false,
                project_id: Some(WorkbenchProjectId::parse(&id(70)).unwrap()),
                label: label.into(),
                primary_directory_id: Some(directory_id.clone()),
                workspace_id: WorkbenchWorkspaceId::parse(&id(80 + n)).unwrap(),
                terminal_context_id: WorkbenchTerminalContextId::parse(&id(90 + n)).unwrap(),
            })
            .collect();
        snapshot.navigation.workspace_order = snapshot
            .navigation
            .workspaces
            .iter()
            .map(|workspace| workspace.workspace_id.clone())
            .collect();
        snapshot.terminal_contexts = snapshot
            .navigation
            .workspaces
            .iter()
            .map(|workspace| TerminalContext {
                terminal_context_id: workspace.terminal_context_id.clone(),
                tabs: Vec::new(),
                groups: Vec::new(),
            })
            .collect();
        let mut trace = Self {
            view: view(120, 40),
            snapshot,
            now: Instant::now(),
            moved: Vec::new(),
            other: Vec::new(),
        };
        // Starts from this read alone, not from the answers the shared fixture shows.
        trace
            .view
            .machines
            .forget_space_observations(&Choice::Local);
        trace.read(true, git);
        trace
    }
    fn source(&self) -> SidebarNavigationProjection {
        SidebarNavigationProjection {
            snapshot: std::sync::Arc::new(self.snapshot.clone()),
            pending: None,
        }
    }
    /// The first terminal of `moved` reached `directory`, and the commit reached the Client.
    fn commit(&mut self, directory: Option<&str>) {
        let label = self.snapshot.navigation.workspaces[0].label.clone();
        self.commit_named(&label, directory);
    }
    /// The same commit for an automatic name, which moves with the directory in one revision.
    fn commit_named(&mut self, label: &str, directory: Option<&str>) {
        self.snapshot.revision = WorkbenchRevision::new(self.snapshot.revision.value() + 1);
        self.snapshot.navigation.workspaces[0].label = label.into();
        self.snapshot.navigation.workspaces[0].terminal_directory =
            directory.map(|path| WorkbenchDirectoryPath::parse(path).unwrap());
        let source = self.source();
        self.view
            .machines
            .observe_native_navigation(Choice::Local, Some(source));
        self.tick(Duration::ZERO);
    }
    /// A first visit as the Client sees it: the commit, the metadata read it triggers, the answer.
    fn visit(&mut self, label: &str, directory: &str, git: Option<Git>) {
        self.commit_named(label, Some(directory));
        self.read(false, [None, None]);
        self.read(false, [git, None]);
    }
    /// One UI loop turn `elapsed` after the last; returns whether a hold ended.
    fn tick(&mut self, elapsed: Duration) -> bool {
        self.now += elapsed;
        let ended = self.view.machines.advance_space_holds(self.now);
        self.record();
        ended
    }
    /// A complete read at the committed revision, with its Git answer for each row.
    fn read(&mut self, git_requested: bool, git: [Option<Git>; 2]) {
        let mut checkpoint = Checkpoint::project_navigation(&self.source(), None).unwrap();
        checkpoint.metadata_revision = WorkbenchRevision::new(2);
        checkpoint.git_requested = git_requested;
        let mut rows = checkpoint.rows.to_vec();
        for (row, git) in rows.iter_mut().zip(git) {
            row.git = git;
        }
        checkpoint.rows = rows.into();
        self.view.machines.observe_spaces(vec![(
            Choice::Local,
            Observation::Ready(std::sync::Arc::new(checkpoint)),
        )]);
        self.tick(Duration::ZERO);
    }
    /// Keeps each Workspace's painted name line and location line, one entry per change. An
    /// automatic name in these traces keeps the prefix `moved`.
    fn record(&mut self) {
        let (rows, _) = paint(&self.view, Locale::EnUs, &Theme::rgb_test_dark(), 120, 40);
        for (label, painted) in [("moved", &mut self.moved), ("other", &mut self.other)] {
            let name = rows
                .iter()
                .position(|row| row.contains(label))
                .unwrap_or_else(|| panic!("no row {label}: {rows:?}"));
            let cell = |row: &str| {
                let words = row.split('│').next().unwrap().split_whitespace();
                words.collect::<Vec<_>>().join(" ")
            };
            let step = format!("{} / {}", cell(&rows[name]), cell(&rows[name + 1]));
            if painted.last() != Some(&step) {
                painted.push(step);
            }
        }
    }
}

#[test]
fn a_directory_change_moves_the_location_line_once_and_never_paints_a_question_mark() {
    let mut trace = Trace::new();
    // `cd ..` leaves the repository: the line holds the branch until Git answers for the new folder.
    trace.commit(Some(OUTSIDE));
    trace.read(false, [None, None]);
    trace.read(false, [no_repository(), None]);
    // `cd .\repo` returns to a directory whose answer is known, so the commit alone is enough.
    trace.commit(None);
    trace.read(false, [None, None]);
    trace.read(true, [on_main(), on_main()]);
    assert_eq!(
        trace.moved,
        ["moved / main", "moved / C:/code", "moved / main"]
    );
    assert_eq!(trace.other, ["other / main"]);
}

/// The sequences a packaged build painted in two steps, each on a first visit: `plain / main`, a
/// non-Git folder with the last branch; `orchbee / ...\\work`, a repository with the last folder;
/// `fixture / main` with another repository's ahead count.
#[test]
fn a_first_visit_changes_the_name_and_the_line_together_to_their_final_value() {
    let mut trace = Trace::new();
    trace.visit("moved-plain", "C:/plain", no_repository());
    trace.visit("moved-orchbee", "C:/orchbee", main_ahead_of_upstream());
    trace.visit("moved-fixture", "C:/fixture", on_main());
    assert_eq!(
        trace.moved,
        [
            "moved / main",
            "moved-plain / C:/plain",
            "moved-orchbee ↑18 / main",
            "moved-fixture / main",
        ]
    );
    assert_eq!(trace.other, ["other / main"]);
}

#[test]
fn a_row_without_an_earlier_git_answer_also_waits_for_its_first_one() {
    let mut trace = Trace::after_first_read([None, on_main()]);
    trace.visit("moved-sub", "C:/repo/sub", on_main());
    assert_eq!(trace.moved, ["moved / C:/repo", "moved-sub / main"]);
}

#[test]
fn a_hung_git_read_releases_the_row_at_the_bound_and_a_late_answer_corrects_it() {
    let mut trace = Trace::new();
    trace.commit_named("moved-a", Some("C:/a"));
    // A further `cd` does not buy the row more time.
    assert!(!trace.tick(SPACE_HOLD_BOUND / 2));
    trace.commit_named("moved-b", Some("C:/b"));
    assert!(!trace.tick(SPACE_HOLD_BOUND / 2 - Duration::from_millis(1)));
    assert_eq!(trace.moved, ["moved / main"]);
    assert!(trace.tick(Duration::from_millis(1)), "the UI must repaint");
    assert_eq!(trace.moved, ["moved / main", "moved-b / C:/b"]);
    trace.read(false, [on_main(), None]);
    assert_eq!(
        trace.moved,
        ["moved / main", "moved-b / C:/b", "moved-b / main"]
    );
}

#[test]
fn a_rename_during_the_wait_shows_at_once() {
    let mut trace = Trace::new();
    trace.commit_named("moved-a", Some("C:/a"));
    trace.read(false, [None, None]);
    trace.commit_named("moved-renamed", Some("C:/a"));
    assert_eq!(trace.moved, ["moved / main", "moved-renamed / C:/a"]);
}

#[test]
fn a_view_that_shows_nothing_from_git_never_waits_for_it() {
    let mut trace = Trace::new();
    configure(
        &mut trace.view,
        serde_json::json!([[token("workspace")]]),
        0,
    );
    trace.commit_named("moved-a", Some("C:/a"));
    let row = trace.view.machines.space_rows()[0].clone();
    assert_eq!(trace.view.machines.space_held_row(&row), None);
}

#[test]
fn a_directory_without_a_coming_answer_ends_the_hold_on_the_real_folder() {
    // A complete Git round that got no answer for the new folder.
    let mut trace = Trace::new();
    trace.commit(Some(OUTSIDE));
    trace.read(true, [None, on_main()]);
    assert_eq!(trace.moved, ["moved / main", "moved / C:/code"]);
    assert_eq!(trace.other, ["other / main"]);

    // Reads that keep failing as a whole: the checkpoint is withdrawn as stale.
    let mut trace = Trace::new();
    trace.commit(Some(OUTSIDE));
    trace.read(false, [None, None]);
    trace
        .view
        .machines
        .observe_spaces(vec![(Choice::Local, Observation::Unavailable)]);
    trace.record();
    assert_eq!(trace.moved, ["moved / main", "moved / C:/code"]);
}

#[test]
fn a_failed_git_read_of_the_same_directory_keeps_the_branch_without_a_marker() {
    let mut trace = Trace::new();
    trace.read(true, [None, None]);
    assert_eq!(trace.moved, ["moved / main"]);
    assert_eq!(trace.other, ["other / main"]);
    let row = trace.view.machines.space_rows()[0].clone();
    assert!(
        trace.view.machines.space_checkpoint_stale(&row),
        "the Workspace picker still reports it"
    );
}
