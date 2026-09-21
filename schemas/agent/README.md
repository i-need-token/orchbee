# Agent Canonical Conformance Corpus

## Status

[`v1/agent-canonical-conformance-v1.json`](v1/agent-canonical-conformance-v1.json) is an
**executed parity corpus**, not a contract schema and not public ingress authority. Its
expectations were produced by running the authoritative `packages/agent` TypeScript
implementation at the recorded base commit; it exists so the staged Rust port in
`apps/orchbee-server` can prove behavioral equivalence. The fixture format is private —
nothing here admits wire, CLI, or provider-facing semantics.

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

## Provenance and regeneration

`provenance` inside the fixture records the base commit and `sourceFingerprints`: the
SHA-256 of every `packages/agent/src` file the corpus was generated from. Regeneration
is owned by `packages/agent` and imports the owning package build output — never a
vendored copy of the TypeScript sources:

```sh
pnpm -F @orchbee/agent build
pnpm -F @orchbee/agent conformance:generate   # rewrites the fixture
pnpm -F @orchbee/agent test:conformance       # regenerates in memory and byte-compares (drift check)
```

If the TypeScript authority or the base changes, the check run reports drift; regenerate
deliberately and review the fixture diff. Do not edit the JSON by hand — expectations
must always be executed output.

## Consumers

- `apps/orchbee-server` `src/agent` tests replay the corpus through the staged Rust
  port (`include_str!`, decoded with the existing `serde_json` dev-dependency).
- `packages/agent` owns the generator and may re-run it as evidence when the TypeScript
  model changes.

Simplified Chinese maintenance translation: [README.zh-CN.md](README.zh-CN.md).
English remains canonical.
