# Semantic Command Protocol

Retained strict transport-neutral contracts for a product coordinator and CLI adapter. The legacy
TypeScript CLI and Desktop semantic endpoint are retired, so this package has no current application
consumer. It is outside the six retired workspaces and is not the native Rust CLI protocol.

## Public contract

The root export owns:

- bounded request, execution, coordinator-generation, operation, and schema-version identities;
- public operation descriptor projections with confirmation and exact-request retry policy;
- JSON-only invocation input and safe terminal result envelopes;
- active and terminal execution snapshots;
- operation list and description documents; and
- invocation admission, receipt lookup, and cooperative-cancellation outcomes.

Every object schema is strict. `SEMANTIC_COMMAND_PROTOCOL_VERSION` is version `1`, while each
operation negotiates its own positive `schemaVersion`. Completion kinds are `completed`,
`unchanged`, `blocked`, `cancelled`, `failed`, and `unsupported`; stable result codes carry the
specific meaning.

```ts
import {
	SemanticCommandInvocationRequestSchema,
	SemanticCommandOperationDescriptorSchema,
} from '@orchbee/semantic-command-protocol';

const descriptor = SemanticCommandOperationDescriptorSchema.parse(candidateDescriptor);
const request = SemanticCommandInvocationRequestSchema.parse(candidateRequest);
```

## Bundled local HTTP mapping

`@orchbee/semantic-command-protocol/local-http` owns the version-locked HTTP/1.1 JSON mapping used
between the local CLI adapter and a product coordinator. It accepts exactly one close-delimited
request and one JSON response per connection, rejects duplicate or transfer-encoding headers,
requires `application/json` on `POST`, and fails every unknown method or path closed.

| Method | Path                                        |
| ------ | ------------------------------------------- |
| `GET`  | `/v1/status`                                |
| `GET`  | `/v1/operations`                            |
| `GET`  | `/v1/operations/{operationId}`              |
| `POST` | `/v1/executions`                            |
| `GET`  | `/v1/requests/{requestIdentity}`            |
| `GET`  | `/v1/executions/{executionIdentity}`        |
| `POST` | `/v1/executions/{executionIdentity}/cancel` |

Request headers, request bodies, response bodies, and locator documents are bounded at 16 KiB,
64 KiB, 1 MiB, and 4 KiB respectively. A strict locator contains only protocol and locator versions,
one coordinator generation, and one 32-character lowercase hexadecimal endpoint identity. It
contains no path, network address, credential, or product data. The subpath supplies schemas,
framing, exact routes, and client request builders; it does not open or discover an endpoint.

## Ownership boundary

This package contains no feature catalog and grants no authority. Feature owners retain input and
output schemas, target rules, authorization, confirmation admission, execution policy, commit
points, and result mapping. Coordinators compose an explicit allowlist and own execution records.
Clients own presentation and transport adapters.

The root has no HTTP method, path, socket, named-pipe, endpoint-discovery, persistence, Action,
Workbench, Electron, or Node dependency. The `./local-http` subpath remains universal and owns no
filesystem enumeration, socket or named-pipe lifecycle, peer authentication, retry, or target
selection. Those mechanisms stay with the CLI and coordinator adapters.

## Verification

Run from the repository root:

```sh
pnpm --filter @orchbee/semantic-command-protocol build
pnpm --filter @orchbee/semantic-command-protocol typecheck
pnpm --filter @orchbee/semantic-command-protocol test
pnpm --filter @orchbee/semantic-command-protocol test:exports
```
