use super::super::{RunAnchor, RunTableEntry};
use super::{MAXIMUM_ENTRY_BYTES, read_in};
use orchbee_server::{AgentResumeProvider, RetainedAgentSession};
use std::{fs, path::PathBuf};

const SESSION: &str = "0f8fad5b-d9cb-469f-a165-70867728950e";
const PID: u32 = 11_052;
/// A Windows creation time: 100 ns units since 1601.
const CREATED: u64 = 134_388_423_375_879_211;
/// The same instant as .NET ticks, 100 ns units since year 1, which Claude Code's PowerShell
/// fallback writes instead.
const CREATED_AS_TICKS: u64 = CREATED + 504_911_232_000_000_000;

const PREFIX: &str = "orchbee-claude-run-table-";

/// A fixture run table directory under the system temporary directory, removed on drop.
struct Directory(PathBuf);

impl Directory {
    fn new() -> Self {
        let mut nonce = [0; 16];
        getrandom::fill(&mut nonce).unwrap();
        let directory =
            Self(std::env::temp_dir().join(format!("{PREFIX}{:032x}", u128::from_le_bytes(nonce))));
        fs::create_dir(&directory.0).unwrap();
        directory
    }

    fn write(&self, pid: u32, contents: impl AsRef<[u8]>) {
        fs::write(self.0.join(format!("{pid}.json")), contents).unwrap();
    }

    fn read(&self, pid: u32, created: u64) -> Vec<RunTableEntry> {
        read_in(&self.0, &[RunAnchor { pid, created }])
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        let path = self.0.canonicalize().unwrap();
        assert_eq!(
            path.parent(),
            Some(std::env::temp_dir().canonicalize().unwrap().as_path())
        );
        assert!(
            path.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with(PREFIX)
        );
        fs::remove_dir_all(path).unwrap();
    }
}

/// An entry with every field Claude Code 2.1.322 writes, holding synthetic values.
fn written(pid: serde_json::Value, session: &str, start: serde_json::Value) -> String {
    serde_json::json!({
        "pid": pid,
        "sessionId": session,
        "cwd": r"C:\work",
        "startedAt": 1_794_368_741_311_u64,
        "procStart": start,
        "version": "2.1.322",
        "peerProtocol": 1,
        "peerFeatures": [],
        "kind": "interactive",
        "entrypoint": "cli",
        "pidDomain": "win32:host",
        "asksForConvertedSkills": false,
        "messagingSocketPath": r"\\.\pipe\LOCAL\cc-msg-0",
        "name": "work",
        "nameSource": "derived",
        "nameSince": 1_794_368_741_311_u64,
        "updatedAt": 1_794_380_566_960_u64,
        "status": "busy",
        "statusUpdatedAt": 1_794_380_566_960_u64,
    })
    .to_string()
}

fn valid() -> String {
    written(PID.into(), SESSION, CREATED.to_string().into())
}

#[test]
fn an_entry_names_its_session_only_for_the_exact_process_it_was_read_for() {
    let directory = Directory::new();
    directory.write(PID, valid());
    let anchor = RunAnchor {
        pid: PID,
        created: CREATED,
    };
    assert_eq!(
        read_in(&directory.0, &[anchor]),
        [RunTableEntry {
            anchor,
            session: RetainedAgentSession::new(AgentResumeProvider::ClaudeCode, SESSION).unwrap(),
        }]
    );
    // The entry an earlier process left behind names nothing for a later one that reuses its pid,
    // nor for one created earlier.
    assert!(directory.read(PID, CREATED + 10_000_000).is_empty());
    assert!(directory.read(PID, CREATED - 1).is_empty());
    // A process without an entry has none, and only the matching anchor of several gets one.
    assert!(directory.read(PID + 4, CREATED).is_empty());
    let entries = read_in(
        &directory.0,
        &[
            RunAnchor {
                pid: PID + 4,
                created: CREATED,
            },
            anchor,
        ],
    );
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].anchor, anchor);
    // An entry filed under another process id names that process, not this one.
    directory.write(PID + 8, valid());
    assert!(directory.read(PID + 8, CREATED).is_empty());
    // A missing directory has no entries.
    assert!(
        read_in(&directory.0.join("absent"), &[anchor]).is_empty(),
        "a missing directory"
    );
}

#[test]
fn a_malformed_oversized_or_foreign_entry_names_nothing() {
    let directory = Directory::new();
    let entry = valid();
    let start = CREATED.to_string();
    let field = |name: &str, value: serde_json::Value| {
        let mut object: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(&entry).unwrap();
        object.insert(name.to_owned(), value);
        serde_json::Value::Object(object).to_string()
    };
    let without = |name: &str| {
        let mut object: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(&entry).unwrap();
        object.remove(name);
        serde_json::Value::Object(object).to_string()
    };
    let padding = "x".repeat(usize::try_from(MAXIMUM_ENTRY_BYTES).unwrap());
    let rows: [(&str, Vec<u8>); 18] = [
        ("an empty file", Vec::new()),
        ("a truncated entry", entry[..entry.len() / 2].into()),
        (
            "a byte order mark",
            [b"\xEF\xBB\xBF".as_slice(), entry.as_bytes()].concat(),
        ),
        ("text that is not JSON", b"pid=11052".to_vec()),
        (
            "the fields as an array",
            format!(r#"[{PID},"{SESSION}","{start}"]"#).into_bytes(),
        ),
        (
            "a numeric procStart",
            written(PID.into(), SESSION, CREATED.into()).into_bytes(),
        ),
        (
            "a procStart with a leading zero",
            written(PID.into(), SESSION, format!("0{start}").into()).into_bytes(),
        ),
        (
            "a signed procStart",
            written(PID.into(), SESSION, format!("+{start}").into()).into_bytes(),
        ),
        (
            "a procStart in .NET ticks",
            written(PID.into(), SESSION, CREATED_AS_TICKS.to_string().into()).into_bytes(),
        ),
        (
            "a pid as text",
            written(PID.to_string().into(), SESSION, start.clone().into()).into_bytes(),
        ),
        ("no sessionId", without("sessionId").into_bytes()),
        ("no procStart", without("procStart").into_bytes()),
        (
            "a sessionId that is not a Claude Code id",
            field("sessionId", "session-1".into()).into_bytes(),
        ),
        (
            "an option-like sessionId",
            field("sessionId", "-f8fad5b-d9cb-469f-a165-70867728950e".into()).into_bytes(),
        ),
        (
            "a repeated sessionId",
            format!(
                r#"{{"pid":{PID},"sessionId":"{SESSION}","sessionId":"7c9e6679-7425-40de-944b-e07fc1f90ae7","procStart":"{start}"}}"#
            )
            .into_bytes(),
        ),
        ("trailing text", format!("{entry}x").into_bytes()),
        (
            "an entry past the size bound",
            field("padding", padding.into()).into_bytes(),
        ),
        (
            "a sessionId of another type",
            field("sessionId", 7.into()).into_bytes(),
        ),
    ];
    // The well-formed entry these cases depart from is admitted.
    directory.write(PID, &entry);
    assert_eq!(directory.read(PID, CREATED).len(), 1);
    for (case, contents) in rows {
        directory.write(PID, contents);
        assert!(directory.read(PID, CREATED).is_empty(), "{case}");
    }
    // A directory where the entry belongs is not one.
    let pid = PID + 1;
    fs::create_dir(directory.0.join(format!("{pid}.json"))).unwrap();
    assert!(directory.read(pid, CREATED).is_empty(), "a directory");
}
