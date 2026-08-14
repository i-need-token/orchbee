# Terminal Host Module Boundaries

## Authority and scope

This reference maps the accepted semantic owners in
[`runtime-model.md`](runtime-model.md#semantic-ownership) to the Terminal Host native source tree.
Repository-wide semantic module design follows
[`docs/MODULE-BOUNDARIES.md`](../../../docs/MODULE-BOUNDARIES.md#semantic-module-design); Zig naming,
resource, safety, and testing rules follow [`docs/ZIG.md`](../../../docs/ZIG.md). This reference owns
only Terminal Host source-module responsibilities, visibility, and allowed dependency direction.

## Module graph

Every cross-module import uses the named import in this table. Direct relative imports stay inside
the importing module directory. A row may import only the rows listed in `Allowed imports`; standard
library, compiler-provided, generated build-option, and pinned external modules are listed only when
they affect the boundary.

| Source boundary                   | Named import or root                         | Responsibility                                                                                       | Allowed imports                                                                                                                                                                |
| --------------------------------- | -------------------------------------------- | ---------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `src/protocol/semantic_input.zig` | `terminal-input`                             | Canonical semantic input value types shared by codecs and the engine                                 | None                                                                                                                                                                           |
| `src/protocol/`                   | `terminal-protocol` through `root.zig`       | Native mirror of shared wire contracts and binary codecs                                             | `terminal-input`                                                                                                                                                               |
| `src/model/`                      | `terminal-host-model` through `root.zig`     | Canonical Host identifiers, configuration, geometry, and lifecycle values                            | `terminal-protocol`                                                                                                                                                            |
| `src/platform/`                   | `terminal-host-platform` through `root.zig`  | PTY, process-containment, and operating-system adaptation                                            | `pty-c` on supported POSIX targets                                                                                                                                             |
| `src/engine/`                     | `terminal-host-engine` through `root.zig`    | One terminal engine, Ghostty state, launch, input, projection, history, and encoding                 | `terminal-input`, `terminal-protocol`, `terminal-host-model`, `terminal-host-platform`, `ghostty-vt`                                                                           |
| `src/core/`                       | `terminal-host-core` through `root.zig`      | Host, registry, terminal-session, broker-session, attachment, and delivery state owners              | `terminal-host-model`, `terminal-host-engine`                                                                                                                                  |
| `src/control/`                    | `terminal-host-control` through `root.zig`   | Lossless adaptation between control records and typed Host commands, replies, and events             | `terminal-protocol`, `terminal-host-model`, `terminal-host-engine`, `terminal-host-core`                                                                                       |
| `src/service/`                    | `terminal-host-service` through `root.zig`   | Request workflows, runtime coordination, egress transactions, maintenance, and Host service lifetime | `terminal-input`, `terminal-protocol`, `terminal-host-model`, `terminal-host-platform`, `terminal-host-engine`, `terminal-host-core`, `terminal-host-control`, `build-options` |
| `src/transport/`                  | `terminal-host-transport` through `root.zig` | Local IPC endpoint, connection framing, peer validation, and transport lifetime                      | `terminal-protocol`, `terminal-host-service`                                                                                                                                   |
| `src/cli/`                        | `terminal-host-cli` through `root.zig`       | Launcher argument parsing and validated executable configuration                                     | `terminal-host-model`, `terminal-host-transport`                                                                                                                               |
| `src/terminal_host.zig`           | `terminal-host`                              | Private executable facade and canonical Host version exposure                                        | `terminal-host-cli`, `terminal-host-service`, `terminal-host-transport`                                                                                                        |
| `src/main.zig`                    | executable root                              | Process composition and top-level diagnostics                                                        | `terminal-host`                                                                                                                                                                |
| `src/benchmark/`                  | `terminal-host-benchmark` through `main.zig` | Controlled PTY throughput harness                                                                    | `terminal-host-cli`, `terminal-host-model`, `terminal-host-engine`                                                                                                             |

Dependencies point only from a lower row to an explicitly listed earlier semantic capability. A
module never imports another directory by relative path, reaches through another module's root to an
implementation file, or exposes mutable internals so a caller can bypass the owning operation.

## Internal review seams

The directory boundaries above remain the build-graph authority. Within them, the following
stateful or platform-dependent seams are durable review units rather than incidental file splits:

| Boundary       | Internal owner                                                                                       | Coordinator or facade responsibility                                                               |
| -------------- | ---------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| Engine         | `input_queue.zig`, `input_ownership.zig`, `selection.zig`, and `selection_export.zig`                | `runtime.zig` orders cross-owner commits under the canonical terminal and input locks              |
| Engine runtime | `runtime_launch.zig`, `runtime_startup.zig`, `runtime_completion.zig`, and `runtime_containment.zig` | `runtime.zig` owns worker lifetime and invokes each one-purpose state owner                        |
| Control        | `decode_*` and `encode_*` host, terminal, projection, and interaction adapters                       | `decode.zig` and `encode.zig` preserve one explicit public protocol surface                        |
| Platform       | `process_posix.zig` and `process_windows.zig`, with shared process type and common modules           | `process.zig` selects the target implementation without reimplementing process behavior            |
| Service        | `runtime_maintenance.zig`                                                                            | `runtime_aggregate.zig` coordinates Core/runtime transitions and delegates retained cleanup policy |
| Transport      | `connection.zig`, `local_server_posix.zig`, and `local_server_windows.zig`                           | `root.zig` selects the local-server implementation and exposes the stable transport boundary       |

A coordinator may retain a larger file when its code is the actual serialized commit protocol.
Do not move such code into a helper that would need mutable owner fields, lock-order knowledge, or
callbacks into the coordinator merely to reduce line count.

## Entry points and tests

- `root.zig` files are explicit namespace manifests or the implementation entry when the boundary
  has one runtime identity. They expose only operations and types required by declared consumers.
- `src/terminal_host.zig` composes product runtime modules; it does not become a second domain owner
  or a broad test facade.
- Unit tests live under the owning boundary's `__tests__/` directory and import the boundary by its
  named module. Cross-boundary conformance tests remain under `src/__tests__/` and use only named
  imports.
- The build graph is executable visibility authority. Adding an allowed edge requires updating this
  reference and `build.zig` in the same accepted structural change.
- Native C and header files live with their owning adapter: process containment under
  `src/platform/`, and POSIX endpoint security or Windows local IPC under `src/transport/`.

## Review gate

Reject a structural change when it introduces a reverse edge, relative cross-directory import,
cycle, broad re-export, duplicate state owner, or callback that lets a lower module invoke its
orchestrator. If a valid operation needs such an edge, return to the runtime model and revise the
owner or commit boundary before changing the graph.
