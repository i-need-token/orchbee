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

| Source boundary                   | Named import or root                         | Responsibility                                                                                                                                               | Allowed imports                                                                                                                                                                |
| --------------------------------- | -------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `src/protocol/semantic_input.zig` | `terminal-input`                             | Canonical semantic input value types shared by codecs and the engine                                                                                         | None                                                                                                                                                                           |
| `src/protocol/`                   | `terminal-protocol` through `root.zig`       | Native mirror of shared wire contracts and binary codecs                                                                                                     | `terminal-input`                                                                                                                                                               |
| `src/model/`                      | `terminal-host-model` through `root.zig`     | Canonical Host identifiers, configuration, runtime theme, geometry, lifecycle, launch/profile, attachment, projection, history, and selection command values | `terminal-protocol`                                                                                                                                                            |
| `src/platform/`                   | `terminal-host-platform` through `root.zig`  | PTY, process-containment, and operating-system adaptation                                                                                                    | `pty-c` on supported POSIX targets                                                                                                                                             |
| `src/engine/`                     | `terminal-host-engine` through `root.zig`    | One terminal engine, Ghostty state, launch, runtime-theme application, input, projection, history, and encoding                                              | `terminal-input`, `terminal-protocol`, `terminal-host-model`, `terminal-host-platform`, `ghostty-vt`                                                                           |
| `src/core/`                       | `terminal-host-core` through `root.zig`      | Pure Host, registry, terminal-session configuration/theme, broker-session, attachment, and delivery state owners                                             | `terminal-host-model`                                                                                                                                                          |
| `src/control/`                    | `terminal-host-control` through `root.zig`   | Lossless adaptation between control records and typed Host commands, replies, events, and complete theme state                                               | `terminal-protocol`, `terminal-host-model`                                                                                                                                     |
| `src/service/`                    | `terminal-host-service` through `root.zig`   | Request workflows, runtime/theme coordination, egress transactions, maintenance, and Host service lifetime                                                   | `terminal-input`, `terminal-protocol`, `terminal-host-model`, `terminal-host-platform`, `terminal-host-engine`, `terminal-host-core`, `terminal-host-control`, `build-options` |
| `src/transport/`                  | `terminal-host-transport` through `root.zig` | Local IPC endpoint, connection framing, peer validation, and transport lifetime                                                                              | `terminal-protocol`, `terminal-host-service`                                                                                                                                   |
| `src/cli/`                        | `terminal-host-cli` through `root.zig`       | Launcher grammar and validated executable configuration                                                                                                      | `terminal-host-model`, `terminal-host-transport`                                                                                                                               |
| `src/terminal_host.zig`           | `terminal-host`                              | Private executable facade and canonical Host version exposure                                                                                                | `terminal-host-cli`, `terminal-host-service`, `terminal-host-transport`                                                                                                        |
| `src/main.zig`                    | executable root                              | Process composition and top-level diagnostics                                                                                                                | `terminal-host`                                                                                                                                                                |
| `src/benchmark/`                  | `terminal-host-benchmark` through `main.zig` | Controlled PTY throughput harness                                                                                                                            | `terminal-host-cli`, `terminal-host-model`, `terminal-host-engine`                                                                                                             |

Dependencies point only from a lower row to an explicitly listed earlier semantic capability. A
module never imports another directory by relative path, reaches through another module's root to an
implementation file, or exposes mutable internals so a caller can bypass the owning operation.

## Internal review seams

The directory boundaries above remain the build-graph authority. Within them, the following
stateful or platform-dependent seams are durable review units rather than incidental file splits:

