//! The File Bar corpora against the Rust binding and `schemas/files/v1`: both admit exactly the
//! valid documents, except cases marked as binding boundaries, which the schema's character
//! counts admit and the binding's UTF-8 byte bounds refuse. Each corpus holds what its capability
//! adds, and every valid request in it needs exactly that capability. The TypeScript binding reads
//! the same corpora.

use std::collections::HashMap;

use orchbee_files_protocol::{
    Chunk, ChunkEnd, FilesRecord, GrantClass, LineEnding, MAXIMUM_DATA_BYTES, MAXIMUM_PATH_BYTES,
    MAXIMUM_REPLY_BYTES, MAXIMUM_REQUEST_BYTES, NoticeKind, Operation, OperationStream, Put,
    PutEnd, RecordPlace, RequestError, TransferId, check_files_record, decode_files_record,
    encode_files_record, encode_notice, encode_reply, encode_request, parse_notice, parse_reply,
    parse_request,
};
use serde_json::Value;

use crate::support::{Corpus, Document, bytes, corpora};

const SCHEMA: &str = include_str!("../../../schemas/files/v1/files.schema.json");

/// Compiles one definition of the schema at a time, with the `orchbee-uint64` format asserted
/// as the TypeScript validators assert it.
struct Schema {
    document: Value,
    validators: HashMap<String, jsonschema::Validator>,
}

impl Schema {
    fn new() -> Self {
        let document: Value = serde_json::from_str(SCHEMA).unwrap();
        assert!(jsonschema::draft202012::meta::is_valid(&document));
        Self {
            document,
            validators: HashMap::new(),
        }
    }

    fn admits(&mut self, definition: &str, instance: &Value) -> bool {
        if !self.validators.contains_key(definition) {
            assert!(
                self.document["$defs"].get(definition).is_some(),
                "{definition} is not a schema definition"
            );
            let mut document = self.document.clone();
            document["$ref"] = Value::String(format!("#/$defs/{definition}"));
            let validator = jsonschema::draft202012::options()
                .with_format("orchbee-uint64", |text: &str| {
                    text.parse::<u64>().is_ok() && (text == "0" || !text.starts_with('0'))
                })
                .should_validate_formats(true)
                .build(&document)
                .unwrap();
            self.validators.insert(definition.to_owned(), validator);
        }
        self.validators[definition].is_valid(instance)
    }
}

fn place(name: &str) -> RecordPlace {
    match name {
        "request-open" => RecordPlace::RequestOpen,
        "events-open" => RecordPlace::EventsOpen,
        "result-frame" => RecordPlace::ResultFrame,
        "notice-frame" => RecordPlace::NoticeFrame,
        other => panic!("unknown record place {other}"),
    }
}

fn number(value: &Value) -> u64 {
    value.as_str().unwrap().parse().unwrap()
}

fn expected_record(value: &Value) -> FilesRecord {
    let json = || value["json"].as_str().unwrap().as_bytes().to_vec();
    let transfer = || TransferId::new(number(&value["transfer"])).unwrap();
    let data = || bytes(value["data"].as_str().unwrap());
    match value["kind"].as_str().unwrap() {
        "request" => FilesRecord::Request(json()),
        "result" => FilesRecord::Result(json()),
        "notice" => FilesRecord::Notice(json()),
        "put" => FilesRecord::Put(Put {
            transfer: transfer(),
            offset: number(&value["offset"]),
            end: match value["end"].as_str().unwrap() {
                "continues" => PutEnd::Continues,
                "last" => PutEnd::Last,
                "commit" => PutEnd::Commit,
                other => panic!("unknown put end {other}"),
            },
            bytes: data(),
        }),
        "chunk" => FilesRecord::Chunk(Chunk {
            transfer: transfer(),
            offset: number(&value["offset"]),
            end: (!value["end"].is_null()).then(|| {
                let end = &value["end"];
                ChunkEnd {
                    line_ending: match end["lineEnding"].as_str().unwrap() {
                        "none" => LineEnding::None,
                        "lf" => LineEnding::Lf,
                        "crlf" => LineEnding::CrLf,
                        other => panic!("unknown line ending {other}"),
                    },
                    changed: end["changed"].as_bool().unwrap(),
                    digest: end["digest"]
                        .as_str()
                        .map(|digest| bytes(digest).try_into().unwrap()),
                }
            }),
            bytes: data(),
        }),
        other => panic!("unknown record kind {other}"),
    }
}

