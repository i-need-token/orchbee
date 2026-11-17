//! A Server record has one bound, two seconds from its first readable byte. Readiness hints the
//! Worker writes while that record arrives spend the bound; they never restart it.

use orchbee_local_ipc::LocalStream as UnixStream;
use std::{
    fs,
    io::{self, Read, Write},
    thread,
    time::{Duration, Instant},
};

use orchbee_capability_node_protocol::{
    CapabilityNodeMessage, ReadMessageError, encode_message, read_message,
};
use orchbee_terminal_protocol::{
    CreationId, Envelope, HostEpoch, PayloadEncoding, TerminalEffectEvent, TerminalId,
    TerminalIdentity, TerminalMetadataChanged, encode_terminal_effect_event,
    encode_transport_envelope,
};

use super::super::server_transport;
use super::drain::{TestResult, nz, request, scripted_host_peer};

/// A Host title change, which leaves the Worker a title hint to write between requests.
fn title_record(epoch: HostEpoch) -> TestResult<Vec<u8>> {
    let event = encode_terminal_effect_event(&TerminalEffectEvent::MetadataChanged(
        TerminalMetadataChanged {
            identity: TerminalIdentity {
                creation_id: CreationId::from_bytes([5; 16])?,
                host_epoch: epoch,
                terminal_id: TerminalId::from_bytes([6; 16])?,
            },
            revision: nz(1),
            title: Some("title".into()),
            current_directory: None,
        },
    ))?;
    Ok(encode_transport_envelope(&Envelope {
        correlation_id: None,
        host_epoch: Some(epoch),
        kind: event.kind,
        optional_message: false,
        payload: &event.payload,
        payload_encoding: PayloadEncoding::ControlJson,
        protocol: orchbee_terminal_protocol::CURRENT_PROTOCOL_VERSION,
        role: event.role,
    })?)
}

/// Fills the Worker's outbound buffer, so the next record it writes waits for the Server to read,
/// and returns how many bytes it holds.
fn fill(stream: &UnixStream) -> io::Result<usize> {
    let mut writer = stream;
    let mut filled = 0;
    for chunk in [65_536, 4_096, 256, 1] {
        let bytes = vec![0_u8; chunk];
        loop {
            match writer.write(&bytes) {
                Ok(written) => filled += written,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) => return Err(error),
            }
        }
    }
    Ok(filled)
}

/// A request's first bytes arrive while the Worker writes a title hint the Server reads only
/// 1.3 s later; the rest never arrives. The request fails two seconds after its first byte, not
/// two seconds after the hint was written.
#[test]
fn a_server_record_has_one_bound_from_its_first_readable_byte_hint_time_included() -> TestResult {
    let (connection, _drain, mut host_peer, directory) = scripted_host_peer()?;
    connection.stream().set_nonblocking(true)?;
    host_peer.write_all(&title_record(connection.host_epoch())?)?;
    let (server, mut server_peer) = UnixStream::pair()?;
    server.set_nonblocking(true)?;
    server_peer.set_read_timeout(Some(Duration::from_secs(5)))?;
    let filled = fill(&server)?;
    let partial = encode_message(&CapabilityNodeMessage::AgentDrainRequest(request(1, 0)))?;
    let driver = thread::spawn(
        move || -> Result<(Instant, CapabilityNodeMessage, UnixStream), String> {
            let mut run = || -> TestResult<(Instant, CapabilityNodeMessage)> {
                thread::sleep(Duration::from_millis(200));
                server_peer.write_all(&partial[..3])?;
                let first_byte = Instant::now();
                thread::sleep(Duration::from_millis(1_300));
                let mut held = vec![0_u8; filled];
                server_peer.read_exact(&mut held)?;
                Ok((first_byte, read_message(&mut server_peer)?))
            };
            let (first_byte, hint) = run().map_err(|error| error.to_string())?;
            Ok((first_byte, hint, server_peer))
        },
    );
    let received = server_transport::receive_until_timed(&server, Some(&connection), None);
    let ended = Instant::now();
    let joined = driver.join().map_err(|_| "Server driver panicked")?;
    drop(host_peer);
    let _ = fs::remove_dir_all(&directory);
    let (first_byte, hint, _server_peer) = joined?;
    assert!(
        matches!(hint, CapabilityNodeMessage::TerminalTitlesChanged(_)),
        "{hint:?}"
    );
    let Err(ReadMessageError::Io(error)) = received else {
        return Err(format!("expected the record bound to pass: {received:?}").into());
    };
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    let waited = ended.duration_since(first_byte);
    assert!(
        waited >= Duration::from_millis(1_950) && waited < Duration::from_millis(2_800),
        "{waited:?}"
    );
    Ok(())
}
