# orchbee-agent-integration-writer

Node-local Agent integration writer: provider inspection, enable and disable plans, and their
filesystem and process effects, using the cohort-admitted observation-adapter capsule and
retained mutation outcomes. Values and report vocabularies come from the
[Agent integration contract](../agent-integration/README.md); this crate never defines them.

## Ownership

- One invocation runs under one cooperative 40-second budget. Callers pass a
  `TerminationSource`; the writer observes it between bounded steps and settles as cancelled.
- Unix and Windows are supported. Windows configuration-location helpers compile only on Windows.
- The actual consumers are `orchbee-worker`, whose job owner runs every inspection and mutation
  for its Session, and `orchbee`, which reuses the read-only configuration helpers for offline
  `agent doctor` and Windows update verification.

## Observation adapter Capsule

[`agent-observation-adapters/`](agent-observation-adapters/README.md) is the source of the Capsule
this writer installs. Native staging and Desktop packaging copy it into each cohort, and
`bundled_payload_sha256` embeds its manifest's identity. Every byte of it except `capsule.json` is
payload, so any edit, including its README, needs a new `payloadSha256` and makes existing
installations `update-required` until `orchbee agent integration enable` repairs them. Its adapter
tests run from the repository root:

```sh
pnpm test:agent-observation-adapters
```

## Conformance fixtures

`tests/fixtures/` holds the corpora replayed by this crate's harnesses. The `test-support`
feature exposes both replays, and the CLI tests run them with the CLI's integration exit mapping:

- `provider-conformance/` — `payloads.json` plus `cases/<provider>.cases.json`:
  per-provider inspection, enable, and disable cases pinning scripted command
  replies, expected argv, filesystem effects, and the rendered CLI verdict.
- `provider-effects/` — `cases.json`: filesystem-effect machine cases —
  capsule publish, move-aside, backup digests, remove walks, cohort and
  version gates, and budget or cancellation frontiers.

The harnesses resolve both corpora through `CARGO_MANIFEST_DIR`, so any
checkout replays them with no environment override or machine-local path.
A case edit is a semantic contract change and belongs in the same reviewed
change as the engine behavior it pins; a new provider adds one file under
`provider-conformance/cases/`.
