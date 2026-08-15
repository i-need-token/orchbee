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

| Source boundary                   | Named import or root                         | Responsibility                                                                                                                                | Allowed imports                                                                                                                                                                |
| --------------------------------- | -------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `src/protocol/semantic_input.zig` | `terminal-input`                             | Canonical semantic input value types shared by codecs and the engine                                                                          | None                                                                                                                                                                           |
| `src/protocol/`                   | `terminal-protocol` through `root.zig`       | Native mirror of shared wire contracts and binary codecs                                                                                      | `terminal-input`                                                                                                                                                               |
| `src/model/`                      | `terminal-host-model` through `root.zig`     | Canonical Host identifiers, configuration, geometry, lifecycle, launch/profile, attachment, projection, history, and selection command values | `terminal-protocol`                                                                                                                                                            |
| `src/platform/`                   | `terminal-host-platform` through `root.zig`  | PTY, process-containment, and operating-system adaptation                                                                                     | `pty-c` on supported POSIX targets                                                                                                                                             |
| `src/engine/`                     | `terminal-host-engine` through `root.zig`    | One terminal engine, Ghostty state, launch, input, projection, history, and encoding                                                          | `terminal-input`, `terminal-protocol`, `terminal-host-model`, `terminal-host-platform`, `ghostty-vt`                                                                           |
| `src/core/`                       | `terminal-host-core` through `root.zig`      | Pure Host, registry, terminal-session, broker-session, attachment, and delivery state owners                                                  | `terminal-host-model`                                                                                                                                                          |
| `src/control/`                    | `terminal-host-control` through `root.zig`   | Lossless adaptation between control records and typed Host commands, replies, and events                                                      | `terminal-protocol`, `terminal-host-model`                                                                                                                                     |
| `src/service/`                    | `terminal-host-service` through `root.zig`   | Request workflows, runtime coordination, egress transactions, maintenance, and Host service lifetime                                          | `terminal-input`, `terminal-protocol`, `terminal-host-model`, `terminal-host-platform`, `terminal-host-engine`, `terminal-host-core`, `terminal-host-control`, `build-options` |
| `src/transport/`                  | `terminal-host-transport` through `root.zig` | Local IPC endpoint, connection framing, peer validation, and transport lifetime                                                               | `terminal-protocol`, `terminal-host-service`                                                                                                                                   |
| `src/cli/`                        | `terminal-host-cli` through `root.zig`       | Launcher grammar, validated executable configuration, and the platform-neutral local-endpoint helper adapter                                  | `terminal-host-model`, `terminal-host-transport`                                                                                                                               |
| `src/terminal_host.zig`           | `terminal-host`                              | Private executable facade and canonical Host version exposure                                                                                 | `terminal-host-cli`, `terminal-host-service`, `terminal-host-transport`                                                                                                        |
| `src/main.zig`                    | executable root                              | Process composition and top-level diagnostics                                                                                                 | `terminal-host`                                                                                                                                                                |
| `src/benchmark/`                  | `terminal-host-benchmark` through `main.zig` | Controlled PTY throughput harness                                                                                                             | `terminal-host-cli`, `terminal-host-model`, `terminal-host-engine`                                                                                                             |

Dependencies point only from a lower row to an explicitly listed earlier semantic capability. A
module never imports another directory by relative path, reaches through another module's root to an
implementation file, or exposes mutable internals so a caller can bypass the owning operation.

## Internal review seams

The directory boundaries above remain the build-graph authority. Within them, the following
stateful or platform-dependent seams are durable review units rather than incidental file splits:

