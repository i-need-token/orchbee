//! Canonical topology, speculative order and independently delayed metadata never share a writer.
use super::*;
use crate::workbench_client::SidebarNavigationProjection;
use orchbee_workbench_protocol::{NavigationConfiguration, configure_navigation};

fn source(
    snapshot: Snapshot,
    pending: Option<NavigationConfiguration>,
) -> SidebarNavigationProjection {
    SidebarNavigationProjection {
        snapshot: Arc::new(snapshot),
        pending,
    }
}

fn move_to_root(snapshot: &Snapshot) -> NavigationConfiguration {
    NavigationConfiguration::MoveWorkspace {
        workspace_id: snapshot.navigation.workspaces[1].workspace_id.clone(),
        project_id: None,
        before_workspace_id: None,
    }
}

#[test]
fn pending_order_rolls_back_without_mutating_either_authority_and_a_newer_commit_wins() {
    let snapshot = fixture();
    let baseline = source(snapshot.clone(), None);
    let observed = Checkpoint::project_navigation(&baseline, None).unwrap();
    let pending = source(snapshot.clone(), Some(move_to_root(&snapshot)));
    let projected = Checkpoint::project_navigation(&pending, Some(&observed)).unwrap();
    let moved = &snapshot.navigation.workspaces[1];
    assert_eq!(
        projected.rows.last().unwrap().target.location,
        ClientContextLocation::Workspace {
            workspace_id: moved.workspace_id.clone(),
            terminal_context_id: moved.terminal_context_id.clone(),
        }
    );
    assert_eq!(projected.rows.last().unwrap().project_id, None);
    assert_eq!(
        projected.revision, snapshot.revision,
        "speculation cannot claim a commit revision"
    );
    assert_eq!(pending.snapshot.as_ref(), &snapshot);
    assert_eq!(
        Checkpoint::project_navigation(&baseline, Some(&observed)).unwrap(),
        observed,
        "definite refusal or unknown outcome removes only the local overlay"
    );
    for _ in 0..3 {
        assert_eq!(
            Checkpoint::project_navigation(&pending, Some(&observed)).unwrap(),
            projected
        );
    }

    let mut committed = snapshot.clone();
    committed.revision = WorkbenchRevision::new(snapshot.revision.value() + 1);
    committed.navigation =
        configure_navigation(&snapshot.navigation, pending.pending.as_ref().unwrap())
            .unwrap()
            .unwrap();
    let canonical =
        Checkpoint::project_navigation(&source(committed.clone(), None), Some(&observed)).unwrap();
    assert_eq!(canonical.rows, projected.rows);
    assert_eq!(
        Checkpoint::project_navigation(&pending, Some(&canonical)).unwrap(),
        canonical
    );
    let mut removed = canonical.clone();
    removed.revision = WorkbenchRevision::new(committed.revision.value() + 1);
    removed.rows = Vec::new().into();
    assert_eq!(
        Checkpoint::project_navigation(&pending, Some(&removed)).unwrap(),
        removed,
        "a later canonical deletion cannot resurrect through an older local snapshot"
    );
}

#[test]
fn metadata_is_retained_by_target_and_git_also_requires_the_same_directory_and_session() {
    let mut snapshot = fixture();
    let mut observed =
        Checkpoint::project_navigation(&source(snapshot.clone(), None), None).unwrap();
    let mut rows = observed.rows.to_vec();
    let workspace_row = rows.iter_mut().find(|row| row.directory.is_some()).unwrap();
    workspace_row.tokens = MetadataTokens::new(
        [(
            MetadataTokenKey::parse("owner").unwrap(),
            MetadataTokenValue::parse("same label").unwrap(),
        )]
        .into(),
    )
    .unwrap();
    workspace_row.git = Some(Git {
        branch: Some("main".into()),
        divergence: None,
    });
    let target = workspace_row.target.clone();
    let tokens = workspace_row.tokens.clone();
    observed.rows = rows.into();
    snapshot.revision = WorkbenchRevision::new(snapshot.revision.value() + 1);
    let projected =
        Checkpoint::project_navigation(&source(snapshot.clone(), None), Some(&observed)).unwrap();
    let row = projected
        .rows
        .iter()
        .find(|row| row.target == target)
        .unwrap();
    assert_eq!(row.tokens, tokens);
    assert_eq!(row.git.as_ref().unwrap().branch.as_deref(), Some("main"));
    assert!(row.git_stale);
    snapshot.navigation.directories[0].path = WorkbenchDirectoryPath::parse("C:/another").unwrap();
    let changed =
        Checkpoint::project_navigation(&source(snapshot.clone(), None), Some(&observed)).unwrap();
    let row = changed
        .rows
        .iter()
        .find(|row| row.target == target)
        .unwrap();
    assert_eq!(row.tokens, tokens);
    assert!(row.git.is_none());
    observed.session_id = SessionId::parse(&id(999)).unwrap();
    let replaced =
        Checkpoint::project_navigation(&source(snapshot, None), Some(&observed)).unwrap();
    assert!(
        replaced
            .rows
            .iter()
            .all(|row| row.tokens == MetadataTokens::default() && row.git.is_none())
    );
}
