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

## Capsule location and ownership

Each environment installs Capsules in its own [data root](../local-runtime/README.md#data-root),
under `internal/integrations/agent-observation/<version>/`. The version is
`bundle/package.json`'s; a name that already holds a different payload takes the first free
`<version>-2`, `<version>-3`, and so on, and an install whose manifest declares the current digest
is reused. `capsule.json` keeps the digest as the identity. Publishing writes the owner record
`bundle/scripts/environment.json`, `{"environment":"<stem>","version":1}`, which the digest
excludes, so every copy a provider makes of `bundle/` names the environment that installed it.

An Agent entry belongs to the environment whose Capsule root it names. A root in another data
root's Capsule folder is that environment's, and a folder of `~/.orchbee/agent-observation-adapters`,
where every environment installed before each kept its own, is release's. A copied scripts folder
belongs to the environment its owner record names, and to release when it has none. Enable and
disable add, update and remove only this environment's entries: hook-configuration documents keep
other environments' entries in place, while a provider that admits one entry reports another
environment's entry as `conflict` and refuses to replace it. Release treats the shared folder's
entries as outdated, so its explicit enable replaces them and its disable removes them; no
environment ever deletes that folder.

At hook time the transport reads the owner record beside it and reports only when the inherited
`ORCHBEE_ENVIRONMENT` equals the owner, or when the variable is absent and the owner is release;
otherwise the hook exits silently and successfully. After a completed enable or disable the writer
removes the other version folders of its own Capsule folder once a fresh inspection shows that no
entry can name one; the current version stays until uninstall.

Because only an installed Capsule lets an entry name an environment, the local runtime's
[`agent_integration_may_have_been_enabled`](../local-runtime/README.md#data-root) answers from the
presence of these two folders without running provider commands or reading provider
configuration, and implicit callers skip inspection when it is `false`.

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