fn operation(document: &Document) -> Operation {
    let name = document.op.as_deref().unwrap();
    Operation::from_name(name).unwrap_or_else(|| panic!("{}: unknown op {name}", document.name))
}

/// The operations of every corpus, which a reply case may name whatever corpus holds it.
fn all_operations(corpora: &[Corpus]) -> impl Iterator<Item = &crate::support::OperationFacts> {
    corpora.iter().flat_map(|corpus| &corpus.operations)
}

#[test]
fn operations_and_bounds_match_the_binding() {
    let corpora = corpora();
    let names = all_operations(&corpora)
        .map(|facts| facts.op.as_str())
        .collect::<Vec<_>>();
    let bound = Operation::ALL
        .iter()
        .map(|operation| operation.name())
        .collect::<Vec<_>>();
    assert_eq!(names, bound);
    let mut schema = Schema::new();
    for corpus in &corpora {
        for facts in &corpus.operations {
            let operation = Operation::from_name(&facts.op).unwrap();
            assert_eq!(
                operation.capability().name(),
                corpus.capability,
                "{}",
                facts.op
            );
        }
    }
    for facts in all_operations(&corpora) {
        let operation = Operation::from_name(&facts.op).unwrap();
        let stream = match operation.stream() {
            OperationStream::Request => "request",
            OperationStream::Events => "events",
        };
        let grant = match operation.grant() {
            GrantClass::TerminalCreate => "terminal-create",
            GrantClass::Lifecycle => "lifecycle",
        };
        assert_eq!(
            (stream, grant, operation.after_close()),
            (
                facts.stream.as_str(),
                facts.grant.as_str(),
                facts.after_close
            ),
            "{}",
            facts.op
        );
        // A request stream answers with a Result or, for a chunk request, a Chunk; an events
        // stream answers with notices and a Result only to fail.
        let events = operation.stream() == OperationStream::Events;
        assert_eq!(facts.snapshot.is_some(), events, "{}", facts.op);
        assert_eq!(facts.update.is_some(), events, "{}", facts.op);
        assert_eq!(
            facts.result.is_none(),
            events || facts.chunk,
            "{}",
            facts.op
        );
        for definition in [&facts.result, &facts.snapshot, &facts.update]
            .into_iter()
            .flatten()
        {
            schema.admits(definition, &Value::Null);
        }
    }
    for corpus in &corpora {
        let bounds = &corpus.bounds;
        assert_eq!(bounds.request_bytes, MAXIMUM_REQUEST_BYTES);
        assert_eq!(bounds.reply_bytes, MAXIMUM_REPLY_BYTES);
        assert_eq!(bounds.data_bytes, MAXIMUM_DATA_BYTES);
        assert_eq!(bounds.path_bytes, MAXIMUM_PATH_BYTES);
    }
}

#[test]
fn records_decode_and_encode_as_the_corpus_states() {
    let corpora = corpora();
    let valid = corpora
        .iter()
        .flat_map(|corpus| &corpus.records.valid)
        .collect::<Vec<_>>();
    let invalid = corpora
        .iter()
        .flat_map(|corpus| &corpus.records.invalid)
        .collect::<Vec<_>>();
    assert!(!valid.is_empty() && !invalid.is_empty());
    for case in valid {
        let record = bytes(&case.hex);
        let expected = expected_record(&case.record);
        let place = place(&case.place);
        assert_eq!(
            decode_files_record(&record, place).as_ref(),
            Ok(&expected),
            "{}",
            case.name
        );
        assert_eq!(check_files_record(&record, place), Ok(()), "{}", case.name);
        assert_eq!(
            encode_files_record(&expected).as_ref(),
            Ok(&record),
            "{}",
            case.name
        );
    }
    for case in invalid {
        let record = bytes(&case.hex);
        let place = place(&case.place);
        assert!(
            decode_files_record(&record, place).is_err(),
            "{}",
            case.name
        );
        assert!(check_files_record(&record, place).is_err(), "{}", case.name);
    }
}

#[test]
fn requests_follow_the_corpus_in_binding_and_schema() {
    let mut schema = Schema::new();
    for corpus in corpora() {
        requests_follow(&corpus, &mut schema);
    }
}

