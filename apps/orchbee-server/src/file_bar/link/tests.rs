use super::{Event, Link, Route};
use crate::file_bar::{Outbound, mailbox::Mailbox};
use orchbee_capability_node_protocol::FilesProcessIdentity;
use orchbee_files_protocol::{
    CallId, FilesRecord, FrameKind, LinkFrame, LinkMessage, encode_files_record, write_link_message,
};
use orchbee_local_ipc::LocalStream;
use orchbee_workbench_protocol::{MultiplexStreamId, StreamCloseReason, StreamFrameKind};
use std::{
    collections::HashMap,
    num::NonZeroU64,
    sync::{Arc, mpsc},
    time::Duration,
};

/// A link served over one end of a pair, the other end standing for the process, and the
/// receiver of what the link reports.
fn serve() -> (Arc<Link>, LocalStream, mpsc::Receiver<Event>) {
    let (server, files) = LocalStream::pair().unwrap();
    let (sender, events) = mpsc::channel();
    let link = Link::serve(
        1,
        NonZeroU64::MIN,
        FilesProcessIdentity {
            process_id: 1,
            start: 1,
        },
        server,
        Arc::new(move |_, event| {
            let _ = sender.send(event);
        }),
    )
    .unwrap();
    (link, files, events)
}

fn call(id: u64) -> CallId {
    CallId::new(id).unwrap()
}

fn stream(id: u64) -> MultiplexStreamId {
    MultiplexStreamId::new(id).unwrap()
}

/// Routes call `id` to stream `id` of `mailbox`.
fn route(link: &Link, mailbox: &Arc<Mailbox>, id: u64, events: bool, mutation: bool) {
    let restarting: Arc<[u8]> = encode_files_record(&FilesRecord::Result(b"{}".to_vec()))
        .unwrap()
        .into();
    let route = Route::new(
        Arc::clone(mailbox),
        stream(id),
        events,
        mutation,
        restarting,
    );
    assert!(link.route(call(id), route));
}

fn frame(files: &mut LocalStream, id: u64, kind: FrameKind) {
    let record = match kind {
        FrameKind::Result => FilesRecord::Result(b"{}".to_vec()),
        FrameKind::Snapshot | FrameKind::Update => FilesRecord::Notice(b"{}".to_vec()),
    };
    let message = LinkMessage::Frame(LinkFrame {
        call: call(id),
        kind,
        record: encode_files_record(&record).unwrap(),
    });
    write_link_message(files, &message).unwrap();
}

fn ended(events: &mpsc::Receiver<Event>) -> bool {
    loop {
        match events.recv_timeout(Duration::from_secs(5)) {
            Ok(Event::Ended) => return true,
            Ok(_) => {}
            Err(_) => return false,
        }
    }
}

/// What each stream received, in order.
fn delivered(mailbox: &Mailbox) -> HashMap<u64, Vec<Outbound>> {
    let mut streams: HashMap<u64, Vec<Outbound>> = HashMap::new();
    for outbound in mailbox.take(usize::MAX, usize::MAX) {
        streams
            .entry(outbound.stream().value())
            .or_default()
            .push(outbound);
    }
    streams
}

fn close(id: u64, reason: StreamCloseReason) -> Outbound {
    Outbound::Close {
        stream: stream(id),
        reason,
    }
}

#[test]
fn a_lost_link_closes_each_open_stream_as_its_call_stood() {
    let (link, mut files, events) = serve();
    let mailbox = Arc::new(Mailbox::default());
    // An unanswered read, an unanswered mutation, an answered subscription and an answered read.
    route(&link, &mailbox, 1, false, false);
    route(&link, &mailbox, 2, false, true);
    route(&link, &mailbox, 3, true, false);
    route(&link, &mailbox, 4, false, false);
    frame(&mut files, 3, FrameKind::Snapshot);
    frame(&mut files, 4, FrameKind::Result);
    drop(files);
    assert!(ended(&events));
    let streams = delivered(&mailbox);
    let kinds = |id: u64| -> Vec<Option<StreamFrameKind>> {
        streams[&id]
            .iter()
            .map(|outbound| match outbound {
                Outbound::Frame { kind, .. } => Some(*kind),
                Outbound::Close { .. } => None,
            })
            .collect()
    };
    // The read ends with `unavailable{restarting}` as its answer.
    assert_eq!(kinds(1), [Some(StreamFrameKind::Result), None]);
    assert_eq!(streams[&1][1], close(1, StreamCloseReason::Complete));
    assert_eq!(streams[&2], [close(2, StreamCloseReason::OutcomeUnknown)]);
    assert_eq!(kinds(3), [Some(StreamFrameKind::Snapshot), None]);
    assert_eq!(streams[&3][1], close(3, StreamCloseReason::Stopping));
    assert_eq!(kinds(4), [Some(StreamFrameKind::Result), None]);
    assert_eq!(streams[&4][1], close(4, StreamCloseReason::Complete));
    // A call routed after the end is refused, so its owner answers it.
    let late = Route::new(
        Arc::clone(&mailbox),
        stream(5),
        false,
        false,
        Arc::from(Vec::new()),
    );
    assert!(!link.route(call(5), late));
}

#[test]
fn a_frame_for_a_call_never_sent_or_out_of_its_order_ends_the_link() {
    let (link, mut files, events) = serve();
    let mailbox = Arc::new(Mailbox::default());
    route(&link, &mailbox, 1, false, false);
    frame(&mut files, 2, FrameKind::Result);
    assert!(ended(&events));
    assert!(link.ended());

    let (link, mut files, events) = serve();
    route(&link, &mailbox, 1, true, false);
    // A subscription's first frame is its Snapshot.
    frame(&mut files, 1, FrameKind::Update);
    assert!(ended(&events));
    assert!(link.ended());
}