| Boundary       | Internal owner                                                                                                | Coordinator or facade responsibility                                                                                      |
| -------------- | ------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| Model          | `lifecycle/`, plus attachment, interaction, launch, and profile value modules                                 | `root.zig` exposes immutable cross-layer values without importing a stateful runtime boundary                             |
| Engine         | `input_queue.zig`, `input_ownership.zig`, `history/`, `projection/`, and selection owners                     | `runtime/` orders cross-owner commits under the canonical terminal and input locks                                        |
| Engine runtime | `runtime/` state, handle adapters, lifecycle, I/O, terminal mutation, projection, interaction, and test seams | `runtime/root.zig` preserves one public namespace while transaction files own independently reviewable runtime operations |
| Core           | `state/` connection, terminal, attachment, and projection transactions, plus their dedicated state owners     | `state/root.zig` preserves the Core namespace; `host_core.zig` retains the single concrete Core owner                     |
| Control        | `decode_*` and `encode_*` host, terminal, projection, and interaction adapters                                | `decode.zig` and `encode.zig` preserve one explicit public protocol surface                                               |
| Platform       | `process_posix.zig` and `process_windows.zig`, with shared process type and common modules                    | `process.zig` selects the target implementation without reimplementing process behavior                                   |
| Service access | `runtime_aggregate/`, `runtime_maintenance/`, retained-terminal, input-admission, and interaction access      | `runtime_aggregate/host_aggregate.zig` retains the single owner; its root preserves the existing service namespace        |
| Service egress | `interaction.zig`, `projection.zig`, and `outgoing.zig`                                                       | `event_delivery.zig` owns fair scheduling and complete-batch handoff                                                      |
| Service        | `terminal_requests/`, `connection_workflow.zig`, and the remaining request workflows                          | `service.zig` owns lifecycle and composition; workflows use the aggregate instead of mutable Core internals               |
| Transport      | `connection.zig`, `local_server_posix.zig`, and `local_server_windows.zig`                                    | `root.zig` selects the local-server implementation and exposes the stable transport boundary                              |

A semantic owner that outgrows one independently reviewable file becomes a directory. Its root is
a narrow namespace or mechanical facade; state declarations and branching transaction logic live
in responsibility-named files. A serialized commit protocol remains explicit in one transaction
module or typed prepare/commit handoff instead of turning the facade into a catch-all file.

Every hand-written production Zig file under `src/` must remain at or below 500 physical lines.
Tests are exempt because they may be scenario catalogs; generated sources, if introduced, remain
owned by their generator and are reviewed through that source instead. Approaching the limit is a
prompt to split by semantic transaction before the file crosses it, not a reason to compress code,
remove useful names, or hide logic behind an untyped helper.

An owner-local transaction file may mutate the private runtime state declared for that directory
only for its documented responsibility. It must not create a second owner, expose the state through
the public root, depend on a reverse module edge, or call an untyped callback into the facade.

## Entry points and tests

- `root.zig` files are explicit namespace manifests or the implementation entry when the boundary
  has one runtime identity. They expose only operations and types required by declared consumers.
- `src/terminal_host.zig` composes product runtime modules; it does not become a second domain owner
  or a broad test facade.
- Unit tests live under the owning boundary's `__tests__/` directory and import the boundary by its
  named module. Large suites are partitioned by semantic owner and share only domain harnesses from
  that test directory; they do not expose production internals merely to arrange fixtures.
  Cross-boundary conformance tests remain under `src/__tests__/` and use only named imports.
- Production roots do not publish codec implementation namespaces for white-box tests. When a
  boundary needs such a seam, its root exposes it only under a `builtin.is_test`-guarded `testing`
  namespace.
- The build graph is executable visibility authority. Adding an allowed edge requires updating this
  reference and `build.zig` in the same accepted structural change.
- Native C and header files live with their owning adapter: process containment under
  `src/platform/`, and POSIX endpoint security or Windows local IPC under `src/transport/`.

## Review gate

Reject a structural change when it introduces a reverse edge, relative cross-directory import,
cycle, broad re-export, duplicate state owner, or callback that lets a lower module invoke its
orchestrator. If a valid operation needs such an edge, return to the runtime model and revise the
owner or commit boundary before changing the graph.
