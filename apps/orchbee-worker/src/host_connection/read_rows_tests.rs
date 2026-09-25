//! Row-read response budget witnesses: a scripted Host peer answers the exact
//! correlated ReadRows request with an otherwise valid kind-46 RowWindow
//! record whose OBRW payload is at, then one byte beyond, the effective budget.
use super::*;
use orchbee_capability_node_protocol::{
    HostTerminalId, OperationId, TerminalViewerAction, TerminalViewerOutcome,
    TerminalViewerRequest, ViewerRowDetail, WorkerGeneration, WorkerId,
};
use orchbee_terminal_protocol::{
    HostEpoch, ReadRows, RowAccessControl, RowWindowOrigin, decode_row_access_control,
};
use std::num::NonZeroU64;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::{self, JoinHandle};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;
type PeerResult = Result<(), Box<dyn Error + Send + Sync>>;

const ROWS_SNAPSHOT_KIND: u16 = 46;
const ROW_WINDOW_HEADER_BYTES: usize = 112;

fn host(row_window_bytes: u32) -> TestResult<(HostBrokerConnection, UnixStream)> {
    let (stream, peer) = UnixStream::pair()?;
    Ok((
        HostBrokerConnection {
            host_epoch: HostEpoch::from_bytes([4; 16])?,
            negotiated_limits: NegotiatedHostLimits {
                frame_bytes: 1_048_576,
                input_bytes: 4096,
                row_window_bytes,
            },
            stream,
            viewers: RefCell::new(viewers::HostViewers::default()),
            controllers: RefCell::new(controllers::HostControllers::default()),
            retired_targets: RefCell::new(target_cleanup::RetiredTargets::default()),
            drain: Rc::new(RefCell::new(crate::agent_drain::AgentDrain::new())),
        },
        peer,
    ))
}

fn attach() -> TestResult<TerminalViewerRequest> {
    Ok(TerminalViewerRequest {
        request_id: OperationId::from_bytes([20; 16])?,
        attachment_id: OperationId::from_bytes([1; 16])?,
        worker_id: WorkerId::from_bytes([3; 16])?,
        worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
        action: TerminalViewerAction::Attach {
            host_epoch: orchbee_capability_node_protocol::HostEpoch::from_bytes([4; 16])?,
            terminal_id: HostTerminalId::from_bytes([5; 16])?,
        },
    })
}

fn read(maximum_bytes: u32, detail: ViewerRowDetail) -> TestResult<TerminalViewerRequest> {
    Ok(TerminalViewerRequest {
        request_id: OperationId::from_bytes([21; 16])?,
        attachment_id: OperationId::from_bytes([1; 16])?,
        worker_id: WorkerId::from_bytes([3; 16])?,
        worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
        action: TerminalViewerAction::ReadRows {
            detail,
            start_row: 2,
            row_count: 4,
            revision: 9,
            generation: NonZeroU64::MIN,
            request_sequence: NonZeroU64::MIN,
            maximum_bytes,
        },
    })
}

// A self-consistent OBRW text-detail window of exactly `bytes` length that
// echoes every routing field of the forwarded read request, so the payload
// byte length is the only property under test.
fn row_window_payload(read: &ReadRows, bytes: usize) -> Vec<u8> {
    let mut payload = vec![0_u8; bytes];
    payload[..4].copy_from_slice(b"OBRW");
    payload[6] = 2;
    payload[8..12].copy_from_slice(&(bytes as u32).to_le_bytes());
    payload[12..14].copy_from_slice(&(ROW_WINDOW_HEADER_BYTES as u16).to_le_bytes());
    payload[14..16].copy_from_slice(&80_u16.to_le_bytes());
    payload[16..32].copy_from_slice(&[4; 16]);
    payload[32..48].copy_from_slice(read.attachment_id.as_bytes());
    payload[48..56].copy_from_slice(&read.request_sequence.get().to_le_bytes());
    payload[56..64].copy_from_slice(&read.revision.to_le_bytes());
    payload[64..72].copy_from_slice(&read.generation.get().to_le_bytes());
    let start_row = match read.origin {
        RowWindowOrigin::Absolute { row } => row,
        other => panic!("row read used a non-absolute origin {other:?}"),
    };
    payload[72..80].copy_from_slice(&(start_row + u64::from(read.row_count)).to_le_bytes());
    payload[80..88].copy_from_slice(&start_row.to_le_bytes());
    payload[88..90].copy_from_slice(&read.row_count.to_le_bytes());
    payload[96..100].copy_from_slice(&((bytes - ROW_WINDOW_HEADER_BYTES) as u32).to_le_bytes());
    payload[100] = 3;
    payload
}

