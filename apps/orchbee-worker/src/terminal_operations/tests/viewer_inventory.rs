//! A viewer's Host inventory walk reads every page until the inventory ends, and a walk that cannot
//! finish within its budget refuses only its own request while the Worker serves the next one.
use super::super::*;
use super::shell::{WORKER, connected_host, record, reply, timeout};
use orchbee_capability_node_protocol::{self as node, *};
use orchbee_local_ipc::LocalStream;
use orchbee_terminal_protocol::{self as terminal, decode_transport_envelope};
use std::{net::Shutdown, thread};

const EPOCH: [u8; 16] = [4; 16];
const TARGET: [u8; 16] = [91; 16];
const TARGET_CREATION: [u8; 16] = [92; 16];
const TARGET_REVISION: u64 = 7;

fn target() -> HostTerminalIdentity {
    HostTerminalIdentity::new(
        HostCreationId::from_bytes(TARGET_CREATION).unwrap(),
        node::HostEpoch::from_bytes(EPOCH).unwrap(),
        HostTerminalId::from_bytes(TARGET).unwrap(),
    )
}

fn viewer(request: u8, action: TerminalViewerAction) -> CapabilityNodeMessage {
    CapabilityNodeMessage::TerminalViewerRequest(TerminalViewerRequest {
        request_id: OperationId::from_bytes([request; 16]).unwrap(),
        worker_id: node::WorkerId::from_bytes(WORKER.as_bytes()).unwrap(),
        worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
        attachment_id: OperationId::from_bytes([90; 16]).unwrap(),
        action,
    })
}

fn outcome(server: &mut LocalStream) -> TerminalViewerOutcome {
    let CapabilityNodeMessage::TerminalViewerResult(result) = node::read_message(server).unwrap()
    else {
        panic!("viewer result");
    };
    result.outcome
}

fn running(
    terminal_id: [u8; 16],
    creation_id: [u8; 16],
    revision: u64,
) -> terminal::TerminalSummary {
    terminal::TerminalSummary::Running {
        core: terminal::TerminalSummaryCore {
            configuration: terminal::TerminalConfiguration {
                scrollback_bytes: 0,
            },
            configuration_revision: 0,
            identity: terminal::TerminalIdentity {
                creation_id: terminal::CreationId::from_bytes(creation_id).unwrap(),
                host_epoch: terminal::HostEpoch::from_bytes(EPOCH).unwrap(),
                terminal_id: terminal::TerminalId::from_bytes(terminal_id).unwrap(),
            },
            revision,
            theme: terminal::TerminalThemeState {
                operation_id: None,
                revision: 0,
                terminal: terminal::TerminalTheme {
                    color_scheme: terminal::ColorScheme::Dark,
                    default_background_rgb: 0,
                    default_cursor_rgb: None,
                    default_foreground_rgb: 0xffffff,
                    palette: vec![0; 256],
                },
            },
        },
        geometry: terminal::TerminalGeometry {
            columns: 80,
            rows: 24,
        },
        terminate_requested: false,
    }
}

fn numbered(fill: u8, offset: u64) -> [u8; 16] {
    let mut bytes = [fill; 16];
    bytes[8..].copy_from_slice(&offset.to_be_bytes());
    bytes
}

/// Answers one `ListTerminals` request with a one-summary page of an inventory that holds
/// `length` summaries, the last being the target; `None` is an inventory that never ends.
fn answer_page(host: &mut LocalStream, request: &[u8], length: Option<u64>) {
    let envelope = decode_transport_envelope(request).unwrap();
    let terminal::InventoryControl::ListTerminals(list) =
        terminal::decode_inventory_control(envelope.kind, envelope.role, envelope.payload).unwrap()
    else {
        panic!("inventory page request");
    };
    let generation = NonZeroU64::MIN;
    let offset = list.cursor.map_or(0, |cursor| cursor.offset);
    let last = length.is_some_and(|length| offset + 1 == length);
    let summary = if last {
        running(TARGET, TARGET_CREATION, TARGET_REVISION)
    } else {
        running(numbered(0xa0, offset), numbered(0xb0, offset), 1)
    };
    let page = terminal::encode_inventory_control(&terminal::InventoryControl::TerminalList(
        terminal::TerminalList {
            inventory_generation: generation,
            next_cursor: (!last).then_some(terminal::TerminalInventoryCursor {
                inventory_generation: generation,
                offset: offset + 1,
            }),
            terminals: vec![summary],
        },
    ))
    .unwrap();
    reply(host, request, page.kind, &page.payload);
}

fn answer_pages(host: &mut LocalStream, length: u64) {
    for _ in 0..length {
        let request = record(host);
        answer_page(host, &request, Some(length));
    }
}

