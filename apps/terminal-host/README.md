# Terminal Host

Terminal Host is Orchbee's local authoritative terminal-execution service. It owns native terminal identities, PTYs and process trees, canonical Ghostty state, retained lifecycle, and attachment-scoped terminal capabilities for one trusted Desktop broker.

The application is private and experimentally versioned. It installs an executable; it does not export a reusable native library.

## Contract authority

- [Runtime contract and correctness model](docs/runtime-model.md) owns Host lifecycle, state, delivery, retry, failure, concurrency, and evidence requirements.
- [Terminal protocol reference](../../packages/terminal-protocol/references/protocol.md) owns wire records, codecs, message schemas, format versions, and protocol safety limits.
- [Desktop](../desktop/README.md) owns product topology, recovery policy, presentation, and platform effects.

The accepted [Desktop update-continuity proposal](../../proposals/terminal-host/desktop-terminal-host-update-continuity.md) records future implementation design; it is not current runtime authority.

The runtime contract is accepted, but acceptance does not claim that every current implementation path already conforms. A behavioral change is complete only when its affected contract claims have the required executable evidence.

## Responsibility

Terminal Host owns:

- one retained terminal session and one native engine per `TerminalId`;
- PTY and process-tree creation, observation, termination, containment, and cleanup;
- the single canonical Ghostty state used by input encoding, projection, history, and selection;
- effective terminal configuration, ordered bounded input, revisions, and generations;
- broker-scoped attachments, demand, delivery flights, bell cursors, history anchors, selections, and exports;
- a private local IPC endpoint protected by operating-system user identity; and
- typed native adaptation of the shared terminal protocol.

It does not own Project, Workspace, Workbench, Tab, Panel, agent, or product-session models. It also does not own Electron lifecycle, product recovery choices, rendering, clipboard behavior, search queries, external navigation, updates, network listeners, remote authentication, encryption, or sharing.

## Runtime promises

The accepted contract requires these externally visible properties:

- One endpoint has at most one active physical Desktop broker session.
- A Desktop disconnect destroys that broker's attachments but does not terminate or dispose retained terminals.
- A Host with retained terminals remains available for reconnect. A Host with no broker and no retained terminal may exit.
- Terminal creation is atomic and idempotent by creation identity and launch fingerprint while the terminal remains retained in one Host epoch. Explicit disposal releases that identity.
- Terminal output becomes a normal exit only after child completion, normal PTY drain, and complete quiescence of the PTY writer.
- Unrecoverable PTY read, PTY write, terminal-reply, resize, or worker-integrity failures become an explicit typed failed outcome. The Host fences unsafe I/O, contains the owned process tree, and continues bounded cleanup retries even while no broker is connected.
- Graceful termination, force termination, and disposal are distinct. Desktop owns escalation timing; disposal is explicit.
- Bounds and scheduler backpressure reject or defer the current operation without corrupting retained state. A full outgoing batch returns its committed prefix and is not a connection failure.
- Large projection and history records cross engine-to-egress ownership once: producers write into reserved response storage or transfer an owned envelope-ready allocation instead of copying a complete payload into a second buffer.
- Input whose reply is lost after admission has an unknown outcome and must not be automatically replayed.
- No client receives raw PTY output. Clients receive bounded protocol projections, history rows, selection state, exports, and lifecycle events derived from canonical terminal state.
- Terminal control queries, including DECRQSS, read that same canonical Ghostty state and enqueue their replies through the Host-owned bounded terminal-reply path; attachments never emulate a second VT reply surface.
- Attachment resize and terminal-driven 80/132-column requests share one native geometry commit
  owner. The Host correlates OS PTY size, Ghostty dimensions, cell pixels, revision, generation, Core
  observation, and the projected snapshot before clients observe the change.
- Application clear and reset requests execute directly against the Host-owned Ghostty state for
  an input-capable attachment. Clear erases and homes the active screen without changing its modes;
  reset returns to a fresh primary-screen state. Neither operation writes escape bytes to the child
  or asks a projected client to emulate VT behavior.
