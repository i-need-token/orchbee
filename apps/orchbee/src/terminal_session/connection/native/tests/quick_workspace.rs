//! Quick Workspace defaults come from canonical directories or the Client launch directory.
use super::super::NativeExecutor;
use super::setup::{TestResult, admit, fixture_geometry, snapshot};
use crate::local_client::LocalClient;
use crate::local_client::tests::server::FakeServer;
use crate::terminal_session::mailbox::CommandMailbox;
use crate::terminal_session::state::OperationDeadlines;
use crate::terminal_session::state::workspace_setup::{NativeSetupToken, quick::QuickPreparation};
use crate::terminal_session::workbench_projection::{
    NativeContextLocation, NativeContextTabs, project_native_context_tabs,
    project_native_standalone_tabs,
};
use orchbee_workbench_protocol::{
    Directory, TerminalContext, WorkbenchDirectoryId, WorkbenchDirectoryPath,
    WorkbenchSnapshotOutcome, WorkbenchTerminalContextId, WorkbenchWorkspaceId, WorkerId,
    Workspace,
};
use std::io::{self, Read};
use std::{sync::Arc, time::Instant};

fn quick(baseline: NativeContextTabs) -> QuickPreparation {
    QuickPreparation {
        review_directory: false,
        target: None,
        generation: 1,
        token: NativeSetupToken::for_test(1),
        baseline,
        deadlines: OperationDeadlines::from_sample(Instant::now()).unwrap(),
    }
}

#[test]
fn quick_workspace_uses_the_focused_workspace_directory_then_the_launch_directory_without_sampling()
-> TestResult {
    let standalone = snapshot(1, 1);
    // Move the live terminal into a Workspace whose recorded directory differs from any
    // terminal working directory; the default must still be the recorded directory.
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
    workspace
        .navigation
        .workspace_order
        .push(workspace_id.clone());
    workspace.terminal_contexts.push(TerminalContext {
        groups: Vec::new(),
        tabs: standalone.terminal_contexts[0].tabs.clone(),
        terminal_context_id: context_id.clone(),
    });
    let location = NativeContextLocation::Workspace {
        workspace_id,
        terminal_context_id: context_id,
    };
    let launch = orchbee_local_site_runtime::normalize_windows_user_path(
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
            project_native_standalone_tabs(&standalone).unwrap(),
            standalone.clone(),
            launch.clone(),
        ),
    ] {
        let server = FakeServer::start(|mut stream| {
            admit(&mut stream)?;
            let mut trailing = [0];
            if stream.read(&mut trailing)? != 0 {
                return Err(io::Error::other("quick defaults never sample a terminal"));
            }
            Ok(())
        })?;
        let client = LocalClient::connect(server.endpoint(), "native-quick-test")?;
        let mut executor = NativeExecutor::new(
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