/// Sends the target's viewer Attach and answers its capability and projection-demand exchanges;
/// its inventory walk follows.
fn begin_attach(server: &mut LocalStream, host: &mut LocalStream) {
    node::write_message(
        server,
        &viewer(
            1,
            TerminalViewerAction::Attach {
                host_epoch: node::HostEpoch::from_bytes(EPOCH).unwrap(),
                terminal_id: HostTerminalId::from_bytes(TARGET).unwrap(),
            },
        ),
    )
    .unwrap();
    let request = record(host);
    let envelope = decode_transport_envelope(&request).unwrap();
    let terminal::AttachmentControl::AttachTerminal(attach) =
        terminal::decode_attachment_control(envelope.kind, envelope.role, envelope.payload)
            .unwrap()
    else {
        panic!("viewer attach");
    };
    let granted = terminal::encode_attachment_control(
        &terminal::AttachmentControl::AttachmentCapabilities(terminal::AttachmentCapabilities {
            attachment_id: attach.attachment_id,
            terminal_id: attach.terminal_id,
            capabilities: attach.capabilities,
            capability_revision: NonZeroU64::MIN,
            resize_lease: false,
        }),
    )
    .unwrap();
    reply(host, &request, granted.kind, &granted.payload);
    let request = record(host);
    let envelope = decode_transport_envelope(&request).unwrap();
    let terminal::ProjectionDemandControl::SetProjectionDemand(demand) =
        terminal::decode_projection_demand_control(envelope.kind, envelope.role, envelope.payload)
            .unwrap()
    else {
        panic!("viewer projection demand");
    };
    let accepted = terminal::encode_projection_demand_control(
        &terminal::ProjectionDemandControl::ProjectionDemandAccepted(
            terminal::ProjectionDemandAccepted {
                attachment_id: demand.attachment_id,
                demand: demand.demand,
            },
        ),
    )
    .unwrap();
    reply(host, &request, accepted.kind, &accepted.payload);
}

fn attach(server: &mut LocalStream, host: &mut LocalStream, length: u64) -> TerminalViewerOutcome {
    begin_attach(server, host);
    answer_pages(host, length);
    outcome(server)
}

/// Answers the Worker's Host detach of the target's viewer.
fn answer_detach(host: &mut LocalStream, request: &[u8]) -> bool {
    let envelope = decode_transport_envelope(request).unwrap();
    let Ok(terminal::AttachmentControl::DetachTerminal(detach)) =
        terminal::decode_attachment_control(envelope.kind, envelope.role, envelope.payload)
    else {
        return false;
    };
    let accepted = terminal::encode_attachment_control(
        &terminal::AttachmentControl::DetachTerminalAccepted(detach),
    )
    .unwrap();
    reply(host, request, accepted.kind, &accepted.payload);
    true
}

#[test]
fn a_viewer_inventory_walk_reads_every_page_until_the_inventory_ends() {
    // One summary per page puts the target far beyond any fixed page allowance.
    const LENGTH: u64 = 40;
    let (mut host, drain, mut host_peer) = connected_host();
    let (server, mut peer) = LocalStream::pair().unwrap();
    timeout(&peer);
    let driver = thread::spawn(move || {
        assert_eq!(
            attach(&mut peer, &mut host_peer, LENGTH),
            TerminalViewerOutcome::Attached
        );
        node::write_message(
            &mut peer,
            &viewer(2, TerminalViewerAction::ReadLifecycle { target: target() }),
        )
        .unwrap();
        answer_pages(&mut host_peer, LENGTH);
        assert_eq!(
            outcome(&mut peer),
            TerminalViewerOutcome::LifecycleObserved {
                target: target(),
                revision: TARGET_REVISION,
                settled: false,
            }
        );
        peer.shutdown(Shutdown::Both).unwrap();
    });
    let result = run(
        &server,
        &mut host,
        WORKER,
        NonZeroU64::MIN,
        &drain,
        &SystemShell::resolve(),
    );
    driver.join().unwrap();
    assert!(result.is_err());
}

#[test]
fn an_unfinished_inventory_walk_refuses_only_its_own_request() {
    let (mut host, drain, mut host_peer) = connected_host();
    let (server, mut peer) = LocalStream::pair().unwrap();
    timeout(&peer);
    let driver = thread::spawn(move || {
        begin_attach(&mut peer, &mut host_peer);
        // Every page arrives well within the Host exchange bound, but the inventory never ends,
        // so no walk budget covers it. A refused Attach grants nothing: its Host attachment goes
        // before the answer.
        loop {
            let request = record(&mut host_peer);
            if answer_detach(&mut host_peer, &request) {
                break;
            }
            thread::sleep(Duration::from_millis(250));
            answer_page(&mut host_peer, &request, None);
        }
        assert_eq!(
            outcome(&mut peer),
            TerminalViewerOutcome::Rejected(ViewerRejection::CapacityExceeded)
        );
        // The Worker serves the next requests, the same attachment identity attaches afresh, and
        // each Host exchange is answered in step.
        assert_eq!(
            attach(&mut peer, &mut host_peer, 1),
            TerminalViewerOutcome::Attached
        );
        node::write_message(&mut peer, &viewer(3, TerminalViewerAction::Detach)).unwrap();
        let request = record(&mut host_peer);
        assert!(answer_detach(&mut host_peer, &request));
        assert_eq!(outcome(&mut peer), TerminalViewerOutcome::Detached);
        peer.shutdown(Shutdown::Both).unwrap();
    });
    let result = run(
        &server,
        &mut host,
        WORKER,
        NonZeroU64::MIN,
        &drain,
        &SystemShell::resolve(),
    );
    driver.join().unwrap();
    assert!(result.is_err());
}
