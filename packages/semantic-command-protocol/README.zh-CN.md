# 语义命令协议

由产品 coordinator 与 Orchbee CLI client 共享的严格 transport-neutral 契约。

## 公共契约

Root export 拥有：

- 有界的 request、execution、coordinator-generation、operation 与 schema-version identity；
- 带 confirmation 和 exact-request retry policy 的 public operation descriptor projection；
- 仅包含 JSON 的 invocation input 与 safe terminal result envelope；
- Active 与 terminal execution snapshot；
- Operation list 与 description document；以及
- Invocation admission、receipt lookup 与 cooperative-cancellation outcome。

每个 object schema 都是严格的。`SEMANTIC_COMMAND_PROTOCOL_VERSION` 为版本 `1`，而每个 operation
单独协商自己的正数 `schemaVersion`。Completion kind 为 `completed`、`unchanged`、`blocked`、
`cancelled`、`failed` 和 `unsupported`；具体含义由稳定 result code 承载。

```ts
import {
	SemanticCommandInvocationRequestSchema,
	SemanticCommandOperationDescriptorSchema,
} from '@orchbee/semantic-command-protocol';

const descriptor = SemanticCommandOperationDescriptorSchema.parse(candidateDescriptor);
const request = SemanticCommandInvocationRequestSchema.parse(candidateRequest);
```

## Bundled 本地 HTTP mapping

`@orchbee/semantic-command-protocol/local-http` 拥有本地 CLI adapter 与产品 coordinator 之间使用的
version-locked HTTP/1.1 JSON mapping。它在每条连接上只接受一个 close-delimited request 和一个 JSON
response，拒绝重复 header 或 transfer-encoding header，要求 `POST` 使用 `application/json`，并对所有
未知 method 或 path 执行 fail closed。

| Method | Path                                        |
| ------ | ------------------------------------------- |
| `GET`  | `/v1/status`                                |
| `GET`  | `/v1/operations`                            |
| `GET`  | `/v1/operations/{operationId}`              |
| `POST` | `/v1/executions`                            |
| `GET`  | `/v1/requests/{requestIdentity}`            |
| `GET`  | `/v1/executions/{executionIdentity}`        |
| `POST` | `/v1/executions/{executionIdentity}/cancel` |

Request header、request body、response body 与 locator document 的上限分别为 16 KiB、64 KiB、1 MiB
和 4 KiB。严格 locator 只包含 protocol 与 locator version、一个 coordinator generation，以及一个
32 字符小写十六进制 endpoint identity；它不包含 path、network address、credential 或产品数据。该
subpath 提供 schema、framing、精确 route 与 client request builder；它不会打开或发现 endpoint。

## 所有权边界

本 package 不包含 feature catalog，也不授予 authority。Feature owner 保留 input 与 output schema、
target rule、authorization、confirmation admission、execution policy、commit point 和 result mapping。
Coordinator 组合显式 allowlist 并拥有 execution record。Client 拥有 presentation 与 transport adapter。

Root 不依赖 HTTP method、path、socket、named-pipe、endpoint-discovery、persistence、Action、Workbench、
Electron 或 Node。`./local-http` subpath 仍是 universal 的，并且不拥有 filesystem enumeration、socket
或 named-pipe lifecycle、peer authentication、retry 或 target selection；这些机制仍归 CLI 与
coordinator adapter 所有。

## 验证

在仓库根目录运行：

```sh
pnpm --filter @orchbee/semantic-command-protocol build
pnpm --filter @orchbee/semantic-command-protocol typecheck
pnpm --filter @orchbee/semantic-command-protocol test
pnpm --filter @orchbee/semantic-command-protocol test:exports
```
