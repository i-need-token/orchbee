use std::sync::mpsc;

use orchbee_files_protocol::{
    CallClose, CallId, ConnectionId, GrantClass, LaneId, LinkRequest, write_link_message,
};

use super::*;

fn start_serving() -> (LocalStream, thread::JoinHandle<End>) {
    let (files, server) = LocalStream::pair().unwrap();
    let serving = thread::spawn(move || {
        let (ends, ended) = mpsc::channel();
        serve(files, ends, &ended, None)
    });
    (server, serving)
}

fn call(value: u64) -> CallId {
    CallId::new(value).unwrap()
}

fn request(id: u64) -> LinkMessage {
    LinkMessage::Request(LinkRequest {
        call: call(id),
        connection: ConnectionId::new(1).unwrap(),
        lane: LaneId::new(1).unwrap(),
        workspace: None,
        grant: GrantClass::TerminalCreate,
        record: b"OBFB".to_vec(),
    })
}

#[test]
fn only_a_drain_ends_serving_cleanly() {
    let (mut server, serving) = start_serving();
    write_link_message(&mut server, &request(1)).unwrap();
    assert_eq!(
        read_link_message(&mut server, LinkSide::Files).unwrap(),
        LinkMessage::Close {
            call: call(1),
            reason: CallClose::InvalidRequest,
        }
    );
    // A cancel that crosses its call's close changes nothing.
    write_link_message(&mut server, &LinkMessage::Cancel { call: call(1) }).unwrap();
    write_link_message(&mut server, &LinkMessage::Drain(DrainReason::Session)).unwrap();
    assert_eq!(serving.join().unwrap(), End::Drained(DrainReason::Session));

    let (server, serving) = start_serving();
    drop(server);
    assert_eq!(serving.join().unwrap(), End::ServerGone);

    let (mut server, serving) = start_serving();
    // A kind the link does not have.
    server
        .write_all(b"OBFL\x00\x01\x00\x63\x00\x00\x00\x00")
        .unwrap();
    assert_eq!(
        serving.join().unwrap(),
        End::Failed(Stage::Read, io::ErrorKind::InvalidData)
    );
}
