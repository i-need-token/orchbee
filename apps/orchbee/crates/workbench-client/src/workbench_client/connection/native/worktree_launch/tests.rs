use super::*;
use crate::workbench_client::connection::native::tests::source_root::{self, SourceFixture};
use crate::workbench_state::worktree_launch::{Origin, WorktreeMode, WorktreeToken};
use orchbee_workbench_protocol::WorkbenchDirectoryPath;

fn prepare(
    executor: &mut ClientExecutor,
    sample: &SourceFixture,
    outcome: &WorkbenchSnapshotOutcome,
) -> Result<Option<WorkbenchDirectoryPath>> {
    let preparation = Preparation {
        generation: sample.generation,
        token: WorktreeToken::for_test(1),
        mode: WorktreeMode::Create,
        origin: Origin {
            cache: sample.baseline.clone(),
            member: None,
            zoom: false,
            geometry: orchbee_client_server_protocol::Geometry::new(80, 24, 8, 16).unwrap(),
            primary_pane: sample.primary_pane.clone(),
        },
        primary: sample.primary.clone(),
        primary_pane: sample.primary_pane.clone(),
        deadlines: sample.deadlines,
    };
    executor
        .prepare_worktree_source(&preparation, outcome)
        .map(|source| match source {
            Some(WorktreeSource::Directory { path }) => Some(path),
            None => None,
            _ => panic!("initial_workspace source must use the sampled primary root"),
        })
}
#[test]
fn worktree_primary_sample_releases_temporary_viewer_and_never_falls_back_to_home_or_child() {
    source_root::sample(prepare).unwrap();
}
#[test]
fn worktree_source_borrows_live_primary_and_retains_it_even_on_identity_failure() {
    source_root::borrow(prepare).unwrap();
}
/// O-2: a commit elsewhere in the Workbench, such as another Workspace's rename, lands between the
/// key and the preparation read and advances only the revision. The source is the focused
/// Workspace's own, so the preparation still samples it instead of reporting it unavailable.
#[test]
fn worktree_source_survives_a_commit_outside_its_scope() {
    source_root::sample(|executor, sample, outcome| {
        let WorkbenchSnapshotOutcome::Available { snapshot } = outcome else {
            panic!("the fixture reads an available snapshot");
        };
        let mut later = snapshot.clone();
        later.revision =
            orchbee_workbench_protocol::WorkbenchRevision::new(snapshot.revision.value() + 1);
        prepare(
            executor,
            sample,
            &WorkbenchSnapshotOutcome::Available { snapshot: later },
        )
    })
    .unwrap();
}
