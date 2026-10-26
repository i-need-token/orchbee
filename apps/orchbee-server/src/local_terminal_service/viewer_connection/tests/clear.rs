use super::fixture::{Effect, Fixture, client_id};
use super::*;
use orchbee_client_server_protocol::{
    TerminalControlAction as Control, TerminalControlOutcome as ControlOutcome,
    TerminalControlRequest,
};

fn attach(connection: &mut LocalTerminalConnection<'_>, action: Action, id: u128) -> ViewerId {
    let ServerMessage::TerminalViewerResult(result) = connection.execute(
        ClientMessage::TerminalViewer(TerminalViewerRequest::new(client_id(id), action)),
    ) else {
        panic!("viewer result")
    };
    let Outcome::Attached { attachment_id, .. } = result.into_outcome() else {
        panic!("attached")
    };
    attachment_id
}
fn control(
    connection: &mut LocalTerminalConnection<'_>,
    action: Control,
    id: u128,
) -> ControlOutcome {
    let ServerMessage::TerminalControlResult(result) = connection.execute(
        ClientMessage::TerminalControl(TerminalControlRequest::new(client_id(id), action)),
    ) else {
        panic!("control result")
    };
    result.into_outcome()
}

#[test]
fn clear_server_keeps_active_controller_and_sequence_without_acquire_takeover_or_resize() {
    let fixture = Fixture::new();
    let mut active = fixture.service.client_connection();
    let mut cli = fixture.service.client_connection();
    let old = attach(&mut active, fixture.attach(0, true), 9010);
    let viewer = attach(&mut cli, fixture.attach(0, true), 9011);
    let ControlOutcome::Acquired { controller_id } = control(
        &mut active,
        Control::Acquire {
            attachment_id: old.clone(),
        },
        9012,
    ) else {
        panic!("acquired")
    };
    let before = fixture.service.controllers.lock().unwrap().len();
    assert_eq!(
        control(
            &mut cli,
            Control::Clear {
                attachment_id: viewer
            },
            9013
        ),
        ControlOutcome::Cleared {
            generation: NonZeroU64::new(2).unwrap(),
            revision: NonZeroU64::new(7).unwrap(),
            cleanup_confirmed: true
        }
    );
    assert_eq!(before, fixture.service.controllers.lock().unwrap().len());
    assert_eq!(
        control(
            &mut active,
            Control::Input {
                attachment_id: old,
                controller_id,
                sequence: NonZeroU64::MIN,
                bytes: orchbee_client_server_protocol::ControlInput::new(vec![b'x']).unwrap()
            },
            9014
        ),
        ControlOutcome::InputAccepted {}
    );
    let effects = fixture.barrier().unwrap();
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, Effect::Clear(..)))
            .count(),
        1
    );
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, Effect::Acquire(..)))
            .count(),
        1
    );
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::Release(..) | Effect::Resize(..)))
    );
}

#[test]
fn clear_server_refuses_foreign_viewer_changed_physical_binding_disposed_or_stale_worker() {
    for fault in [
        "foreign-session",
        "physical-binding",
        "disposed",
        "worker-replaced",
    ] {
        let fixture = Fixture::new();
        let mut cli = fixture.service.client_connection();
        let viewer = attach(&mut cli, fixture.attach(0, true), 9020);
        match fault {
            "physical-binding" => {
                let entry = cli.viewers.iter_mut().flatten().next().unwrap();
                entry.target.identity = orchbee_capability_node_protocol::HostTerminalIdentity::new(
                    entry.target.identity.creation_id(),
                    orchbee_capability_node_protocol::HostEpoch::from_bytes([6; 16]).unwrap(),
                    orchbee_capability_node_protocol::HostTerminalId::from_bytes([7; 16]).unwrap(),
                );
            }
            "disposed" => fixture.dispose(0),
            "worker-replaced" => fixture.replace_worker(),
            _ => {}
        }
        let outcome = if fault == "foreign-session" {
            control(
                &mut fixture.service.client_connection(),
                Control::Clear {
                    attachment_id: viewer,
                },
                9021,
            )
        } else {
            control(
                &mut cli,
                Control::Clear {
                    attachment_id: viewer,
                },
                9021,
            )
        };
        assert!(
            matches!(
                outcome,
                ControlOutcome::Rejected { .. } | ControlOutcome::Unavailable { .. }
            ),
            "{fault}: {outcome:?}"
        );
        assert!(
            !fixture
                .effects
                .lock()
                .unwrap()
                .iter()
                .any(|e| matches!(e, Effect::Clear(..)))
        );
    }
}
