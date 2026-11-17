# Terminal Protocol Version Policy and Release History

This file is the authority for versions owned by `schemas/terminal` and, after the first
public release, their release history.

## Version domains

Five independent business contracts are in scope. Each carries a generation in the major
component of its existing version field, with minor and patch zero; generation 1 replaced `0.1.0`.
The [generation registry](../../connection/README.md#generation-registry) declares each contract's
supported and superseded generations under the entry named below; this file records what each
generation changed.

| Contract                  | Registry entry | Owns                                                                         |
| ------------------------- | -------------- | ---------------------------------------------------------------------------- |
| Wire protocol             | `worker-host`  | Handshake, transport envelope, control messages, input, and message registry |
| `OBTF` terminal frame     | `obtf`         | Packed terminal-frame bytes                                                  |
| `OBPB` projection buffer  | `obpb`         | Packed Host-to-render projection bytes                                       |
| `OBRW` row window         | `obrw`         | Packed render/text history-window bytes and render-detail graphics           |
| `OBKG` graphics extension | `obkg`         | Owned decoded images, resolved placements, layers, and animation tick        |

The NPM package version, Terminal Host executable version, Zig toolchain version, and third-party
dependency versions belong to their own release owners. They are not wire-compatibility versions
and do not advance with these five contracts automatically.

## Before the first automatic-update release

Releases before 0.1.0-beta.14 followed the repository
[pre-release version rule](../../../docs/ARCHITECTURE.md#data-boundaries): `v1/constants.json` and
the Rust, TypeScript, and Zig constants, encoders, decoders, fixtures, exports, tests, and current
protocol reference changed together whenever the current contract changed.

## From the first automatic-update release

The runtime accepts only its exact current generation; a record of any other generation is
malformed. The accepted
[versioning and upgrade direction](../../../proposals/platform/versioning-and-upgrade.md) adds
negotiated generations and capability names in its P0 phase, and the first automatic-update release
freezes the current generation of each domain. From then on, every externally observable change
adds a release-history entry here in the same change:

- A backward-compatible addition adds a capability name; the generation stays.
- A backward-compatible correction changes neither.
- An incompatible change that no capability can express introduces the next generation of its
  domain, only as the last resort of the direction's
  [compatibility window](../../../proposals/platform/versioning-and-upgrade.md#compatibility-window).

After P0, Worker and Terminal Host negotiate a common wire-protocol generation and capability names
in the handshake, and each attachment negotiates its frame-format generations. The wire-protocol
generation current at the first automatic-update release grows only by capabilities the Host
advertises; if it ever advances, it stays supported in every later 1.x release unless a security
fix requires its removal. Every other generation follows the direction's compatibility window,
which the registry's [`superseded` dates](../../connection/README.md#compatibility-window) start.
Fixtures of a released generation are frozen: tests never change their expected bytes to admit a
new change, and the registry's [frozen fixtures](../../connection/README.md#frozen-fixtures) seal
them.

## Fixture naming

Fixture names describe their content without a version suffix, such as `transport-envelope.bin`
and `ghostty-render-row-window.bin`. The exact owning wire or format version lives in the fixture
contents and implementation constants. `canonical.json` is generator input for the current
fixtures, not a compatibility artifact.

## Released versions

- 0.1.0-beta.14, the first automatic-update release: generation 1 of all five contracts, encoded
  `1.0.0`, is frozen with the fixtures the registry seals.
- 0.1.0-beta.16: the wire protocol declares the capabilities `keeper` and `handover` with the
  [Host handover](../references/protocol.md#host-handover) kinds 90 to 105, the payload encoding
  `handover-record` (6), and its 4 MiB + 32 B bound. Generations stay at 1.
- 0.1.0-beta.22: a Windows Terminal Host offers `keeper` and serves `AttachKeeper`, `DetachKeeper`,
  `KeeperLost`, `KeeperUnresponsive`, `TerminalUnheld`, and `holdingId`, which this release
  freezes; no release Worker offers `keeper`. Generations stay at 1.