// The scripted Host reads exactly one request record, reports the row budget
// the Worker forwarded, then answers with a kind-46 RowWindow record whose
// payload is exactly `payload_bytes`.
fn host_peer(
    mut peer: UnixStream,
    payload_bytes: usize,
    forwarded: Sender<(u32, orchbee_terminal_protocol::RowWindowDetail)>,
) -> JoinHandle<PeerResult> {
    thread::spawn(move || {
        let mut header = [0_u8; TRANSPORT_ENVELOPE_HEADER_BYTES];
        peer.read_exact(&mut header)?;
        let length = usize::try_from(u32::from_le_bytes(header[8..12].try_into()?))?;
        let mut record = vec![0_u8; length];
        record[..TRANSPORT_ENVELOPE_HEADER_BYTES].copy_from_slice(&header);
        peer.read_exact(&mut record[TRANSPORT_ENVELOPE_HEADER_BYTES..])?;
        let envelope = decode_transport_envelope(&record)?;
        let RowAccessControl::ReadRows(read) =
            decode_row_access_control(envelope.kind, envelope.role, envelope.payload)?
        else {
            panic!("Host received a non-ReadRows request")
        };
        forwarded.send((read.maximum_bytes, read.detail))?;
        let payload = row_window_payload(&read, payload_bytes);
        let record = encode_transport_envelope(&Envelope {
            correlation_id: envelope.correlation_id,
            host_epoch: envelope.host_epoch,
            kind: ROWS_SNAPSHOT_KIND,
            optional_message: false,
            payload: &payload,
            payload_encoding: PayloadEncoding::RowWindow,
            protocol: CURRENT_PROTOCOL_VERSION,
            role: MessageRole::Response,
        })?;
        peer.write_all(&record)?;
        Ok(())
    })
}

type RowExchangeOutcome = (
    Result<TerminalViewerOutcome, Box<dyn Error>>,
    JoinHandle<PeerResult>,
    Receiver<(u32, orchbee_terminal_protocol::RowWindowDetail)>,
);

fn row_exchange(
    detail: ViewerRowDetail,
    request_maximum: u32,
    negotiated_maximum: u32,
    payload_bytes: usize,
) -> TestResult<RowExchangeOutcome> {
    let (host, peer) = host(negotiated_maximum)?;
    host.viewers
        .borrow_mut()
        .admit(&attach()?)
        .map_err(|reason| format!("{reason:?}"))?;
    let (server, _quiet) = UnixStream::pair()?;
    let (forwarded, observed) = channel();
    let peer_thread = host_peer(peer, payload_bytes, forwarded);
    let result = crate::terminal_viewer_operation::execute(
        read(request_maximum, detail)?,
        orchbee_local_site_protocol::WorkerId::from_bytes([3; 16]),
        NonZeroU64::MIN,
        &host,
        &server,
    )
    .map(|result| result.outcome);
    Ok((result, peer_thread, observed))
}

#[test]
fn row_read_response_at_the_effective_budget_is_delivered() -> TestResult {
    let (result, peer_thread, observed) =
        row_exchange(ViewerRowDetail::Text, 8 * 1024 * 1024, 4096, 4096)?;
    let TerminalViewerOutcome::Record(record) = result? else {
        panic!("an exact-budget row window must be delivered")
    };
    let envelope = decode_transport_envelope(&record)?;
    assert_eq!(envelope.kind, ROWS_SNAPSHOT_KIND);
    assert_eq!(envelope.payload_encoding, PayloadEncoding::RowWindow);
    assert_eq!(envelope.payload.len(), 4096);
    peer_thread
        .join()
        .unwrap()
        .map_err(|error| -> Box<dyn Error> { error })?;
    assert_eq!(
        observed.recv()?,
        (4096, orchbee_terminal_protocol::RowWindowDetail::Text)
    );
    Ok(())
}

#[test]
fn row_read_request_budget_below_negotiation_is_the_effective_maximum() -> TestResult {
    let (result, peer_thread, observed) =
        row_exchange(ViewerRowDetail::Text, 2048, 65_536, 2048 + 1)?;
    let error = result.expect_err("a row window past its budget must be rejected");
    assert!(
        error.to_string().contains("byte budget"),
        "rejection must name the byte budget, got {error}"
    );
    peer_thread
        .join()
        .unwrap()
        .map_err(|error| -> Box<dyn Error> { error })?;
    assert_eq!(
        observed.recv()?,
        (2048, orchbee_terminal_protocol::RowWindowDetail::Text)
    );
    Ok(())
}

#[test]
fn row_read_response_one_byte_over_the_effective_budget_is_rejected() -> TestResult {
    let (result, peer_thread, observed) =
        row_exchange(ViewerRowDetail::Text, 8 * 1024 * 1024, 4096, 4096 + 1)?;
    let error = result.expect_err("a row window past its budget must be rejected");
    assert!(
        error.to_string().contains("byte budget"),
        "rejection must name the byte budget, got {error}"
    );
    peer_thread
        .join()
        .unwrap()
        .map_err(|error| -> Box<dyn Error> { error })?;
    // The forwarded request proves the effective budget is the negotiated
    // limit, the smallest of request, negotiation, and carrier capacity.
    assert_eq!(
        observed.recv()?,
        (4096, orchbee_terminal_protocol::RowWindowDetail::Text)
    );
    Ok(())
}

#[test]
fn render_row_read_reaches_host_with_the_same_negotiated_budget() -> TestResult {
    let (result, peer, observed) =
        row_exchange(ViewerRowDetail::Render, 8 * 1024 * 1024, 4096, 4096)?;
    assert!(matches!(result?, TerminalViewerOutcome::Record(_)));
    peer.join()
        .unwrap()
        .map_err(|error| -> Box<dyn Error> { error })?;
    assert_eq!(
        observed.recv()?,
        (4096, orchbee_terminal_protocol::RowWindowDetail::Render)
    );
    Ok(())
}
