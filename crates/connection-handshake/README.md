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

| API                                                                              | Contract                                                                                                                                                                              |
| -------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `encode_initiator_frame`, `encode_acceptor_frame`                                | Encode one frame with compact JSON, or refuse a payload above 1,048,576 bytes.                                                                                                        |
| `decode_initiator_frame`, `decode_acceptor_frame`                                | Decode exactly one complete in-memory frame for one direction.                                                                                                                        |
| `read_initiator_message`, `read_acceptor_message`                                | Read one frame from a stream and leave later bytes unread; report `ForeignProtocol` at the first byte that differs from `OBHS` and refuse a bad length before reading the payload.    |
| `CANDIDATE_DEADLINE`                                                             | The 2-second wait for a candidate's frame; the caller sets it as the stream's read timeout.                                                                                           |
| `Offer::local`, `AcceptorProfile::local`                                         | This build's offer or acceptor facts for a connection: registry generations and capabilities, and the fingerprint outside `orchbee`.                                                  |
| `AcceptorProfile::decide`                                                        | Run the ordered checks on a decoded frame and return `Accept`, `Serve`, `Unsupported`, or `Reject`.                                                                                   |
| `AcceptorProfile::reject`                                                        | Build a rejection with the acceptor's facts, for `malformed` after a decode or read failure and for `timeout`.                                                                        |
| `conclude_offer`, `conclude_request`                                             | Check the reply to an offer or a control request; the caller closes on every failure.                                                                                                 |
| `RequestKind::Handover`, `HandoverResult`, `HandoverTicket`                      | The `handover` control request, which only a Server serves, and its reply; a decoded result carries a reason exactly when it is a refusal.                                            |
| `ConnectionKind`, `FrameFormat`, `Delivery`                                      | The registry mirror: names, supported generations, `declared_capabilities`, and `ConnectionKind::development_fingerprint`. `capabilities` lists the declared names this build offers. |
| `FrameFormat::STREAMED`                                                          | The terminal stream formats every Session process carries, which a Server's query lists.                                                                                              |
| `KEEPER`, `HANDOVER`                                                             | The `worker-host` capabilities before a Worker may attach a Keeper and move terminals. The registry declares them; no build offers them before its Worker uses them.                  |
| `PROMPT_GATED_INPUT`                                                             | The capability name both connections list once a send may ask for a confirmed idle prompt; a peer without it gets no gated send.                                                      |
| `SCREEN_GATED_INPUT`                                                             | The original `worker-host` screen-gated input capability, retained for older Hosts.                                                                                                   |
| `SCREEN_GATED_SEND`                                                              | The `worker-host` capability required to send `ScreenGatedKeyInput` with input history preserved across an empty detach.                                                              |
| `SCREEN_GATED_PROMPT`                                                            | The `worker-host` capability for including a whole prompt in `ScreenGatedKeyInput`. The prompt also requires `SCREEN_GATED_SEND`.                                                     |
| `COMPATIBILITY_WINDOW`                                                           | The `client-server` capability that marks a Session of the first automatic-update release or later. Every build from that release lists it.                                           |
| `TERMINAL_RUNTIME`                                                               | The `client-server` capability before a Client may read `workbench.terminal-runtime`; a Client whose Server lacks it never sends the read.                                            |
| `STREAM_OUTCOME_UNKNOWN`                                                         | The `client-server` capability before a Server closes one OBWP request stream as `OutcomeUnknown` instead of ending the connection.                                                   |
| `UPDATE_CHECK`                                                                   | The `client-server` capability before a Client may send `workbench.update-check`; without it, Check for updates shows as unavailable.                                                 |
| `CONFIGURED_COMMAND_SOURCE_EXITED`                                               | The `client-server` capability before a Server refuses a configured command with `source-exited`; a Client without it reads `unavailable` for that refusal.                           |
| `TERMINAL_LIMIT_REACHED`                                                         | The `client-server` capability before a Server refuses a terminal create with `terminal-limit-reached`; a Client without it reads `server-busy` for that refusal.                     |
| `AGENT_CONFIRMATION`                                                             | The `client-server` capability before a Client may read or change how a Server answers an Agent's routine confirmations; without it, a Client shows nothing about them.               |
| `PROJECT_IDENTITY`                                                               | The `client-server` capability before a Client may name a Project's random identity in `project.create`; a Client never sends one to a Server without it.                             |
| `WORKSPACE_IDLE`                                                                 | The `client-server` capability before a Client may send `workbench.workspace-idle`; without it, a Client cannot tell what runs in a Workspace and asks before ending it.              |
| `RECONNECT`                                                                      | The `client-server` capability of a Client that copes with its Server being replaced: it comes back on its own or ends with its lost-connection outcome. Every Client offers it.      |
| `QueryResult::new`, `UnknownRequest::new`, `BuildEnvironment`, `GenerationRange` | Constructors that enforce cross-member rules, so a value that encodes is a valid message.                                                                                             |
| `Extensions`                                                                     | The open `extensions` member; later records that admit extensions may reuse it.                                                                                                       |
| `parse_strict_object`                                                            | The strict JSON object parser of the payload rules, for connections whose records follow them, such as `machine-hub` and `machine-link`.                                              |

Decoding rejects every rule of the contract: duplicate members, nesting deeper than 32, integer
members spelled with a fraction, exponent, or minus sign, `null` members, unknown members, and
unordered capability or connection lists. `reason`, `role`, `state`, and `step` decode unknown names
as `Unknown`. An initiator frame of an unknown type with a valid `environment` decodes as
`UnknownRequest`.

## Development fingerprint

`build.rs` reads `schemas/connection/v1/connections.json`, computes each negotiated connection's
fingerprint over its `fingerprintInputs` with [`build/fingerprint.rs`](build/fingerprint.rs), and
compiles the values into `ConnectionKind::development_fingerprint`. Cargo reruns the script when a
listed directory changes. The integration tests compile the same algorithm file and check it
against reference values computed with coreutils.

## Maintenance

- Change generations or capability names in `connections.json` and in `src/registry.rs` together;
  the registry test compares the declared names in both directions and holds every offered name
  among them.
- Add fixtures to `schemas/connection/v1/fixtures/` rather than Rust-only cases when a rule belongs
  to the contract. Bounds too large for a fixture file stay in `tests/conformance.rs`.
- `cargo test -p orchbee-connection-handshake` runs every fixture, the schema, the registry mirror,
  the fingerprint reference, and the registry's
  [compatibility window and frozen fixtures](../../schemas/connection/README.md#compatibility-window).
