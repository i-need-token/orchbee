# Agent Canonical Conformance Corpus

## Status

[`v1/agent-canonical-conformance-v1.json`](v1/agent-canonical-conformance-v1.json) is a
**frozen executed parity corpus**, not a contract schema and not public ingress authority.
Its expectations were produced by running the TypeScript reference implementation that
`packages/agent` held at the recorded base commit; it pins the behavior of the Rust
implementation in `apps/orchbee-server`. The fixture format is private — nothing here
admits wire, CLI, or provider-facing semantics.

## What the corpus pins

- 29 vectors covering all nine semantic statuses, freshness transitions, attention
  request/resolve and turn-scoped auto-clear, turn outcomes, session end and
  heuristic-presence retire, stale/disconnected/post-end behavior, sequence deduplication
  and generation fencing, re-observe idempotency, presentation attach/detach/visibility
  conflicts, parentage, capability and source gating, journal paging/retention/reset,
  UTF-16 canonical ordering, aggregate and location projections, and runtime disposal.
- Every vector records its ordered `events[]` and the expected per-event transition
  (`kind`, `reason` when rejected, post-transition `revision`, ordered `semanticEvents`),
  the complete final session map with snapshot/summary/status/explanation/
  `matchedWaitConditions`, `journalReads[]`, and — when computed — the `aggregate` and
  `location` projection results **with their recorded inputs** so consumers can replay
  them exactly.
- `events[].programGenerated` marks the v24 retention program's generated events; the
  emitted vector already embeds all 520 generated events, so consumers replay the file
  verbatim and never re-expand a template.
- `journalReads[].onEmptyRuntime` selects a second, fresh runtime for that read; the
  change layer has no `baseline` kind (the operation layer above it synthesizes that).
- A `null` `aggregate` or `location` means "not computed for this vector" — consumers
  skip the assertion rather than expecting a computed null.

## Provenance and changes

`provenance` inside the fixture records the base commit and `sourceFingerprints`: the
SHA-256 of every `packages/agent/src` file the corpus was generated from. Each vector's
`sourceRefs` cite lines of those files at that commit. The TypeScript reference was
removed once no Client consumed it, so the corpus can no longer be regenerated. A
deliberate change to canonical behavior edits the affected vectors together with the Rust
implementation and the tests that replay them, and is reviewed as a contract change; never
edit an expectation only to make a failing replay pass.

## Consumers

- `apps/orchbee-server` `src/agent` tests replay the corpus through the Rust
  implementation (`include_str!`, decoded with the existing `serde_json` dev-dependency).
