//! The engine's answers as records carry them: a listing page's JSON within a result record, and
//! a stat answer per path.

use std::{
    fs::{self, File},
    sync::mpsc,
    time::Duration,
};

use orchbee_files_protocol::{
    FilesReply, MAXIMUM_REPLY_BYTES, Operation, PageCursor, WorkspaceId, encode_reply, parse_reply,
};

use super::*;
use crate::fs::tests::Scratch;

const WAIT: Duration = Duration::from_secs(10);

fn list(engine: &Engine, path: &std::path::Path, cursor: Option<PageCursor>) -> FsListResult {
    let request = FsList {
        workspace_id: WorkspaceId::from_bytes([1; 16]).unwrap(),
        path: FilePath::new(path.to_str().unwrap()).unwrap(),
        cursor,
        single_child: false,
    };
    let (send, receive) = mpsc::channel();
    engine.list(
        ConnectionId::new(1).unwrap(),
        &request,
        path.to_path_buf(),
        Arc::new(AtomicBool::new(false)),
        move |result| send.send(result).unwrap(),
    );
    receive.recv_timeout(WAIT).unwrap().unwrap()
}

#[test]
fn a_listing_page_fits_a_result_record() {
    let scratch = Scratch::new("engine-page");
    // Long names fill the byte bound long before the entry bound.
    for index in 0..1_500 {
        File::create(
            scratch
                .path()
                .join(format!("{index:04}-{}", "n".repeat(180))),
        )
        .unwrap();
    }
    let engine = Engine::start().unwrap();
    let mut cursor = None;
    let mut listed = 0;
    let mut pages = 0;
    loop {
        let page = list(&engine, scratch.path(), cursor);
        let json = encode_reply(&FilesReply::FsList(page.clone()));
        assert!(json.len() <= MAXIMUM_REPLY_BYTES, "{} bytes", json.len());
        assert_eq!(
            parse_reply(Operation::FsList, &json).unwrap(),
            FilesReply::FsList(page.clone())
        );
        listed += page.entries.as_slice().len();
        pages += 1;
        cursor = page.cursor;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(listed, 1_500);
    assert!(pages > 1);
}

#[test]
fn stat_answers_each_path() {
    let scratch = Scratch::new("engine-stat");
    let file = scratch.path().join("file");
    fs::write(&file, b"four").unwrap();
    let missing = scratch.path().join("missing");
    let engine = Engine::start().unwrap();
    let wire = |path: &std::path::Path| FilePath::new(path.to_str().unwrap()).unwrap();
    let (send, receive) = mpsc::channel();
    engine.stat(
        vec![
            (wire(&file), file.clone()),
            (wire(&missing), missing.clone()),
        ],
        move |result| send.send(result).unwrap(),
    );
    let result = receive.recv_timeout(WAIT).unwrap().unwrap();
    let items = result.items.as_slice();
    assert_eq!(items[0].path, wire(&file));
    let entry = items[0].outcome.as_ref().unwrap();
    assert_eq!((entry.name.as_str(), entry.size.get()), ("file", 4));
    assert_eq!(
        items[1].outcome.as_ref().unwrap_err().code,
        ErrorCode::NotFound
    );
    assert!(
        parse_reply(
            Operation::FsStat,
            &encode_reply(&FilesReply::FsStat(result.clone()))
        )
        .is_ok()
    );
}
