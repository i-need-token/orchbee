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

- one retained terminal record that binds one semantic session to either one deferred launch
  preparation or one started native engine per `TerminalId`;
- PTY and process-tree creation, observation, termination, containment, and cleanup;
- the single canonical Ghostty state used by input encoding, projection, history, and selection;
- effective terminal configuration, complete revisioned runtime themes, ordered bounded input,
  revisions, and generations;
- broker-scoped attachments, demand, delivery flights, bell cursors, history anchors, selections, and exports;
- bounded OSC 52 plain-text write intent, bounded private-APC application-signal transport, current
  terminal progress, and latest semantic command completion facts derived from Ghostty output;
- a private local IPC endpoint protected by operating-system user identity; and
- typed native adaptation of the shared terminal protocol.

It does not own Project, Workspace, Workbench, Tab, Panel, Agent, provider, or product-session models, and it never derives Agent semantics from a terminal signal. It also does not own Electron lifecycle, product recovery choices, rendering, platform clipboard contents, application commit policy, clipboard effects, search queries, external navigation, updates, network listeners, remote authentication, encryption, or sharing.

## Runtime promises

The accepted contract requires these externally visible properties:

- One endpoint has at most one active physical Desktop broker session.
- A Desktop disconnect destroys that broker's attachments but does not terminate or dispose retained terminals.
- A Host with retained terminals remains available for reconnect. A Host with no broker and no retained terminal may exit.
- Terminal creation is atomic and idempotent by creation identity and durable launch fingerprint
  while the terminal remains retained in one Host epoch. A separate, non-persistent environment
  overlay is applied only to that creation generation and represented in retained state by a
  target-normalized, order-independent digest. A non-empty replay must match the digest; an empty
  replay may validate an existing retained creation without reconstructing its capability. Explicit
  disposal releases the identity and overlay generation.
- A launch with null geometry reserves identity, configuration, and prepared launch ownership in
  `awaiting-start` without spawning a child or PTY. `StartTerminal` requires one complete geometry;
  success uses it for the child, PTY, canonical state, Core observation, and first frame. Exact
  replay is idempotent, conflicting geometry is rejected, and failed spawn remains retryable.
- Immediate, headless, and test launches provide complete explicit geometry at creation. The Host
  never substitutes a visual startup timeout or an 80x24 fallback.
- Each terminal retains one complete runtime-theme state: operation identity, independent theme
  revision, color scheme, default foreground/background, nullable cursor color, and exactly 256
  palette entries. Apply uses expected-revision compare-and-set and changes the deferred launch
  preparation or started engine before one semantic commit. Exact replay is idempotent; stale,
  conflicting, invalid, exhausted, or failed operations preserve the prior complete state and
  recovery inventory returns that state without requiring a live attachment.
- A started runtime-theme replacement updates defaults and active colors under one Ghostty mutation
  boundary, clearing prior OSC dynamic-color overrides. Later OSC 4/10/11/12 changes may override
  it; OSC reset forms, RIS, and Host reset return to the latest runtime theme. A physical color
  change advances content revision once without changing generation, so every attachment projects
  the same coherent revision.
- Terminal output becomes a normal exit only after child completion, normal PTY drain, and complete quiescence of the PTY writer.
- Unrecoverable PTY read, PTY write, terminal-reply, resize, or worker-integrity failures become an explicit typed failed outcome. The Host fences unsafe I/O, contains the owned process tree, and continues bounded cleanup retries even while no broker is connected.
- Graceful termination, force termination, and disposal are distinct. Desktop owns escalation timing; disposal is explicit.
- Bounds and scheduler backpressure reject or defer the current operation without corrupting retained state. A full outgoing batch returns its committed prefix and is not a connection failure.
- Large projection and history records cross engine-to-egress ownership once: producers write into reserved response storage or transfer an owned envelope-ready allocation instead of copying a complete payload into a second buffer.
- Input whose reply is lost after admission has an unknown outcome and must not be automatically replayed.
- No client receives raw PTY output. Clients receive bounded protocol projections, history rows, selection state, exports, and lifecycle events derived from canonical terminal state.
- Kitty graphics remain bounded native state. Ghostty owns direct, regular-file, temporary-file,
  and supported POSIX shared-memory media plus animation, virtual/relative placement, cropping,
  query/delete, and signed-z composition. Host projects only owned decoded pixels and resolved
  placements through OBKG `0.0.2`; render-detail OBRW `0.0.3` includes a graphics snapshot, while
  text detail excludes graphics. macOS and supported libc-backed Linux targets enable every medium;
  Windows enables direct, regular-file, and temporary-file media but not POSIX shared memory.
