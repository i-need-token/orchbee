# File Bar contract

## Authority

This directory is the language-neutral authority for the records a Client and a Server exchange
for the File Bar under the `files` capability: OBFB, the record OBWP stream frames carry, and the
JSON of every `files` operation.

| File                                                             | Content                                                         |
| ---------------------------------------------------------------- | --------------------------------------------------------------- |
| This README                                                      | Records, JSON rules, compatibility and the corpus format        |
| [`v1/files.schema.json`](v1/files.schema.json)                   | JSON Schema 2020-12 of every request, result, error and notice  |
| [`v1/files-conformance-v1.json`](v1/files-conformance-v1.json)   | Operations, bounds, and valid and invalid records and documents |
| [Backend proposal](../../proposals/platform/file-bar-backend.md) | The accepted design and its reserved decisions                  |

`crates/files-protocol` is the Rust binding, which `orchbee-server` and `orchbee-files` use.
Bindings are independent implementations of this contract and consume the corpus unchanged.

The contract is frozen when its batch is released; until then, a change edits the schema, the
corpus and every binding together. After the release, additions arrive under new capability names
and released records never change.

## Records

An OBFB record is a 12-byte header, `OBFB`, u16 version 1, u16 kind and u32 body length, all big
endian, followed by exactly that many body bytes.

| Kind      | Travels in                                  | Body                                                                     | Bound           |
| --------- | ------------------------------------------- | ------------------------------------------------------------------------ | --------------- |
| 1 Request | Client StreamOpen in Request or Events mode | Request JSON                                                             | 1 B – 16 KiB    |
| 2 Result  | Server StreamFrame Result                   | Result or error JSON                                                     | 1 B – 128 KiB   |
| 3 Notice  | Server StreamFrame Snapshot or Update       | Notice JSON                                                              | 1 B – 128 KiB   |
| 4 Put     | Client StreamOpen in Request mode           | u64 transfer, u64 offset, u8 flags, data                                 | 128 KiB of data |
| 5 Chunk   | Server StreamFrame Result                   | u64 transfer, u64 offset, u8 flags, u8 line ending, digest on flag, data | 128 KiB of data |

- A transfer is nonzero; offsets may be any u64.
- Put flags are 0, 1 (`last`) or 3 (`last` and `commit`).
- A Chunk that does not end its read has flags 0 and line ending 0. The final Chunk sets `last` (1)
  and may set `changed` (2), when the file's modification time or size moved during the read, and
  `digest` (4), when 32 bytes of SHA-256 over the file bytes the read consumed follow the line
  ending, which a text read reports so a later save can send it back as its precondition. Its line
  ending is 1 (none), 2 (LF) or 3 (CRLF).
- Any other kind, flag, line ending, length or placement is invalid, and a decoder refuses the
  record rather than skipping it.

## JSON

Every body is one JSON object under the [`OBHS` payload rules](../connection/README.md): UTF-8
without a byte order mark, no repeated member at any depth, at most 32 levels of nesting, and no
second value. Beyond the schema, bindings enforce:

- String bounds in UTF-8 bytes where the schema counts characters: paths at most 4,096 bytes and
  every other `maxLength` as bytes. The corpus marks the cases only a binding refuses as
  `"boundary": "binding"`.
- The record bounds: 16 KiB for a request and 128 KiB for a result or notice.
- `null` only where the schema names it; an optional member is absent instead.
- An integer may be spelled `1.0` or `1e0`, as JSON Schema admits; encoders write it without a
  fraction or exponent and omit an optional boolean that is false.

Each request names `op` and `workspaceId`. A Result carries either the operation's result or
`{"error": …}`; a subscription and `fs.read-chunk` answer with a Result only to fail. The corpus
`operations` table names, for every operation, its stream mode, the least Session grant, whether
it may name a Workspace that has closed, and the definitions of its result and notices.

## Compatibility

- A Server closes a stream whose `op` it does not serve with StreamClose Unsupported, and one whose
  request breaks the operation's rules with StreamClose InvalidRequest.
- A Client treats an error code it does not know as `failed`, a `reason` or `gitErrorCode` it does
  not know as absent, and a notice it cannot read as absent. `remote-disconnected` is the Client's
  own code and never crosses the wire.

## Corpus

`files-conformance-v1.json` holds `bounds`, `operations`, and `records`, `requests`, `replies` and
`notices`, each with `valid` and `invalid` cases; `requests` also lists `unsupported` cases.

- A record case names its `place`: `request-open`, `events-open`, `result-frame` or `notice-frame`.
  A valid case also states the decoded `record`, with u64 values as decimal strings and data as
  hexadecimal.
- A document case holds `json`, a JSON value, or `text`, JSON text a value cannot hold, such as a
  repeated member. Reply and notice cases name their `op`, and notice cases their `kind`,
  `snapshot` or `update`.
- A binding admits exactly the valid cases and encodes each decoded record to its bytes. The schema
  admits exactly the valid `json` cases, except those marked `"boundary": "binding"`.
