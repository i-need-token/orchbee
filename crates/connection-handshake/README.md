# Connection Handshake Rust Binding

## Scope

`orchbee-connection-handshake` is the Rust binding of `OBHS`, the permanent handshake that opens
every negotiated connection. The language-neutral contract, schema, registry, and fixtures live
under [`schemas/connection`](../../schemas/connection/README.md). The crate owns validated wire
values, the strict frame and message codec, the acceptor's ordered admission checks, the
initiator's checks of a reply, the Rust mirror of the generation registry, and this build's
development protocol fingerprints.

The crate does not open a transport, run a deadline timer, check peer credentials, count slots,
track lifecycle, perform a control request, pick a restart tier, decode the records that follow an
answer, derive the build's environment name, or localize a rejection.

## Public API

| API                                                                              | Contract                                                                                                                                                                           |
| -------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `encode_initiator_frame`, `encode_acceptor_frame`                                | Encode one frame with compact JSON, or refuse a payload above 1,048,576 bytes.                                                                                                     |
| `decode_initiator_frame`, `decode_acceptor_frame`                                | Decode exactly one complete in-memory frame for one direction.                                                                                                                     |
| `read_initiator_message`, `read_acceptor_message`                                | Read one frame from a stream and leave later bytes unread; report `ForeignProtocol` at the first byte that differs from `OBHS` and refuse a bad length before reading the payload. |
| `CANDIDATE_DEADLINE`                                                             | The 2-second wait for a candidate's frame; the caller sets it as the stream's read timeout.                                                                                        |
| `Offer::local`, `AcceptorProfile::local`                                         | This build's offer or acceptor facts for a connection: registry generations and capabilities, and the fingerprint outside `orchbee`.                                               |
| `AcceptorProfile::decide`                                                        | Run the ordered checks on a decoded frame and return `Accept`, `Serve`, `Unsupported`, or `Reject`.                                                                                |
| `AcceptorProfile::reject`                                                        | Build a rejection with the acceptor's facts, for `malformed` after a decode or read failure and for `timeout`.                                                                     |
| `conclude_offer`, `conclude_request`                                             | Check the reply to an offer or a control request; the caller closes on every failure.                                                                                              |
| `ConnectionKind`, `FrameFormat`, `Delivery`                                      | The registry mirror: names, supported generations, capability names, and `ConnectionKind::development_fingerprint`.                                                                |
| `PROMPT_GATED_INPUT`                                                             | The capability name both connections list once a send may ask for a confirmed idle prompt; a peer without it gets no gated send.                                                   |
| `QueryResult::new`, `UnknownRequest::new`, `BuildEnvironment`, `GenerationRange` | Constructors that enforce cross-member rules, so a value that encodes is a valid message.                                                                                          |
| `Extensions`                                                                     | The open `extensions` member; later records that admit extensions may reuse it.                                                                                                    |

Decoding rejects every rule of the contract: duplicate members, nesting deeper than 32, integer
members spelled with a fraction, exponent, or minus sign, `null` members, unknown members, and
unordered capability or connection lists. `reason`, `role`, and `state` decode unknown names as
`Unknown`. An initiator frame of an unknown type with a valid `environment` decodes as
`UnknownRequest`.

## Development fingerprint

`build.rs` reads `schemas/connection/v1/connections.json`, computes each negotiated connection's
fingerprint over its `fingerprintInputs` with [`build/fingerprint.rs`](build/fingerprint.rs), and
compiles the values into `ConnectionKind::development_fingerprint`. Cargo reruns the script when a
listed directory changes. The integration tests compile the same algorithm file and check it
against reference values computed with coreutils.

## Maintenance

- Change generations or capability names in `connections.json` and in `src/registry.rs` together;
  the registry test compares them in both directions.
- Add fixtures to `schemas/connection/v1/fixtures/` rather than Rust-only cases when a rule belongs
  to the contract. Bounds too large for a fixture file stay in `tests/conformance.rs`.
- `cargo test -p orchbee-connection-handshake` runs every fixture, the schema, the registry mirror,
  and the fingerprint reference.
