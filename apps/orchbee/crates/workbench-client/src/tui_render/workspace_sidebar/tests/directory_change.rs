//! What a Workspace's location line paints while its first terminal changes directory, step by step
//! through the Client's own projection, retention and paint: a committed snapshot, the metadata read
//! the commit triggers, and the Git answer that follows.
use super::*;
use crate::machine_spaces::checkpoint::Observation;
use crate::machine_spaces::navigation_projection::SidebarNavigationProjection;
use crate::machine_spaces::{Checkpoint, Git};

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

/// A Client showing two Workspaces of one repository, of which only `moved` changes directory.
struct Trace {
    view: ClientView,
    snapshot: Snapshot,
    moved: Vec<String>,
    other: Vec<String>,
}
impl Trace {
    fn new() -> Self {
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
            moved: Vec::new(),
            other: Vec::new(),
        };
        trace.read(true, [on_main(), on_main()]);
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
        self.snapshot.revision = WorkbenchRevision::new(self.snapshot.revision.value() + 1);
        self.snapshot.navigation.workspaces[0].terminal_directory =
            directory.map(|path| WorkbenchDirectoryPath::parse(path).unwrap());
        let source = self.source();
        self.view
            .machines
            .observe_native_navigation(Choice::Local, Some(source));
        self.record();
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
        self.record();
    }
    /// Keeps each Workspace's painted location line, one entry per change.
    fn record(&mut self) {
        let (rows, _) = paint(&self.view, Locale::EnUs, &Theme::rgb_test_dark(), 120, 40);
        for (label, painted) in [("moved", &mut self.moved), ("other", &mut self.other)] {
            let name = rows
                .iter()
                .position(|row| row.contains(label))
                .unwrap_or_else(|| panic!("no row {label}: {rows:?}"));
            let line = rows[name + 1].split('│').next().unwrap().trim().to_owned();
            if painted.last() != Some(&line) {
                painted.push(line);
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
    assert_eq!(trace.moved, ["main", OUTSIDE, "main"]);
    assert_eq!(trace.other, ["main"]);
}

#[test]
fn a_directory_without_a_coming_answer_ends_the_hold_on_the_real_folder() {
    // A complete Git round that got no answer for the new folder.
    let mut trace = Trace::new();
    trace.commit(Some(OUTSIDE));
    trace.read(true, [None, on_main()]);
    assert_eq!(trace.moved, ["main", OUTSIDE]);
    assert_eq!(trace.other, ["main"]);

    // Reads that keep failing as a whole: the checkpoint is withdrawn as stale.
    let mut trace = Trace::new();
    trace.commit(Some(OUTSIDE));
    trace.read(false, [None, None]);
    trace
        .view
        .machines
        .observe_spaces(vec![(Choice::Local, Observation::Unavailable)]);
    trace.record();
    assert_eq!(trace.moved, ["main", OUTSIDE]);
}

#[test]
fn a_failed_git_read_of_the_same_directory_keeps_the_branch_without_a_marker() {
    let mut trace = Trace::new();
    trace.read(true, [None, None]);
    assert_eq!(trace.moved, ["main"]);
    assert_eq!(trace.other, ["main"]);
    let row = trace.view.machines.space_rows()[0].clone();
    assert!(
        trace.view.machines.space_checkpoint_stale(&row),
        "the Workspace picker still reports it"
    );
}
