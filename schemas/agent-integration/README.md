# Agent Integration Cross-Language Contract

## Authority

Contract changes follow the repository [pre-release version rule](../../docs/ARCHITECTURE.md#data-boundaries).

[`v1/agent-integration.schema.json`](v1/agent-integration.schema.json) is the language-neutral
authority for the version-1 Agent integration operation: the request a Client asks a node to run
and the report the node's writer answers. The Rust binding in
[`orchbee-agent-integration`](../../crates/agent-integration/README.md) and the TypeScript binding
in [`@orchbee/workbench-client-protocol`](../../packages/workbench-client-protocol/README.md#agent-integration-jobs)
are independent implementations. Neither binding may add accepted values, omit invariants, or
redefine field meaning.

Version 1 uses decoded JSON values governed by JSON Schema 2020-12. Every object is closed, every
vocabulary is closed, and an integration is spelled by its provider-registry id, the only provider
vocabulary on the wire. A nullable field may be omitted, which means null.

## Rules beyond JSON Schema

- A carrier enforces its own encoded budget: 8 KiB per request and 32 KiB per report inside the
  [OBCN job messages](../../crates/capability-node-protocol/README.md#agent-integration-job-messages),
  and 10 KiB and 36 KiB for the whole
  [Workbench direction](../workbench/README.md#agent-integration-jobs).
- A text path holds at most 4,096 UTF-8 bytes; JSON Schema can bound only its characters.
- A Unix byte path or a Windows code-unit path is rebuilt only on the platform family that produced
  it, so a node refuses the other family's form. A binding that never rebuilds a path, such as the
  TypeScript one, accepts both forms.

## Conformance

The [Workbench Agent integration corpus](../workbench/v1/workbench-agent-integration-conformance-v1.json)
exercises these documents inside the Workbench direction. Rust, TypeScript, and JSON Schema decide
every case; a case marked `semantic` is shape-valid, and both bindings refuse it by a rule above.
Valid cases use only text paths, so every platform decides the corpus alike.

## Non-goals

This contract does not own provider configuration, inspection, mutation plans, Capsule contents,
job lifecycle, cancellation, deadlines, authorization, CLI grammar, or presentation. The
[writer](../../crates/agent-integration-writer/README.md) owns what an operation does, the Worker
owns its job, the Server owns admission, and each Client owns how it presents a report.
