//! Quick Workspace defaults come from the Server's current-directory read, then the focused
//! Workspace's directory, then the launch directory.
use super::super::ClientExecutor;
use super::setup::{TestResult, admit, fixture_geometry, snapshot};
use crate::client_context::ClientContextLocation;
use crate::local_client::LocalClient;
use crate::local_client::test_support::server::FakeServer;
use crate::workbench_client::mailbox::CommandMailbox;
use crate::workbench_projection::{
    ClientContextTabs, project_native_context_tabs, project_preferred_workspace,
};
use crate::workbench_state::workspace_setup::SetupToken;
use crate::workbench_state::workspace_setup::quick::QuickPreparation;
use crate::workbench_state::{CarrierFailure, OperationDeadlines};
use orchbee_workbench_protocol::{
    CurrentDirectoryFailure, CurrentDirectoryOutcome, CurrentDirectoryResult, Directory,
    ProtocolVersion, Snapshot, TerminalContext, WorkbenchDirectoryId, WorkbenchDirectoryPath,
    WorkbenchRequest, WorkbenchResponse, WorkbenchSnapshotOutcome, WorkbenchTerminalContextId,
    WorkbenchWorkspaceId, WorkerId, Workspace, read_workbench_request, write_workbench_response,
};
use std::io::{self, Read};
use std::{sync::Arc, time::Instant};

fn quick(baseline: ClientContextTabs) -> QuickPreparation {
    QuickPreparation {
        review_directory: false,
        target: None,
        generation: 1,
        token: SetupToken::for_test(1),
        baseline,
        deadlines: OperationDeadlines::from_sample(Instant::now()).unwrap(),
    }
}

/// Prepares the default for `baseline` against a Server that answers one read of the focused
/// Workspace with `outcome` and then expects nothing more.
fn prepare(
    baseline: ClientContextTabs,
    snapshot: Snapshot,
    outcome: CurrentDirectoryOutcome,
) -> TestResult<Result<Option<WorkbenchDirectoryPath>, CarrierFailure>> {
    let ClientContextLocation::Workspace { workspace_id, .. } = baseline.location().clone() else {
        return Err("a focused Workspace is expected".into());
    };
    let session_id = baseline.session_id().clone();
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
        write_workbench_response(
            &mut stream,
            &WorkbenchResponse::CurrentDirectory(CurrentDirectoryResult {
                protocol_version: ProtocolVersion,
                request_id: request.request_id,
                session_id: request.session_id,
                workspace_id: request.workspace_id,
                outcome,
            }),
        )
        .map_err(io::Error::other)?;
        // The Server samples the terminal, so the Client attaches no Viewer of its own.
        let mut trailing = [0];
        if stream.read(&mut trailing)? != 0 {
            return Err(io::Error::other("nothing follows the read"));
        }
        Ok(())
    })?;
    let client = LocalClient::connect(server.endpoint(), "native-quick-test")?;
    let mut executor = ClientExecutor::new(
        client,
        Arc::new(CommandMailbox::new(10, fixture_geometry()?)),
    );
    let prepared = executor.prepare_quick_cwd(
        &quick(baseline),
        &WorkbenchSnapshotOutcome::Available { snapshot },
    );
    drop(executor);
    server.finish()?;
    Ok(prepared)
}

#[test]
fn quick_workspace_uses_the_directory_the_server_reads() -> TestResult {
    let snapshot = snapshot(1, 1);
    let directory = WorkbenchDirectoryPath::parse("C:/work")?;
    assert_eq!(
        prepare(
            project_preferred_workspace(&snapshot).unwrap(),
            snapshot,
            CurrentDirectoryOutcome::Observed {
                directory: directory.clone()
            },
        )?,
        Ok(Some(directory))
    );
    Ok(())
}

#[test]
fn quick_workspace_falls_back_to_the_focused_workspace_directory_then_the_launch_directory()
-> TestResult {
    let initial_workspace = snapshot(1, 1);
    // A Workspace with a recorded directory that closes before the read keeps that directory.
    let mut workspace = snapshot(0, 1);
    let workspace_id = WorkbenchWorkspaceId::parse(&format!("{:032x}", 300))?;
    let context_id = WorkbenchTerminalContextId::parse(&format!("{:032x}", 301))?;
    let directory_id = WorkbenchDirectoryId::parse(&format!("{:032x}", 302))?;
    workspace.navigation.directories.push(Directory {
        directory_id: directory_id.clone(),
        worker_id: WorkerId::parse(&format!("{:032x}", 1_003))?,
        path: WorkbenchDirectoryPath::parse("C:/项目 space")?,
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
        tabs: initial_workspace.terminal_contexts[0].tabs.clone(),
        terminal_context_id: context_id.clone(),
    });
    let location = ClientContextLocation::Workspace {
        workspace_id,
        terminal_context_id: context_id,
    };
    let launch = orchbee_local_runtime::normalize_user_path(
        ".",
        |name| std::env::var_os(name),
        std::env::current_dir,
    )
    .unwrap();
    for (baseline, snapshot, reason, expected) in [
        (
            project_native_context_tabs(&workspace, &location).unwrap(),
            workspace.clone(),
            CurrentDirectoryFailure::NotFound,
            "C:/项目 space".to_owned(),
        ),
        // Neither a sample nor a directory of its own leaves the launch directory.
        (
            project_preferred_workspace(&initial_workspace).unwrap(),
            initial_workspace.clone(),
            CurrentDirectoryFailure::Unavailable,
            launch.clone(),
        ),
    ] {
        assert_eq!(
            prepare(
                baseline,
                snapshot,
                CurrentDirectoryOutcome::Failed { reason }
            )?,
            Ok(Some(WorkbenchDirectoryPath::parse(&expected)?))
        );
    }
    Ok(())
}
