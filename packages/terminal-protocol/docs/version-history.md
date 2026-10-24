# Terminal Protocol Version Policy and Release History

This file is the authority for semantic versions owned by `@orchbee/terminal-protocol` and, after
the first public release, their release history.

## Version domains

Five independent business-contract versions are in scope:

| Contract                  | Current version | Owns                                                                         |
| ------------------------- | --------------- | ---------------------------------------------------------------------------- |
| Wire protocol             | `0.1.0`         | Handshake, transport envelope, control messages, input, and message registry |
| `OBTF` terminal frame     | `0.1.0`         | Packed terminal-frame bytes                                                  |
| `OBPB` projection buffer  | `0.1.0`         | Packed Host-to-render projection bytes                                       |
| `OBRW` row window         | `0.1.0`         | Packed render/text history-window bytes and render-detail graphics           |
| `OBKG` graphics extension | `0.1.0`         | Owned decoded images, resolved placements, layers, and animation tick        |

The NPM package version, Terminal Host executable version, Zig toolchain version, and third-party
dependency versions belong to their own release owners. They are not wire-compatibility versions
and do not advance with these five contracts automatically.

## Before the first public release

Follow the repository [pre-release version rule](../../../docs/ARCHITECTURE.md#data-boundaries).
Update the Rust, TypeScript, and Zig constants, encoders, decoders, fixtures, exports, tests, and
current protocol reference together whenever the current contract changes.

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

Fixture names describe their content without a version suffix, such as `transport-envelope.bin`
and `ghostty-render-row-window.bin`. The exact owning wire or format version lives in the fixture
contents and implementation constants. `canonical.json` is generator input for the current
fixtures, not a compatibility artifact.

## Released versions

None. Release-history entries begin with the first public `1.0.0` contract.
