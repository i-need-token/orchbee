//! Worktree shortcuts run Git where the Server says the focused Panel's terminal stands, resolved
//! as the Web and Desktop Clients resolve it.
use super::super::ClientExecutor;
use super::setup::{TestResult, admit, fixture_geometry, snapshot};
use crate::client_context::ClientContextLocation;
use crate::local_client::LocalClient;
use crate::local_client::test_support::server::FakeServer;
use crate::workbench_client::mailbox::CommandMailbox;
use crate::workbench_projection::{
    ClientContextTabs, ClientMemberTarget, project_native_context_tabs, project_preferred_workspace,
};
use crate::workbench_state::worktree_launch::{Origin, Preparation, WorktreeMode, WorktreeToken};
use crate::workbench_state::{CarrierFailure, OperationDeadlines};
use orchbee_client_server_protocol::Geometry;
use orchbee_workbench_protocol::{
    CurrentDirectoryFailure, CurrentDirectoryOutcome, CurrentDirectoryPanel,
    CurrentDirectoryResult, Directory, ProtocolVersion, Snapshot, TerminalContext,
    WorkbenchDirectoryId, WorkbenchDirectoryPath, WorkbenchRequest, WorkbenchResponse,
    WorkbenchRevision, WorkbenchSnapshotOutcome, WorkbenchTerminalContextId, WorkbenchWorkspaceId,
    WorkerId, Workspace, WorktreeSource, read_workbench_request, write_workbench_response,
};
use std::io::{self, Read};
use std::{sync::Arc, time::Instant};

/// The focused Workspace `C:/repo` beside the fixture's directory-less one.
fn directory_workspace() -> TestResult<(ClientContextTabs, Snapshot)> {
    let fixture = snapshot(1, 1);
    let mut workspace = snapshot(0, 1);
    let workspace_id = WorkbenchWorkspaceId::parse(&format!("{:032x}", 300))?;
    let context_id = WorkbenchTerminalContextId::parse(&format!("{:032x}", 301))?;
    let directory_id = WorkbenchDirectoryId::parse(&format!("{:032x}", 302))?;
    workspace.navigation.directories.push(Directory {
        directory_id: directory_id.clone(),
        worker_id: WorkerId::parse(&format!("{:032x}", 1_003))?,
        path: WorkbenchDirectoryPath::parse("C:/repo")?,
    });
    workspace.navigation.workspaces.push(Workspace {
        terminal_directory: None,
        automatic_label: false,
        label: "Workspace".into(),
        project_id: None,
        primary_directory_id: Some(directory_id),
        terminal_context_id: context_id.clone(),
        workspace_id: workspace_id.clone(),
    });
    workspace.navigation.workspace_order = workspace
        .navigation
        .workspaces
        .iter()
        .map(|workspace| workspace.workspace_id.clone())
        .collect();
    workspace.terminal_contexts.push(TerminalContext {
        groups: Vec::new(),
        tabs: fixture.terminal_contexts[0].tabs.clone(),
        terminal_context_id: context_id.clone(),
    });
    let location = ClientContextLocation::Workspace {
        workspace_id,
        terminal_context_id: context_id,
    };
    Ok((
        project_native_context_tabs(&workspace, &location).map_err(|_| "projection")?,
        workspace,
    ))
}

fn focused(cache: &ClientContextTabs) -> TestResult<WorkbenchWorkspaceId> {
    let ClientContextLocation::Workspace { workspace_id, .. } = cache.location() else {
        return Err("a focused Workspace is expected".into());
    };
    Ok(workspace_id.clone())
}

/// Prepares the source for `cache` without a focused Panel against a Server that answers its one
/// read with `outcome`.
fn prepare(
    cache: ClientContextTabs,
    snapshot: Snapshot,
    outcome: CurrentDirectoryOutcome,
) -> TestResult<Result<Option<WorktreeSource>, CarrierFailure>> {
    prepare_from(cache, snapshot, None, move |_| outcome.clone())
}

/// Prepares the source for `cache` focused on `member` against a Server that answers one read of
/// the focused Workspace by the Panel it names, and then expects nothing more: no Viewer and no
/// process sample.
fn prepare_from(
    cache: ClientContextTabs,
    snapshot: Snapshot,
    member: Option<ClientMemberTarget>,
    answer: impl Fn(Option<&CurrentDirectoryPanel>) -> CurrentDirectoryOutcome + Send + 'static,
) -> TestResult<Result<Option<WorktreeSource>, CarrierFailure>> {
    let workspace_id = focused(&cache)?;
    let session_id = cache.session_id().clone();
    let server = FakeServer::start(move |mut stream| {
        admit(&mut stream)?;
        let WorkbenchRequest::CurrentDirectory(request) =
            read_workbench_request(&mut stream).map_err(io::Error::other)?
        else {
            return Err(io::Error::other("one current-directory read expected"));
        };
        if request.session_id != session_id || request.workspace_id != workspace_id {
            return Err(io::Error::other("the focused Workspace is read"));
        }
        let outcome = answer(request.panel.as_ref());
        write_workbench_response(
            &mut stream,
            &WorkbenchResponse::CurrentDirectory(CurrentDirectoryResult {
                protocol_version: ProtocolVersion,
                request_id: request.request_id,
                session_id: request.session_id,
                workspace_id: request.workspace_id,
                panel: request.panel,
                outcome,
            }),
        )
        .map_err(io::Error::other)?;
        let mut trailing = [0];
        if stream.read(&mut trailing)? != 0 {
            return Err(io::Error::other("nothing follows the read"));
        }
        Ok(())
    })?;
    let client = LocalClient::connect(server.endpoint(), "native-worktree-source-test")?;
    let mut executor = ClientExecutor::new(
        client,
        Arc::new(CommandMailbox::new(10, fixture_geometry()?)),
    );
    let preparation = Preparation {
        generation: 1,
        token: WorktreeToken::for_test(1),
        mode: WorktreeMode::Create,
        origin: Origin {
            cache,
            member,
            zoom: false,
            geometry: Geometry::new(80, 24, 8, 16)?,
            primary_pane: None,
        },
        deadlines: OperationDeadlines::from_sample(Instant::now()).ok_or("deadlines")?,
    };
    let prepared = executor.prepare_worktree_source(
        &preparation,
        &WorkbenchSnapshotOutcome::Available { snapshot },
    );
    drop(executor);
    server.finish()?;
    Ok(prepared)
}

