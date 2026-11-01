//! Quick Workspace defaults come from the first terminal, canonical directories or the launch directory.
use super::super::ClientExecutor;
use super::setup::{TestResult, admit, fixture_geometry, snapshot};
use super::source_root;
use crate::local_client::LocalClient;
use crate::local_client::test_support::server::FakeServer;
use crate::workbench_client::mailbox::CommandMailbox;
use crate::workbench_projection::{
    ClientContextLocation, ClientContextTabs, project_native_context_tabs,
    project_preferred_workspace,
};
use crate::workbench_state::OperationDeadlines;
use crate::workbench_state::workspace_setup::SetupToken;
use crate::workbench_state::workspace_setup::quick::QuickPreparation;
use orchbee_client_server_protocol::{
    ClientMessage, ServerMessage, TerminalViewerAction, TerminalViewerOutcome,
    TerminalViewerResult, ViewerUnavailableReason, read_client_message, write_server_message,
};
use orchbee_workbench_protocol::{
    Directory, TerminalContext, WorkbenchDirectoryId, WorkbenchDirectoryPath,
    WorkbenchSnapshotOutcome, WorkbenchTerminalContextId, WorkbenchWorkspaceId, WorkerId,
    Workspace,
};
use std::io::{self, Read};
use std::{sync::Arc, time::Instant};

fn quick(baseline: ClientContextTabs) -> QuickPreparation {
    let first_terminal = baseline.tabs()[0]
        .panel(0)
        .and_then(|panel| panel.primary().binding())
        .cloned();
    QuickPreparation {
        review_directory: false,
        target: None,
        generation: 1,
        token: SetupToken::for_test(1),
        baseline,
        first_terminal,
        first_terminal_pane: None,
        deadlines: OperationDeadlines::from_sample(Instant::now()).unwrap(),
    }
}

#[test]
fn quick_workspace_uses_the_first_terminal_directory() -> TestResult {
    source_root::borrow(|executor, source, outcome| {
        let mut preparation = quick(source.baseline.clone());
        preparation.first_terminal_pane.clone_from(&source.primary_pane);
        executor.prepare_quick_cwd(&preparation, outcome)
    })
}

#[test]
fn quick_workspace_falls_back_to_the_focused_workspace_directory_then_the_launch_directory()
-> TestResult {
    let initial_workspace = snapshot(1, 1);
    // Move the live terminal into a Workspace with a recorded directory; while its first
    // terminal cannot be sampled, the default is the recorded directory.
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
    for (baseline, snapshot, expected) in [
        (
            project_native_context_tabs(&workspace, &location).unwrap(),
            workspace.clone(),
            "C:/项目 space".to_owned(),
        ),
        (
            project_preferred_workspace(&initial_workspace).unwrap(),
            initial_workspace.clone(),
            launch.clone(),
        ),
    ] {
        let server = FakeServer::start(|mut stream| {
            admit(&mut stream)?;
            let ClientMessage::TerminalViewer(request) =
                read_client_message(&mut stream).map_err(io::Error::other)?
            else {
                return Err(io::Error::other("first terminal sample expected"));
            };
            if !matches!(request.action(), TerminalViewerAction::AttachBinding { .. }) {
                return Err(io::Error::other("temporary Viewer expected"));
            }
            write_server_message(
                &mut stream,
                &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                    request.request_id().clone(),
                    TerminalViewerOutcome::Unavailable {
                        reason: ViewerUnavailableReason::TerminalRuntimeUnavailable,
                    },
                )),
            )
            .map_err(io::Error::other)?;
            let mut trailing = [0];
            if stream.read(&mut trailing)? != 0 {
                return Err(io::Error::other("an unavailable sample is not retried"));
            }
            Ok(())
        })?;
        let client = LocalClient::connect(server.endpoint(), "native-quick-test")?;
        let mut executor = ClientExecutor::new(
            client,
            Arc::new(CommandMailbox::new(10, fixture_geometry()?)),
        );
        assert_eq!(
            executor.prepare_quick_cwd(
                &quick(baseline),
                &WorkbenchSnapshotOutcome::Available { snapshot }
            ),
            Ok(Some(WorkbenchDirectoryPath::parse(&expected)?))
        );
        drop(executor);
        server.finish()?;
    }
    Ok(())
}