- Each Ghostty bell advances one terminal-owned sequence. A read-capable attachment receives only
  later bells, once per committed event record; reconnect does not replay earlier bell intent.
- A successful grid reflow preserves a tracked selection and committed history anchor when Ghostty's pins remain valid, while cancelling stale row flights and already encoded export bytes. Screen replacement and retained-row eviction remain explicit selection eviction boundaries.

The complete state model, retry classes, commit boundaries, invariants, and evidence matrix live in the [runtime contract](docs/runtime-model.md).

## Trust and continuity boundary

The runtime directory and endpoint are private to the operating-system user. POSIX uses a trusted per-user rendezvous, a no-follow directory descriptor, effective-UID ownership, private mode, removal and validation of extended and inheritable ACLs, a lifetime lock, and post-bind validation of the actual Unix-domain socket. Host and Desktop both require the connected peer's effective UID through kernel credentials on every connection. Windows uses an owned non-reparse runtime directory, protected DACLs, a singleton mutex, a local named pipe, and peer-SID validation.

These controls establish a same-user trust boundary. The endpoint identifier is not a secret, and Terminal Host does not distinguish one same-user application from another. It is not a multi-tenant service.

Desktop reconnect is supported while the Host survives. The POSIX authenticated transport helper owns only its proxy connection and bounded buffers; closing it does not terminate or otherwise own the Host. PTY migration or recovery after a Host or operating-system crash is not promised.

Process-tree ownership follows the containment domain created at launch: a dedicated POSIX session or a Windows kill-on-close Job. Cleanup is complete only after that domain has no live members. Preventing a trusted same-user child from deliberately escaping a POSIX session is not an isolation promise.

## Executable interface

The installed binary is `orchbee-terminal-host` on POSIX and `orchbee-terminal-host.exe` on Windows. Its private launcher ABI is:

```text
orchbee-terminal-host serve \
  --runtime-directory <absolute-path> \
  --endpoint-id <32-lowercase-hex> \
  [--scrollback-bytes <decimal-bytes>]
```

Desktop derives the runtime directory and endpoint identifier from its product user-data scope; the Host treats both as infrastructure values and does not infer product topology from them.

On POSIX, Desktop also uses the binary's private `posix-prepare-runtime` and `posix-connect` operations. The former prepares and validates the runtime-directory descriptor. The latter validates the directory and endpoint, connects, checks kernel peer credentials, reports authentication on file descriptor 3, and only then proxies bounded stdio to the socket. These operations are an internal Desktop/Host composition ABI, not a protocol compatibility layer or a general client interface.

`--scrollback-bytes` sets the startup default for newly created terminals. The current default is 10,000,000 decimal bytes; accepted values range from `0` through `1,000,000,000`. A broker may replace the process default for later terminals or supply an explicit configuration when creating one. Zero disables retained scrollback.

`orchbee-terminal-host --version` prints the executable name and its embedded semantic version. The binary version identifies the packaged process and is independent from protocol and binary format versions.

## Pinned native inputs

| Input                | Value                                                            |
| -------------------- | ---------------------------------------------------------------- |
| Zig                  | `0.15.2` exactly                                                 |
| Ghostty revision     | `91f66da24527fa02d92b5fd0b41cd020f553a64c`                       |
| Ghostty package hash | `ghostty-1.3.2-dev-5UdBCx16GwVpqni9yfmuNj13woRRCOiIvLQ64uB3CnbG` |

`build.zig.zon` is the dependency authority. The build rejects another Zig version or Ghostty archive content and does not read a sibling Ghostty checkout.

## Development

Run from the repository root:

```sh
pnpm --filter @orchbee/terminal-host format:check
pnpm --filter @orchbee/terminal-host build
pnpm --filter @orchbee/terminal-host test
pnpm --filter @orchbee/terminal-host test:protocol
pnpm --filter @orchbee/terminal-host test:surface-fixtures
pnpm --filter @orchbee/terminal-host test:tla
pnpm --filter @orchbee/terminal-host test:cross
pnpm verify
```

