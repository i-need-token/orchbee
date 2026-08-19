# Terminal Protocol Version Policy and Release History

This file is the authority for semantic versions owned by `@orchbee/terminal-protocol` and, after
the first public release, their release history.

## Version domains

Four independent business-contract versions are in scope:

| Contract                 | Current version | Owns                                                                         |
| ------------------------ | --------------- | ---------------------------------------------------------------------------- |
| Wire protocol            | `0.0.13`        | Handshake, transport envelope, control messages, input, and message registry |
| `OBTF` terminal frame    | `0.0.2`         | Packed terminal-frame bytes                                                  |
| `OBPB` projection buffer | `0.0.1`         | Packed Host-to-render projection bytes                                       |
| `OBRW` row window        | `0.0.1`         | Packed render/text history-window bytes                                      |

The NPM package version, Terminal Host executable version, Zig toolchain version, and third-party
dependency versions belong to their own release owners. They are not wire-compatibility versions
and do not advance with these four contracts automatically.

## Before the first public release

All business-contract versions use major version `0`. The repository retains only the exact current
development contract, currently `0.0.13`; it does not promise compatibility with an earlier `0.x`
shape or retain changelog entries for pre-release iteration. A pre-release contract update changes
the TypeScript and Zig constants, encoders, decoders, fixtures, exports, tests, and current protocol
reference together.

## Public release and later

The first public contract release sets each published contract to `1.0.0`. From that point onward,
every externally observable change must update the owning semantic version and add a release-history
entry here in the same change:

- `MAJOR` identifies an incompatible contract change.
- `MINOR` identifies a backward-compatible addition.
- `PATCH` identifies a backward-compatible correction.

The current runtime accepts only its exact current version; semantic versioning classifies the
contract change but does not imply an unimplemented negotiation range. Released fixture bytes must
never be silently overwritten under the same version.

## Fixture naming

Every compatibility fixture carries the complete semantic version of the contract that owns its
bytes or schema. Wire fixtures therefore use names such as `transport-envelope-v0.0.13.bin`, while
format fixtures use names such as `terminal-frame-v0.0.2.bin`. Wildcards and major-only suffixes are
not used. `canonical.json` is unversioned because it is generator input for the current fixtures,
not a compatibility artifact.

## Released versions

None. Release-history entries begin with the first public `1.0.0` contract.
