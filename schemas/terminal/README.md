# Terminal contract

## Authority

This directory owns the language-neutral Terminal wire contract. The
[current reference](references/protocol.md) defines layouts and semantics;
[version policy](docs/version-history.md) governs the five independent OBTP version domains.
The single `v1/` directory contains the current contract, whose five versions remain `0.1.0`.
It does not introduce a sixth wire version.

[`v1/constants.json`](v1/constants.json) is the manually maintained registry for magic values,
format versions and header sizes, message kinds and their role/encoding/success-response contracts,
envelope enum codes, and protocol limits. Change it with the reference and every affected binding.
Rust, Zig and TypeScript remain independent implementations; no binding or fixture generator
generates the registry or source code from it.

## Consumers and evidence

- `crates/terminal-protocol` consumes the registry in its private `wire_constants` test module,
  comparing constants actually used by decoders, encoded role flags, payload enum codes, the
  complete message registry, and its implemented transport limits.
- `apps/terminal-host` reads bounded registry and fixture bytes through its conformance build
  options. Its protocol suite compares the five formats, every message contract, envelope enum
  codes and all shared limits. Host builds do not require the TypeScript package.
- `packages/terminal-protocol` implements the TypeScript codecs and schemas. Its registry test
  compares the five formats, all message contracts, actual encoded envelope enum values and limits.
  The package exports code; repository consumers read fixtures directly from this directory.
- The existing affected-verification resolver selects all three bindings and their transitive
  consumers for a change here. TLA registrations consume this authority and the native bindings.

The registry checks replace comparisons against a TypeScript-produced constant snapshot.
Existing malformed-input, strict-boundary, native-production and consumer-semantic witnesses remain
required; agreement on constants alone does not prove codec or runtime behavior.

## Fixtures and maintenance

[`v1/fixtures/`](v1/fixtures/) owns canonical records and conformance corpora. File names are
unversioned; contents carry the current format. The TypeScript binding's existing
`fixtures:generate` and `test:fixtures` commands produce and check the codec fixtures in this
directory. `canonical.json` is generator input, and `control-conformance.json` contains payload
cases rather than a second constant registry.

Terminal Host's existing `surface-fixtures:generate` and `test:surface-fixtures` commands produce
and byte-compare the three `ghostty-*.bin` records. The terminal renderer consumes those same
records for its semantic checks. Native production, fixture authority, and consumer assertions
retain their separate responsibilities.