`test` runs the current platform's native tests, cross-language protocol conformance, and the
pinned-Ghostty surface-fixture comparison. `test:protocol` runs protocol conformance alone;
`test:surface-fixtures` regenerates the Host-owned primary/alternate OBTF corpus in a temporary
directory and rejects byte drift. The paired snapshots come from one Ghostty terminal and
Projection so metadata and rows can be checked across a real alternate-screen transition. After an
intentional reviewed corpus or encoder change, run
`pnpm --filter @orchbee/terminal-host surface-fixtures:generate` and review the binary consumer
assertions in `@orchbee/terminal`. `test:cross` compiles the complete test graph for Linux, FreeBSD,
macOS, and Windows; cross-compilation is not runtime evidence for those platforms. Repository CI
supplies native Linux, macOS, and Windows behavior evidence.

The narrower POSIX endpoint-security evidence can be run without the complete Host suite:

```sh
(cd apps/terminal-host && zig build test-cli)
(cd apps/terminal-host && zig build test-transport)
(cd apps/terminal-host && zig build test-transport-compile -Dtarget=x86_64-linux-gnu)
(cd apps/terminal-host && zig build test-transport-compile -Dtarget=x86_64-freebsd)
```

The native macOS run exercises real libc ACL inheritance and `getpeereid`; the cross-target steps compile the Linux `SO_PEERCRED` and FreeBSD `getpeereid` branches but are not runtime evidence for those systems.

`test:tla` model-checks the Host-owned registry, per-terminal lifecycle, multi-runtime shutdown, delivery, and request-outcome abstractions. It is change-triggered correctness evidence and is intentionally not part of the ordinary `test` or `pnpm verify` gate. It uses `java` from `PATH` by default; set `TERMINAL_HOST_JAVA` to an explicit Java executable when the runtime is installed elsewhere. Each model has a 120-second default budget so accidental state-space multiplication fails explicitly; `TERMINAL_HOST_TLA_TIMEOUT_MILLISECONDS` may override that budget for a slower verified environment.

The focused throughput harness runs through the real PTY:

```sh
pnpm --filter @orchbee/terminal-host benchmark -- \
  --bytes 67108864 --columns 120 --rows 40 --mode no-demand

pnpm --filter @orchbee/terminal-host benchmark -- \
  --bytes 67108864 --columns 120 --rows 40 --mode active

pnpm --filter @orchbee/terminal-host benchmark -- \
  --file /absolute/path/to/output.txt \
  --columns 113 --rows 38 \
  --cell-pixel-width 18 --cell-pixel-height 36 \
  --mode active
```

The file-fixture mode runs the repository's fixed workload as `time -p cat -n`, resolving both utilities through the child's effective `PATH` rather than fixed FHS paths; the Desktop harness applies the same synchronized policy. A maintained POSIX environment needs `time` and `cat` reachable on `PATH`, and the benchmark reports a clear error otherwise.

`ORCHBEE_TERMINAL_HOST_TRACE=1` enables bounded interaction timing records for the controlled Desktop benchmark harness. It is a test diagnostic, not product telemetry.

Build output remains in ignored `.zig-cache/` and `zig-out/` directories.

## Native composition boundary

`src/main.zig` is the executable composition root. `src/terminal_host.zig` is the narrow private
facade used by that executable. Each domain test graph imports its owning named module directly; the
facade does not re-export internals for tests. The build graph is the visibility authority: no other
workspace imports these Zig source files.

Source modules should follow the semantic owners in the runtime contract: local IPC transports records, the native protocol adapter translates records, the Host registry owns terminal membership, each terminal session owns one engine and lifecycle, and each broker session owns its attachments. A file split is useful only when it makes one of those ownership boundaries clearer.

The package-local [module boundary contract](docs/module-boundaries.md) maps those owners to source
directories, named Zig modules, allowed imports, and test placement.
