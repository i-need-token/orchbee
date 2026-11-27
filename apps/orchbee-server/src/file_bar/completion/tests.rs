use super::settle;
use crate::file_bar::link::{Event, Link};
use orchbee_capability_node_protocol::FilesProcessIdentity;
use orchbee_files_protocol::{
    DrainReason, LinkMessage, LinkSide, WorkspaceId, WorkspaceScope, read_link_message,
};
use orchbee_local_ipc::LocalStream;
use orchbee_server::WorkbenchAuthority;
use orchbee_workbench_protocol::{SessionId, WorkbenchWorkspaceId};
use std::{num::NonZeroU64, sync::Arc};

fn workbench(n: u128) -> WorkbenchWorkspaceId {
    WorkbenchWorkspaceId::parse(&format!("{n:032x}")).unwrap()
}

fn files(n: u128) -> WorkspaceId {
    WorkspaceId::parse(&format!("{n:032x}")).unwrap()
}

#[test]
fn each_completion_reaches_a_serving_process_once_and_one_while_none_serves_never() {
    let session = SessionId::parse(&format!("{:032x}", 80)).unwrap();
    let journal = WorkbenchAuthority::new(session).unwrap().event_journal();
    journal.agent_completed(workbench(1));
    let position = settle(&journal, None, 0);
    let (server, mut process) = LocalStream::pair().unwrap();
    let link = Link::serve(
        1,
        NonZeroU64::MIN,
        FilesProcessIdentity {
            process_id: 1,
            start: 1,
        },
        server,
        Arc::new(|_: u64, _: Event| {}),
    )
    .unwrap();
    assert!(
        link.scopes()
            .before_call(files(1), Some(WorkspaceScope::NoRoot))
            .is_ok()
    );
    // The completion from before the process served stays behind.
    let position = settle(&journal, Some(&link), position);
    journal.agent_completed(workbench(1));
    // The link never heard of Workspace 2.
    journal.agent_completed(workbench(2));
    let position = settle(&journal, Some(&link), position);
    settle(&journal, Some(&link), position);
    link.send(&LinkMessage::Drain(DrainReason::Idle));
    let mut received = Vec::new();
    loop {
        let message = read_link_message(&mut process, LinkSide::Server).unwrap();
        if matches!(message, LinkMessage::Drain(_)) {
            break;
        }
        received.push(message);
    }
    assert_eq!(
        received,
        [LinkMessage::AgentSettled {
            workspace: files(1)
        }]
    );
}
