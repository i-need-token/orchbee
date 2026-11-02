# orchbee-agent-integration

Managed Agent integration identities and the language-neutral operation contract exchanged by
native Clients, the Server relay, and the node-local writer. The crate owns values and codecs
only; the [writer](../agent-integration-writer/README.md) inspects and changes provider
configuration.

## Identity

`IntegrationId` names the 20 managed integrations. Contract documents spell an integration by its
provider-registry id (`provider()`, for example `antigravity`), the only provider vocabulary on
the wire. `as_str()` keeps the managed payload spelling (for example `antigravity-cli`), which
never appears in contract documents.

## Contract

[`schemas/agent-integration/v1`](../../schemas/agent-integration/README.md) is the language-neutral
authority; this crate is its Rust binding.

- Documents are UTF-8 JSON objects with camelCase fields, kebab-case `kind` tags, and no unknown
  fields. A field-less variant is an empty struct variant, because serde ignores unknown fields
  beside an internally tagged unit variant.
- `IntegrationRequest` is `{operation, adapterSource?, budgetMilliseconds?}`. `operation.kind` is
  `list` with an optional `provider`, or `explain`, `enable`, or `disable` with a required
  `provider`. A present budget is 1 through 40,000 milliseconds. An encoded request is at most
  8 KiB.
- `IntegrationReport.kind` is `list` (items and capsule status), `explain` (item, inspection
  detail, capsule status, and planned steps), `mutation` (one mutation report), or
  `home-unavailable`. An encoded report is at most 32 KiB.
- Closed vocabularies (states, issues, availability, management modes, provenance, Marketplace and
  target states, capsule source kinds, filesystem step operations, verdicts, refusals, and failure
  codes) decode only to known members.
- Capsule identity is `payloadSha256`.
- Node paths are lossless: `{"text": …}` for Unicode paths, `{"unixBytesHex": …}` for other Unix
  bytes, and `{"windowsUtf16": […]}` for other Windows code units. A path holds at most 4,096
  units, never contains NUL, and is rebuilt only on the platform family that produced it.

## Carriers

The [OBCN Agent integration job](../capability-node-protocol/README.md#agent-integration-job-messages)
carries encoded documents as opaque bytes between Server and Worker. The
[Workbench Agent integration direction](../workbench-protocol/README.md#agent-integration-jobs)
embeds them as JSON values between Client and Server.

## Verification

`src/contract/tests.rs` covers registry provider ids, budget and unknown-field refusal, lossless
node paths, and closed report vocabularies. The Workbench binding decides the shared
[conformance corpus](../../schemas/workbench/v1/workbench-agent-integration-conformance-v1.json)
with this crate's decoders.
