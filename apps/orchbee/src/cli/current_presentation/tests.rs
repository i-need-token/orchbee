use super::*;
use crate::invocation::{QueryCommand, Target};
use orchbee_workbench_protocol::{
    ClientViewportObservation, ClientViewportUnavailable, WorkbenchRevision,
};
use serde_json::json;
fn id(n: u64) -> String {
    format!("{n:032x}")
}
fn panel() -> results::PanelDoc {
    let snapshot: Snapshot = serde_json::from_value(json!({"protocolVersion":1,"siteId":id(1),"revision":"7",
        "navigation":{"projects":[], "standaloneTerminalContextId":id(2), "directories":[], "workspaces":[]},
        "terminalContexts":[{"terminalContextId":id(2),"groups":[],"tabs":[{"tabId":id(3),"label":"tab","pinned":false,"groupId":null,
        "root":{"kind":"panel","panelId":id(4),"primaryPresentation":{"presentationId":id(5),"label":"primary","binding":null},
        "childPresentations":[{"presentationId":id(6),"label":"child","customLabel":"custom child","binding":{"terminalId":id(7),"workerId":id(8)}}]}}]}]})).unwrap();
    let results::Projection::PanelGet(panel) =
        results::project(&QueryCommand::PanelGet(Target::Id(id(4))), &snapshot).unwrap()
    else {
        panic!()
    };
    panel
}
fn read() -> Read {
    Read {
        member: ClientPresentationSelection::Bound {
            presentation_id: TerminalPresentationId::parse(&id(6)).unwrap(),
            binding: PresentationBinding {
                terminal_id: TerminalId::parse(&id(7)).unwrap(),
                worker_id: WorkerId::parse(&id(8)).unwrap(),
            },
            attachment_generation: WorkbenchRevision::new(9),
            incarnation: 1,
            viewport: ClientViewportObservation::Unavailable {
                reason: ClientViewportUnavailable::FrameUnavailable,
            },
        },
        operation_id: WorkbenchRequestId::parse(&id(10)).unwrap(),
    }
}
#[test]
fn all_current_origins_preserve_canonical_structure_and_exact_member() {
    let panel = panel();
    let primary = Resolved::new(&panel, Origin::Primary).unwrap();
    assert_eq!(primary.presentation_id.as_str(), id(5));
    assert!(primary.binding.is_none());
    let caller = Resolved::new(&panel, Origin::Caller(TerminalId::parse(&id(7)).unwrap())).unwrap();
    assert_eq!(caller.presentation_id.as_str(), id(6));
    assert_eq!(caller.value()["displayLabel"], "custom child");
    let client = Resolved::new(&panel, Origin::Client(Box::new(read()))).unwrap();
    assert_eq!(client.binding, caller.binding);
    assert_eq!(
        client.value()["source"],
        json!({"kind":"client","attachmentGeneration":"9","incarnation":1})
    );
    assert_eq!(panel.primary_presentation.presentation_id, id(5));
    assert!(panel.primary_presentation.terminal_id.is_none());
    assert_eq!(panel.child_presentations[0].presentation_id, id(6));
    assert!(Resolved::new(&panel, Origin::Caller(TerminalId::parse(&id(99)).unwrap())).is_err());
    let mut stale = read();
    let ClientPresentationSelection::Bound { binding, .. } = &mut stale.member else {
        unreachable!()
    };
    binding.worker_id = WorkerId::parse(&id(99)).unwrap();
    assert!(Resolved::new(&panel, Origin::Client(Box::new(stale))).is_err());
    let mut missing = read();
    let ClientPresentationSelection::Bound {
        presentation_id, ..
    } = &mut missing.member
    else {
        unreachable!()
    };
    *presentation_id = TerminalPresentationId::parse(&id(99)).unwrap();
    assert!(Resolved::new(&panel, Origin::Client(Box::new(missing))).is_err());
}

#[test]
fn client_empty_member_keeps_its_identity_without_attachment_facts() {
    let mut read = read();
    read.member = ClientPresentationSelection::Unbound {
        presentation_id: TerminalPresentationId::parse(&id(5)).unwrap(),
        selection_generation: WorkbenchRevision::new(9),
    };
    let selected = Resolved::new(&panel(), Origin::Client(Box::new(read.clone()))).unwrap();
    assert_eq!(selected.value()["binding"], serde_json::Value::Null);
    assert_eq!(
        selected.value()["source"],
        json!({"kind":"client-unbound","selectionGeneration":"9"})
    );
    assert!(selected.client().unwrap().member.viewport().is_none());
    read.member = ClientPresentationSelection::Unbound {
        presentation_id: TerminalPresentationId::parse(&id(6)).unwrap(),
        selection_generation: WorkbenchRevision::new(9),
    };
    assert!(Resolved::new(&panel(), Origin::Client(Box::new(read))).is_err());
}
