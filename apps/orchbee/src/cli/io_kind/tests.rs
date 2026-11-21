use super::IoKind;
use serde_json::{Value, json};
use std::io;

/// The vocabulary, in the order every schema lists it.
const TOKENS: [&str; 11] = [
    "not-found",
    "permission-denied",
    "invalid-input",
    "invalid-data",
    "timed-out",
    "unsupported",
    "interrupted",
    "unexpected-eof",
    "would-block",
    "out-of-memory",
    "other",
];

/// Every `serverNotStartedDetails` definition in a schema, wherever it nests.
fn server_not_started_details<'a>(value: &'a Value, found: &mut Vec<&'a Value>) {
    match value {
        Value::Object(object) => {
            for (key, member) in object {
                if key == "serverNotStartedDetails" {
                    found.push(member);
                }
                server_not_started_details(member, found);
            }
        }
        Value::Array(items) => {
            for item in items {
                server_not_started_details(item, found);
            }
        }
        _ => {}
    }
}

#[test]
fn every_document_names_io_kinds_with_one_closed_vocabulary() {
    for (kind, token) in [
        (io::ErrorKind::NotFound, "not-found"),
        (io::ErrorKind::PermissionDenied, "permission-denied"),
        (io::ErrorKind::InvalidInput, "invalid-input"),
        (io::ErrorKind::InvalidData, "invalid-data"),
        (io::ErrorKind::TimedOut, "timed-out"),
        (io::ErrorKind::Unsupported, "unsupported"),
        (io::ErrorKind::Interrupted, "interrupted"),
        (io::ErrorKind::UnexpectedEof, "unexpected-eof"),
        (io::ErrorKind::WouldBlock, "would-block"),
        (io::ErrorKind::OutOfMemory, "out-of-memory"),
        (io::ErrorKind::ConnectionRefused, "other"),
        (io::ErrorKind::Other, "other"),
    ] {
        assert_eq!(IoKind::of(kind).token(), token);
    }

    let status: Value =
        serde_json::from_str(include_str!("../schemas/orchbee.status.client.json")).unwrap();
    assert_eq!(
        status["$defs"]["cohortDiagnostic"]["properties"]["ioKind"]["enum"],
        json!(TOKENS)
    );
    for (name, text) in [
        (
            "orchbee.cli.failure",
            include_str!("../schemas/orchbee.cli.failure.json"),
        ),
        (
            "orchbee.cli.failure.agent-start",
            include_str!("../schemas/orchbee.cli.failure.agent-start.json"),
        ),
        (
            "orchbee.cli.failure.tab-move",
            include_str!("../schemas/orchbee.cli.failure.tab-move.json"),
        ),
        (
            "orchbee.cli.failure.workspace-move",
            include_str!("../schemas/orchbee.cli.failure.workspace-move.json"),
        ),
        (
            "orchbee.workspace.creation",
            include_str!("../schemas/orchbee.workspace.creation.json"),
        ),
    ] {
        let schema: Value = serde_json::from_str(text).unwrap();
        let mut found = Vec::new();
        server_not_started_details(&schema, &mut found);
        assert!(
            !found.is_empty(),
            "{name} names no server-not-started details"
        );
        for details in found {
            assert_eq!(
                details["properties"]["ioKind"]["enum"],
                json!(TOKENS),
                "{name}"
            );
        }
    }
}
