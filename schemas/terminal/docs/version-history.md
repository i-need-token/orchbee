# Terminal Protocol Version Policy and Release History

This file is the authority for versions owned by `schemas/terminal` and, after the first
public release, their release history.

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
Update `v1/constants.json` and the Rust, TypeScript, and Zig constants, encoders, decoders, fixtures, exports, tests, and
current protocol reference together whenever the current contract changes.

## Public release and later

The runtime accepts only its exact current version. The accepted
[versioning and upgrade direction](../../../proposals/platform/versioning-and-upgrade.md) replaces
the semantic versions above with negotiated generations and capability names in its P0 phase, and
the first public release freezes the current generation of each domain. From then on, every
externally observable change adds a release-history entry here in the same change:

- An incompatible change introduces the next generation of its domain.
- A backward-compatible addition adds a capability name; the generation stays.
- A backward-compatible correction changes neither.

After P0, Worker and Terminal Host negotiate a common wire-protocol generation and capability names
in the handshake, and each attachment negotiates its frame-format generations. The wire-protocol
generation current at the first public release stays supported in every 1.x release unless a
security fix requires its removal; every other generation follows the direction's compatibility
window. Fixtures of a released generation are frozen: tests never change their expected bytes to
admit a new change.

## Fixture naming

Fixture names describe their content without a version suffix, such as `transport-envelope.bin`
and `ghostty-render-row-window.bin`. The exact owning wire or format version lives in the fixture
contents and implementation constants. `canonical.json` is generator input for the current
fixtures, not a compatibility artifact.

## Released versions

None. Release-history entries begin with the generations frozen at the first public release.
