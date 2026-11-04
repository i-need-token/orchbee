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
    CurrentDirectoryFailure, CurrentDirectoryOutcome, CurrentDirectoryPanel,
    CurrentDirectoryResult, Directory, ProtocolVersion, Snapshot, TerminalContext,
    WorkbenchDirectoryId, WorkbenchDirectoryPath, WorkbenchRequest, WorkbenchResponse,
    WorkbenchSnapshotOutcome, WorkbenchTerminalContextId, WorkbenchWorkspaceId, WorkerId,
    Workspace, read_workbench_request, write_workbench_response,
};
use std::io::{self, Read};
use std::{sync::Arc, time::Instant};

fn quick(baseline: ClientContextTabs, panel: Option<CurrentDirectoryPanel>) -> QuickPreparation {
    QuickPreparation {
        review_directory: false,
        target: None,
        generation: 1,
        token: SetupToken::for_test(1),
        baseline,
        panel,
        deadlines: OperationDeadlines::from_sample(Instant::now()).unwrap(),
    }
}

/// Prepares the default for `baseline` without a focused Panel against a Server that answers its
/// one read with `outcome`.
fn prepare(
    baseline: ClientContextTabs,
    snapshot: Snapshot,
    outcome: CurrentDirectoryOutcome,
) -> TestResult<Result<Option<WorkbenchDirectoryPath>, CarrierFailure>> {
    prepare_from(baseline, snapshot, None, move |_| outcome.clone())
}

/// Prepares the default for `baseline` focused on `panel` against a Server that answers one read
/// of the focused Workspace by the Panel it names and then expects nothing more.
fn prepare_from(
    baseline: ClientContextTabs,
    snapshot: Snapshot,
    panel: Option<CurrentDirectoryPanel>,
    answer: impl Fn(Option<&CurrentDirectoryPanel>) -> CurrentDirectoryOutcome + Send + 'static,
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
        &quick(baseline, panel),
        &WorkbenchSnapshotOutcome::Available { snapshot },
    );
    drop(executor);
    server.finish()?;
    Ok(prepared)
}

/// A person who changed directory in a Panel other than the Workspace's first terminal and starts a
/// Quick Workspace there gets that Panel's directory.
#[test]
fn quick_workspace_follows_the_focused_panel() -> TestResult {
    let snapshot = snapshot(2, 1);
    let baseline = project_preferred_workspace(&snapshot).unwrap();
    let panel = baseline.tabs().get(1).and_then(|tab| tab.panel(0)).unwrap();
    let shown = CurrentDirectoryPanel {
        panel_id: panel.panel_id().clone(),
        presentation_id: panel.primary().presentation_id().clone(),
    };
    let there = WorkbenchDirectoryPath::parse("C:/second")?;
    let (second, first) = (
        CurrentDirectoryOutcome::Observed {
            directory: there.clone(),
        },
        CurrentDirectoryOutcome::Observed {
            directory: WorkbenchDirectoryPath::parse("C:/first")?,
        },
    );
    let expected = shown.clone();
    assert_eq!(
        prepare_from(baseline, snapshot, Some(shown), move |panel| {
            if panel == Some(&expected) {
                second.clone()
            } else {
                first.clone()
            }
        })?,
        Ok(Some(there))
    );
    Ok(())
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