- Terminal control queries, including DECRQSS, read that same canonical Ghostty state and enqueue their replies through the Host-owned bounded terminal-reply path; attachments never emulate a second VT reply surface.
- Attachment resize and terminal-driven 80/132-column requests share one native geometry commit
  owner. The Host correlates OS PTY size, Ghostty dimensions, cell pixels, revision, generation, Core
  observation, and the projected snapshot before clients observe the change.
- A running terminal has at most one attachment resize lease. A resize-capable observer can claim
  it explicitly; Host publishes the old owner's revocation before the new owner's grant and then
  acknowledges the claim. Detaching an owner promotes the remaining attachment with the newest
  successful claim, with earliest admission breaking ties among never-granted observers; an
  observer never has to reopen to become owner.
- Every attachment starts at capability revision `1`. A live input-capability update is accepted
  only at the exact next revision and is idempotent only for the exact current value. Raw and
  semantic PTY input must carry that current revision at preparation and final queue commit. A
  downgrade repairs only the target attachment's held key, mouse, and focus ownership before revoking
  input; an upgrade creates a fresh empty ownership ledger and never replays input.
- Focus is one terminal-wide OR aggregate over claims from attached, input-capable attachments with
  active projection demand. Only the first claim emits `FocusIn` and only the final release emits
  `FocusOut`; demand pause, detach, broker loss, and capability downgrade repair the target's key
  and mouse ownership before clearing its claim, while activation and upgrade require fresh input.
- Application clear and reset requests execute directly against the Host-owned Ghostty state for
  an input-capable attachment. Clear erases and homes the active screen without changing its modes;
  reset returns to a fresh primary-screen state, restores the latest runtime theme colors, and
  restores the retained profile's explicit cursor shape and blinking defaults. Terminal escapes may
  override those values until their reset form, RIS, or Host reset.
  Neither operation writes escape bytes to the child or asks a projected client to emulate VT
  behavior.
- A semantic key event may mark one macOS Option chord as terminal Alt. The Host applies that intent
  to Ghostty's authoritative encoder for the current terminal modes; it never treats the renderer's
  layout-produced special character as an already encoded terminal byte sequence.
- Each Ghostty bell advances one terminal-owned sequence. A read-capable attachment receives only
  later bells, once per committed event record; reconnect does not replay earlier bell intent.
- OSC 52 can publish one bounded plain-text write intent to one read-capable attachment, but never
  reads the user's clipboard or performs a platform effect. Outgoing backpressure retries the same
  intent, while successful handoff and reconnect cannot duplicate it.
- An unknown APC beginning with the exact private prefix `orchbee.agent.v3;` can publish one
  base64url-decoded, bounded application signal. Reader-batch admission is transactional, up to 64
  admitted signals remain in FIFO order, and outgoing backpressure preserves the exact queue head
  until commit. The Host does not authenticate the emitter, interpret Agent or provider state, or
  request a product or platform notification. The protocol record's literal channel
  `orchbee.agent.v1` is only a historical Host-to-Desktop discriminator, not a child wire sequence.
  Public notification channels such as OSC 9 and OSC 777 are ignored by this transport.
- Ghostty progress reports project only the terminal's current bounded local sample; they do not
  assert process completion or request a system notification.
- OSC 133 `C` and `D` actions transactionally retain only one terminal-local completion sequence,
  monotonic duration, wall-clock completion timestamp, and normalized exit code. The Host never
  retains or projects command text, arguments, directory, environment, or output for reminders;
  freshness, focus, permission, rate limiting, and notification effects remain client policy.
- A successful grid reflow preserves a tracked selection and committed history anchor when Ghostty's pins remain valid, while cancelling stale row flights and already encoded export bytes. Screen replacement and retained-row eviction remain explicit selection eviction boundaries.
- The public selection-export request and response admit only the literal `plain` format. Ghostty's
  internal formatter support for HTML and raw VT is not a Host protocol capability and cannot be
  selected by a client.

The complete state model, retry classes, commit boundaries, invariants, and evidence matrix live in the [runtime contract](docs/runtime-model.md).

## Trust and continuity boundary

