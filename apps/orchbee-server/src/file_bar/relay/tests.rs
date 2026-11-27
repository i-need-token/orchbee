use super::{MAXIMUM_DATA_STREAMS, MAXIMUM_STREAMS, PRIORITY_INTERVAL, Relay};
use crate::file_bar::{FileBar, Host, Outbound};
use orchbee_files_protocol::{
    ConnectionId, ErrorCode, FilesRecord, FilesReply, GrantClass, Operation, Put, PutEnd,
    RecordPlace, TransferId, decode_files_record, encode_files_record, parse_reply,
};
use orchbee_workbench_protocol::{
    MultiplexLaneId, MultiplexStreamId, StreamCloseReason, StreamFrameKind, StreamMode,
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

const STAT: &str = r#"{"op":"fs.stat","workspaceId":"0123456789abcdef0123456789abcdef","paths":["/home/dev/project/src/main.rs"]}"#;
const STAGE: &str = r#"{"op":"git.stage","workspaceId":"0123456789abcdef0123456789abcdef","requestId":"fedcba9876543210fedcba9876543210","repository":"/home/dev/project","paths":["/home/dev/project/src/main.rs"]}"#;

/// A File Bar with no Worker, which these tests never reach: each stream they open is answered
/// at the relay.
fn file_bar() -> Arc<FileBar> {
    let host = Host {
        worker: Box::new(|| None),
        scopes: Box::new(|workspaces| vec![None; workspaces.len()]),
    };
    FileBar::start(std::env::temp_dir(), host, None).unwrap()
}

fn relay(bar: &Arc<FileBar>, grant: Option<GrantClass>) -> Relay {
    bar.relay(ConnectionId::new(1).unwrap(), grant)
}

fn stream(id: u64) -> MultiplexStreamId {
    MultiplexStreamId::new(id).unwrap()
}

fn request(json: &str) -> Vec<u8> {
    encode_files_record(&FilesRecord::Request(json.as_bytes().to_vec())).unwrap()
}

fn put() -> Vec<u8> {
    encode_files_record(&FilesRecord::Put(Put {
        transfer: TransferId::new(1).unwrap(),
        offset: 0,
        end: PutEnd::Continues,
        bytes: vec![0],
    }))
    .unwrap()
}

fn open(
    relay: &mut Relay,
    id: u64,
    mode: StreamMode,
    record: Vec<u8>,
) -> Result<(), StreamCloseReason> {
    relay.open(stream(id), MultiplexLaneId::ROOT, mode, record)
}

/// The error code of a Result frame's record.
fn error_code(outbound: &Outbound) -> Option<ErrorCode> {
    let Outbound::Frame {
        kind: StreamFrameKind::Result,
        record,
        ..
    } = outbound
    else {
        return None;
    };
    let Ok(FilesRecord::Result(json)) = decode_files_record(record, RecordPlace::ResultFrame)
    else {
        return None;
    };
    match parse_reply(Operation::FsStat, &json) {
        Ok(FilesReply::Error(error)) => Some(error.code),
        _ => None,
    }
}

#[test]
fn a_connection_holds_at_most_24_file_bar_streams_of_which_16_carry_data() {
    let bar = file_bar();
    let mut relay = relay(&bar, None);
    let data = u64::try_from(MAXIMUM_DATA_STREAMS).unwrap();
    let all = u64::try_from(MAXIMUM_STREAMS).unwrap();
    for id in 1..=data {
        assert_eq!(open(&mut relay, id, StreamMode::Request, put()), Ok(()));
    }
    assert_eq!(
        open(&mut relay, data + 1, StreamMode::Request, put()),
        Err(StreamCloseReason::Capacity)
    );
    for id in data + 2..=all + 1 {
        assert_eq!(
            open(&mut relay, id, StreamMode::Request, request(STAT)),
            Ok(())
        );
    }
    assert_eq!(
        open(&mut relay, all + 2, StreamMode::Request, request(STAT)),
        Err(StreamCloseReason::Capacity)
    );
    // A stream its Client closes gives its place back.
    assert!(relay.cancel(stream(1)));
    assert_eq!(
        open(&mut relay, all + 3, StreamMode::Request, put()),
        Ok(())
    );
    drop(relay);
    bar.stop_before(Instant::now() + Duration::from_secs(5));
}

#[test]
fn a_call_beyond_the_connection_grants_is_denied_and_a_record_the_relay_cannot_route_closes_its_stream()
 {
    let bar = file_bar();
    let mut reader = relay(&bar, None);
    assert_eq!(
        open(&mut reader, 1, StreamMode::Request, request(STAT)),
        Ok(())
    );
    let mut creator = relay(&bar, Some(GrantClass::TerminalCreate));
    assert_eq!(
        open(&mut creator, 1, StreamMode::Request, request(STAGE)),
        Ok(())
    );
    for relay in [&mut reader, &mut creator] {
        let answer = relay.take(Instant::now(), false);
        assert_eq!(answer.len(), 2);
        assert_eq!(error_code(&answer[0]), Some(ErrorCode::Denied));
        assert_eq!(
            answer[1],
            Outbound::Close {
                stream: stream(1),
                reason: StreamCloseReason::Complete
            }
        );
    }
    let unknown = r#"{"op":"fs.unknown","workspaceId":"0123456789abcdef0123456789abcdef"}"#;
    assert_eq!(
        open(&mut creator, 2, StreamMode::Request, request(unknown)),
        Err(StreamCloseReason::Unsupported)
    );
    assert_eq!(
        open(&mut creator, 3, StreamMode::Events, request(STAT)),
        Err(StreamCloseReason::InvalidRequest)
    );
    assert_eq!(
        open(&mut creator, 4, StreamMode::Request, request("{")),
        Err(StreamCloseReason::InvalidRequest)
    );
    drop((reader, creator));
    bar.stop_before(Instant::now() + Duration::from_secs(5));
}

#[test]
fn file_bar_records_yield_to_other_records_but_one_passes_every_100_milliseconds() {
    let bar = file_bar();
    let mut relay = relay(&bar, None);
    for id in 1..=3 {
        assert_eq!(
            open(&mut relay, id, StreamMode::Request, request(STAT)),
            Ok(())
        );
    }
    let start = Instant::now();
    // While other records wait, one File Bar record goes, with the close that follows it.
    let first = relay.take(start, true);
    assert_eq!(first.len(), 2);
    assert!(first.iter().all(|outbound| outbound.stream() == stream(1)));
    assert_eq!(relay.due(start), Some(PRIORITY_INTERVAL));
    let early = start + PRIORITY_INTERVAL / 2;
    assert!(relay.take(early, true).is_empty());
    assert_eq!(relay.due(early), Some(PRIORITY_INTERVAL / 2));
    let second = relay.take(start + PRIORITY_INTERVAL, true);
    assert!(second.iter().all(|outbound| outbound.stream() == stream(2)));
    // Once nothing else waits, every waiting record goes at once.
    let rest = relay.take(start + PRIORITY_INTERVAL, false);
    assert!(rest.iter().all(|outbound| outbound.stream() == stream(3)));
    assert_eq!(rest.len(), 2);
    assert_eq!(relay.due(start + PRIORITY_INTERVAL), None);
    drop(relay);
    bar.stop_before(Instant::now() + Duration::from_secs(5));
}