| Boundary       | Internal owner                                                                                                                                                                                              | Coordinator or facade responsibility                                                                                      |
| -------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| Protocol       | `message_kind.zig` owns the exact numeric kind registry; codec and format modules own their bounded wire shapes                                                                                             | `contract.zig` owns version/bound constants and role/encoding/success mappings; `root.zig` exposes the protocol namespace |
| Model          | `lifecycle/`, plus attachment, interaction, launch, profile, and runtime-theme value modules                                                                                                                | `root.zig` exposes immutable cross-layer values without importing a stateful runtime boundary                             |
| Engine         | `input_queue.zig`, `input_ownership.zig`, `history/`, `projection/`, and selection owners                                                                                                                   | `runtime/` orders cross-owner commits under the canonical terminal and input locks                                        |
| Engine runtime | `runtime/` state, handle adapters, lifecycle, I/O, theme, terminal mutation, projection, interaction, and test seams                                                                                        | `runtime/root.zig` preserves one public namespace while transaction files own independently reviewable runtime operations |
| Core           | `state/` connection, terminal/theme, attachment, and projection transactions, plus their dedicated state owners                                                                                             | `state/root.zig` preserves the Core namespace; `host_core.zig` retains the single concrete Core owner                     |
| Control        | `decode_*` and `encode_*` host, terminal, theme, projection, and interaction adapters                                                                                                                       | `decode.zig` and `encode.zig` preserve one explicit public protocol surface                                               |
| Platform       | `process_session.zig` owns stable POSIX containment identity; `process_posix.zig` and `process_windows.zig` own target process adapters; `windows_command_line.zig` owns executable/batch argument encoding | `process.zig` selects the target implementation without reimplementing process behavior                                   |
| Service access | `runtime_aggregate/`, `runtime_maintenance/`, retained-terminal, theme, input-admission, history, and selection access                                                                                      | `runtime_aggregate/host_aggregate.zig` retains the single owner; its root preserves the existing service namespace        |
| Service egress | `interaction.zig`, `projection.zig`, and `outgoing.zig`                                                                                                                                                     | `event_delivery.zig` owns fair scheduling and complete-batch handoff                                                      |
| Service        | `terminal_requests/`, capability-named request workflows, `connection_workflow.zig`, and the test-only composition observer                                                                                 | `service.zig` owns lifecycle and composition; workflows use the aggregate instead of mutable Core internals               |
| Transport      | `connection.zig`, `local_server_posix.zig`, and `local_server_windows.zig`                                                                                                                                  | `root.zig` selects the local-server implementation and exposes the stable transport boundary                              |

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
- `src/__tests__/host_black_box_test.zig` is an external executable-boundary refinement. Its
  adjacent transport helper acts only as a native client; the built Host remains a separate child
  process and the test observes it exclusively through the launcher ABI and Local IPC protocol.
- Production roots do not publish codec implementation namespaces for white-box tests. When a
  boundary needs such a seam, its root exposes it only under a `builtin.is_test`-guarded `testing`
  namespace.
- `src/service/__tests__/composition_observation.zig` is a test-only copied projection assembled from
  owner-provided Core and Engine observations. It retains no domain state and supplies the shared
  cross-owner invariant oracle used after stable request and poll boundaries. The adjacent
  `composition_trace.zig` vocabulary and `composition_reference*.zig` executable reducer retain only
  test evidence and action-determined abstract facts; they cannot authorize or commit product work.
  They live with the generated-state-machine drivers, replay codecs, and shrinkers under
  `src/service/__tests__/` and consume only narrow owner-provided testing seams rather than becoming
  another Service coordinator or entering the production namespace.
  Commit-seam evidence and deterministic scheduling also remain test-only: the stable matrix maps
  each fallible commit boundary to named evidence, while scripted schedules constrain test event
  order without creating a production coordinator or another source of domain state.
- The build graph is executable visibility authority. Adding an allowed edge requires updating this
  reference and `build.zig` in the same accepted structural change.
- Native C and header files live with their owning adapter: process containment under
  `src/platform/`, and POSIX endpoint security or Windows local IPC under `src/transport/`.
  Windows-only black-box client calls may reuse the existing test support functions in that
  transport C unit without exposing them through a Zig product module.

## Review gate

Reject a structural change when it introduces a reverse edge, relative cross-directory import,
cycle, broad re-export, duplicate state owner, or callback that lets a lower module invoke its
orchestrator. If a valid operation needs such an edge, return to the runtime model and revise the
owner or commit boundary before changing the graph.

## Retained lifecycle encoding

Within the existing Control boundary, `encode_lifecycle.zig` owns the lossless
encoding of terminal phase, exit result, runtime failure, process observation
and containment, including exited/failed events. `encode_host.zig` composes
these values into Host inventory and retains handshake/default/configuration
encoding. Both are pure adapters: input values, output bytes, allocation/error
behavior and the existing `encode.zig` public namespace stay unchanged. There
is no new state, writer, module dependency or lifecycle transition. Existing
control and cross-language protocol conformance checks judge byte preservation.