The runtime directory and endpoint are private to the operating-system user. POSIX uses a trusted per-user rendezvous, a no-follow directory descriptor, effective-UID ownership, private mode, removal and validation of extended and inheritable ACLs, a lifetime lock, and post-bind validation of the actual Unix-domain socket. Host and Desktop both require the connected peer's effective UID through kernel credentials on every connection. Windows uses an owned non-reparse runtime directory, protected DACLs, a singleton mutex, a local named pipe, and peer-SID validation.

These controls establish a same-user trust boundary. The endpoint identifier is not a secret, and Terminal Host does not distinguish one same-user application from another. It is not a multi-tenant service.

Desktop reconnect is supported while the Host survives. On POSIX, Desktop's in-process native endpoint adapter owns the client socket and authenticates the Host before protocol work; Terminal Host independently authenticates every accepted client. Closing the Desktop connection does not terminate or otherwise own the Host. PTY migration or recovery after a Host or operating-system crash is not promised. The client-side model lives in the Desktop [POSIX Host endpoint contract](../desktop/docs/posix-host-endpoint.md).

Process-tree ownership follows the containment domain created at launch: a dedicated POSIX session or a Windows kill-on-close Job. Cleanup is complete only after that domain has no live members. On Windows, the Host waits on the Job's empty signal before closing HPCON; Desktop-owned force escalation terminates the Job and wakes that wait. Preventing a trusted same-user child from deliberately escaping a POSIX session is not an isolation promise.

## Executable interface

The installed binary is `orchbee-terminal-host` on POSIX and `orchbee-terminal-host.exe` on Windows. Its private launcher ABI is:

```text
orchbee-terminal-host serve \
  --runtime-directory <absolute-path> \
  --endpoint-id <32-lowercase-hex> \
  [--scrollback-bytes <decimal-bytes>]
```

Desktop derives the runtime directory and endpoint identifier from its product user-data scope; the Host treats both as infrastructure values and does not infer product topology from them.

The binary exposes no client connection or runtime-preparation command. Desktop owns those operations
inside its platform adapter, while Terminal Host remains a separately retained server process.

`--scrollback-bytes` sets the startup default for newly created terminals. The current default is 10,000,000 decimal bytes; accepted values range from `0` through `1,000,000,000`. A broker may replace the process default for later terminals or supply an explicit configuration when creating one. Zero disables retained scrollback.

`orchbee-terminal-host --version` prints the executable name and its embedded semantic version. The binary version identifies the packaged process and is independent from protocol and binary format versions.

## SSH Environment release component

The accepted schema-version-2 SSH Environment release reuses this executable unchanged as its
`terminal-host` component. This workspace owns the component build recipe; repository-level release
automation owns cross-application assembly, signing inputs, and final bundles. The exact remote
targets are `aarch64-macos` and `x86_64-linux-gnu`, built with Zig `0.16.0`, `ReleaseSafe`, stripping,
and `-Dhost-version=<semver>`.

After Desktop explicitly opens the accepted `terminal-host-v1` service, the remote SSH Runtime
connects to this executable's unchanged same-user private endpoint or starts the exact cohort-pinned
version, then relays opaque Terminal Host protocol bytes over the existing SSH attachment. Terminal
Host still owns native terminal identities, retained terminal state, PTYs, and process trees. The
component itself gains no network listener, SSH parsing, or generic command authority; remote
lifecycle and relay policy belong to Desktop and the SSH Runtime.

## Pinned native inputs

| Input                | Value                                                            |
| -------------------- | ---------------------------------------------------------------- |
| Zig                  | `0.16.0` exactly                                                 |
| Ghostty revision     | `9c3ec931d64561a8407dde7ac984ce156ae91539`                       |
| Ghostty package hash | `ghostty-1.3.2-dev-5UdBCzw0UwUUuHE16pHQg0d9Bplq2ccuQ2cmOJRYkd-y` |

`build.zig.zon` is the dependency authority. The build rejects another Zig version or Ghostty archive content and does not read a sibling Ghostty checkout.

## Development

The repository-level `pnpm desktop` development runtime runs `zig build --watch -fincremental
-Doptimize=ReleaseSafe` for this package and observes the completed Host binary and source inputs by
content digest. A failed build or a same-source binary rebuild leaves the last runnable Host process
and binary generation untouched. Zig code is not patched into a live Host process: after a
successful changed source build, Desktop durably stops and restages its current terminal
presentations, retires the old Host, and starts the captured binary as a new process. PTY process
state, VT history, and Host identities therefore do not cross a Zig source update. Packaged Host
lifecycle and protocol behavior are unchanged.

