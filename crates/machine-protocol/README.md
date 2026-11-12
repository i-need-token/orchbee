# Machine Protocol Rust Binding

## Scope

`orchbee-machine-protocol` is the Rust binding of the two machine connections that follow an `OBHS`
answer: `machine-link`, from the remote-machine hub to the remote helper, and `machine-hub`, from a
client to the hub. The language-neutral contract and its fixtures live under
[`schemas/machine`](../../schemas/machine/README.md), which is the authority for every byte, member,
bound, and rule. The crate owns the `OBMX` frame codec, the JSON payload rules, the closed typed
records of both connections, and one end's channel state machine: channel ids, credit, the request
and `ping` ledgers, the receiver's verdicts, and the fair sender.

The crate performs no I/O, starts no thread, and keeps no clock. It does not open a carrier or an
endpoint, start `ssh`, run the `OBHS` exchange, time out a peer, decide hub policy, relay between
connections, carry a login answer, or localize an outcome. It builds for every platform the
workspace targets.

## Public API

| API                                                      | Contract                                                                                                                                                                                                                                |
| -------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Frame`, `FrameKind`, `decode_frame`, `FrameDecoder`     | Build a frame only within its kind's channel and length rules; decode one complete frame, or feed partial input and get each frame once its last byte arrives. A `FrameError` closes the connection through `Connection::frame_failed`. |
| `Record<P>::decode`, `Record<P>::encode`                 | Decode one frame's record for the sending role without connection state, refusing it at the `encoding` or `record` layer; encode with compact JSON, refusing a payload above 65,536 bytes.                                              |
| `link`, `hub`                                            | Each connection's typed channels, outcomes, requests, replies, events, and wire values; growable names decode their fallback, and constructors enforce cross-member rules.                                                              |
| `Connection<P>`                                          | One end of a connection: `initiator`, `acceptor` (a link acceptor queues its `facts` first), `receive` with a `Receipt` of `accepted`, `discarded`, or `reply`, or the `Violation` that closes it.                                      |
| `Connection::open` … `Connection::bye`                   | The send side: every call checks the same rules the peer enforces and returns a `SendError` instead of queueing a frame the peer would refuse.                                                                                          |
| `Connection::take`                                       | The consumer's report of taken bytes; `credit` follows at 131,072 taken bytes, and a responder holds it until it sends `accept`.                                                                                                        |
| `Connection::poll_transmit`                              | The next frame to write: other frames first, with state events replaced in place by subject, then one `data` frame per ready channel per round, `end` and `close` behind their data, and `bye` last.                                    |
| `Violation`, `Verdict`                                   | The fixture vocabulary for a receiver's outcome; after a violation the connection sends only `bye` with reason `violation`.                                                                                                             |
| `INITIAL_CREDIT`, `MAXIMUM_CREDIT`, and the other bounds | The contract's numeric bounds, for carriers and tests that size buffers by them.                                                                                                                                                        |
| `Extensions`                                             | The open `extensions` member, shared with `orchbee-connection-handshake`.                                                                                                                                                               |

The JSON payload rules reuse `parse_strict_object` from `orchbee-connection-handshake` and add the
encoding-layer refusal of `null` and of any number that is not a non-negative integer outside the
top-level `extensions` member.

## Maintenance

- Add contract cases to [`schemas/machine/v1/fixtures/`](../../schemas/machine/v1/fixtures/) rather
  than Rust-only cases. Rules that need thousands of frames or two connected ends stay in
  `tests/exercise.rs`: the flood, half-close, cancel, and relay exercises, the 257th `open`, the
  route reply bound, and event coalescing.
- Change the records and the contract together; the fixtures freeze the bytes of both connections.
- `cargo test -p orchbee-machine-protocol` runs every fixture and the two-ended exercise.