fn observed(path: &str) -> TestResult<CurrentDirectoryOutcome> {
    Ok(CurrentDirectoryOutcome::Observed {
        directory: WorkbenchDirectoryPath::parse(path)?,
    })
}

/// A person who changed directory in a Panel other than the Workspace's first terminal and
/// presses the shortcut there gets Git where that Panel's terminal stands.
#[test]
fn worktree_source_follows_the_focused_panel() -> TestResult {
    let fixture = snapshot(2, 1);
    let cache = project_preferred_workspace(&fixture).map_err(|_| "projection")?;
    let workspace_id = focused(&cache)?;
    let tab = cache.tabs().get(1).ok_or("a second Tab")?;
    let panel = tab.panel(0).ok_or("its Panel")?;
    let member = ClientMemberTarget {
        tab: tab.tab_id().clone(),
        panel: panel.panel_id().clone(),
        presentation: panel.primary().presentation_id().clone(),
    };
    let shown = CurrentDirectoryPanel {
        panel_id: member.panel.clone(),
        presentation_id: member.presentation.clone(),
    };
    let (there, first) = (observed("C:/second")?, observed("C:/first")?);
    assert_eq!(
        prepare_from(cache, fixture, Some(member), move |panel| {
            if panel == Some(&shown) {
                there.clone()
            } else {
                first.clone()
            }
        })?,
        Ok(Some(WorktreeSource::WorkspaceDirectory {
            workspace_id,
            path: WorkbenchDirectoryPath::parse("C:/second")?,
        }))
    );
    Ok(())
}

/// A terminal that left the Workspace's directory names the directory it stands in, so the
/// shortcut lists the same repository the GUI's Workspace menu lists.
#[test]
fn worktree_source_is_where_the_server_says_the_workspace_works() -> TestResult {
    let (cache, snapshot) = directory_workspace()?;
    let workspace_id = focused(&cache)?;
    assert_eq!(
        prepare(cache.clone(), snapshot.clone(), observed("C:/elsewhere")?)?,
        Ok(Some(WorktreeSource::WorkspaceDirectory {
            workspace_id: workspace_id.clone(),
            path: WorkbenchDirectoryPath::parse("C:/elsewhere")?,
        }))
    );
    // Another spelling of the Workspace's own directory stays the Workspace source.
    assert_eq!(
        prepare(cache, snapshot, observed("c:\\repo\\")?)?,
        Ok(Some(WorktreeSource::Workspace { workspace_id }))
    );
    Ok(())
}

/// A Workspace without a directory of its own still runs Git where its terminal stands, and a
/// commit elsewhere that only advances the revision leaves that source current.
#[test]
fn worktree_source_without_a_workspace_directory_follows_the_terminal() -> TestResult {
    let fixture = snapshot(1, 1);
    let cache = project_preferred_workspace(&fixture).map_err(|_| "projection")?;
    let workspace_id = focused(&cache)?;
    let mut later = fixture;
    later.revision = WorkbenchRevision::new(later.revision.value() + 1);
    assert_eq!(
        prepare(cache, later, observed("C:/项目 space")?)?,
        Ok(Some(WorktreeSource::WorkspaceDirectory {
            workspace_id,
            path: WorkbenchDirectoryPath::parse("C:/项目 space")?,
        }))
    );
    Ok(())
}

/// Without an answer only the Workspace's own directory is a source; the Client never samples a
/// terminal itself, as the GUI never does.
#[test]
fn worktree_source_without_an_answer_is_the_workspace_directory_or_nothing() -> TestResult {
    let unavailable = || CurrentDirectoryOutcome::Failed {
        reason: CurrentDirectoryFailure::Unavailable,
    };
    let (cache, snapshot) = directory_workspace()?;
    let workspace_id = focused(&cache)?;
    assert_eq!(
        prepare(cache, snapshot, unavailable())?,
        Ok(Some(WorktreeSource::Workspace { workspace_id }))
    );
    let fixture = super::setup::snapshot(1, 1);
    assert_eq!(
        prepare(
            project_preferred_workspace(&fixture).map_err(|_| "projection")?,
            fixture,
            unavailable(),
        )?,
        Ok(None)
    );
    Ok(())
}