Run from the repository root:

```sh
pnpm --filter @orchbee/terminal-host format:check
pnpm --filter @orchbee/terminal-host build
pnpm --filter @orchbee/terminal-host test
pnpm --filter @orchbee/terminal-host test:black-box
pnpm --filter @orchbee/terminal-host test:protocol
pnpm --filter @orchbee/terminal-host test:surface-fixtures
pnpm --filter @orchbee/terminal-host test:tla
pnpm --filter @orchbee/terminal-host test:cross
pnpm verify
```

`test` runs the current platform's native tests, the real-process Local IPC/PTY black-box trace,
cross-language protocol conformance, and the pinned-Ghostty surface-fixture comparison.
The generated cross-language selection cases admit only `plain` and require HTML/raw VT requests to
fail strict decoding; `terminal-parity/HS-07-selection-export` adds the real-PTY/Electron clipboard
consequence from the owning application system suite.
`test:black-box` runs only that external trace: it starts the built Host, creates and attaches a
real terminal through Local IPC, disconnects, reconnects and replays the creation identity,
reattaches the same attachment identity, drives the child through PTY input, observes drained PTY
output in the final revision and a successful exit event, disposes the retained terminal, and
requires the Host process to stop cleanly. `test:protocol` runs protocol conformance alone;
`test:surface-fixtures` regenerates and byte-compares three Host-owned native fixtures in a
temporary directory. The primary/alternate OBTF pair comes from one Ghostty terminal and Projection
across a real alternate-screen transition. A separate Ghostty terminal replays the rich primary
corpus, adds DECSCA protected and unprotected sentinels, and reaches the render-detail OBRW fixture
only through `row_encoding.encode` and `projection_buffer.encodeRenderSnapshot`. The protocol
package owns the tracked bytes and exports; `@orchbee/terminal` owns the exact strict-view,
trusted-view, and packed-store semantic oracle. This evidence covers the deterministic native
projection mapping, not PTY delivery, history anchoring, or authorization of another trust path.
After an intentional reviewed corpus or encoder change, run
`pnpm --filter @orchbee/terminal-host surface-fixtures:generate` and review all producer and
consumer assertions. `test:cross` compiles the complete test graph, including the
black-box runner, for Linux, FreeBSD, macOS, and Windows; cross-compilation is not runtime evidence
for those platforms. Repository CI runs the package `test` command, and therefore the external Host
trace, natively on Linux, macOS, and Windows. FreeBSD remains compile-only evidence and is not
reported as native PTY or IPC conformance.

`format:check` also enforces the package boundary rule that every hand-written production Zig file
under `src/` stays at or below 500 physical lines. Scenario catalogs under `__tests__/` and
`*_test.zig` files are exempt.

The narrower POSIX endpoint-security evidence can be run without the complete Host suite:

```sh
(cd apps/terminal-host && zig build test-cli)
(cd apps/terminal-host && zig build test-transport)
(cd apps/terminal-host && zig build test-transport-compile -Dtarget=x86_64-linux-gnu)
(cd apps/terminal-host && zig build test-transport-compile -Dtarget=x86_64-freebsd)
```

The native macOS run exercises real libc ACL inheritance and `getpeereid`; the cross-target steps compile the Linux `SO_PEERCRED` and FreeBSD `getpeereid` branches but are not runtime evidence for those systems.

`test:tla` model-checks the Host-owned registry, per-terminal lifecycle, multi-runtime shutdown,
delivery, and request-outcome abstractions through the repository runner. It is change-triggered
correctness evidence and is intentionally not part of the ordinary `test` gate. The runner uses
`java` from `PATH` by default; set `ORCHBEE_TLA_JAVA` to an explicit Java executable or
`ORCHBEE_TLA_TIMEOUT_MILLISECONDS` to a positive per-check timeout override for a slower verified
environment.

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

Source modules should follow the semantic owners in the runtime contract: local IPC transports records, the native protocol adapter translates records, the Host registry owns terminal membership, each retained terminal record binds one semantic session to its deferred launch preparation or started engine, each session owns its lifecycle, and each broker session owns its attachments. A file split is useful only when it makes one of those ownership boundaries clearer.

The package-local [module boundary contract](docs/module-boundaries.md) maps those owners to source
directories, named Zig modules, allowed imports, and test placement.