fn requests_follow(corpus: &Corpus, schema: &mut Schema) {
    for case in &corpus.requests.valid {
        let request =
            parse_request(&case.bytes()).unwrap_or_else(|error| panic!("{}: {error}", case.name));
        let name = case.json.as_ref().unwrap()["op"].as_str().unwrap();
        assert_eq!(request.operation().name(), name, "{}", case.name);
        assert_eq!(
            request.capability().name(),
            corpus.capability,
            "{}",
            case.name
        );
        assert_eq!(
            parse_request(&encode_request(&request)).as_ref(),
            Ok(&request),
            "{}",
            case.name
        );
        assert!(
            schema.admits("request", case.json.as_ref().unwrap()),
            "{}",
            case.name
        );
    }
    for case in &corpus.requests.invalid {
        assert_eq!(
            parse_request(&case.bytes()).err(),
            Some(RequestError::Invalid),
            "{}",
            case.name
        );
        if let Some(json) = &case.json {
            assert_eq!(
                schema.admits("request", json),
                case.binding_boundary(),
                "{}",
                case.name
            );
        }
    }
    for case in &corpus.requests.unsupported {
        assert_eq!(
            parse_request(&case.bytes()).err(),
            Some(RequestError::Unsupported),
            "{}",
            case.name
        );
        assert!(
            !schema.admits("request", case.json.as_ref().unwrap()),
            "{}",
            case.name
        );
    }
}

#[test]
fn replies_follow_the_corpus_in_binding_and_schema() {
    let corpora = corpora();
    let mut schema = Schema::new();
    let results = all_operations(&corpora)
        .map(|facts| (facts.op.clone(), facts.result.clone()))
        .collect::<HashMap<_, _>>();
    let mut admits = |document: &Document, json: &Value| {
        let result = &results[document.op.as_deref().unwrap()];
        schema.admits("errorReply", json)
            || result
                .as_ref()
                .is_some_and(|definition| schema.admits(definition, json))
    };
    let replies = corpora.iter().map(|corpus| &corpus.replies);
    let valid = replies.clone().flat_map(|cases| &cases.valid);
    let invalid = replies.flat_map(|cases| &cases.invalid);
    for case in valid {
        let operation = operation(case);
        let reply = parse_reply(operation, &case.bytes())
            .unwrap_or_else(|error| panic!("{}: {error}", case.name));
        assert_eq!(
            parse_reply(operation, &encode_reply(&reply)).as_ref(),
            Ok(&reply),
            "{}",
            case.name
        );
        assert!(admits(case, case.json.as_ref().unwrap()), "{}", case.name);
    }
    for case in invalid {
        assert!(
            parse_reply(operation(case), &case.bytes()).is_err(),
            "{}",
            case.name
        );
        if let Some(json) = &case.json {
            assert_eq!(admits(case, json), case.binding_boundary(), "{}", case.name);
        }
    }
}

#[test]
fn notices_follow_the_corpus_in_binding_and_schema() {
    let corpora = corpora();
    let mut schema = Schema::new();
    let notices = all_operations(&corpora)
        .map(|facts| {
            (
                facts.op.clone(),
                (facts.snapshot.clone(), facts.update.clone()),
            )
        })
        .collect::<HashMap<_, _>>();
    let kind = |document: &Document| match document.kind.as_deref() {
        Some("snapshot") => NoticeKind::Snapshot,
        Some("update") => NoticeKind::Update,
        other => panic!("{}: unknown notice kind {other:?}", document.name),
    };
    let mut admits = |document: &Document, json: &Value| {
        let (snapshot, update) = &notices[document.op.as_deref().unwrap()];
        let definition = match kind(document) {
            NoticeKind::Snapshot => snapshot,
            NoticeKind::Update => update,
        };
        definition
            .as_ref()
            .is_some_and(|definition| schema.admits(definition, json))
    };
    let cases = corpora.iter().map(|corpus| &corpus.notices);
    let valid = cases.clone().flat_map(|cases| &cases.valid);
    let invalid = cases.flat_map(|cases| &cases.invalid);
    for case in valid {
        let (operation, kind) = (operation(case), kind(case));
        let notice = parse_notice(operation, kind, &case.bytes())
            .unwrap_or_else(|error| panic!("{}: {error}", case.name));
        assert_eq!(
            parse_notice(operation, kind, &encode_notice(&notice)).as_ref(),
            Ok(&notice),
            "{}",
            case.name
        );
        assert!(admits(case, case.json.as_ref().unwrap()), "{}", case.name);
    }
    for case in invalid {
        assert!(
            parse_notice(operation(case), kind(case), &case.bytes()).is_err(),
            "{}",
            case.name
        );
        assert!(!admits(case, case.json.as_ref().unwrap()), "{}", case.name);
    }
}
