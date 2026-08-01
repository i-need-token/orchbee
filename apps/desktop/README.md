# Desktop

Sandboxed Electron client and scoped local broker for Orchbee's native Terminal Host.

## Current scope

Slice 5 mounts one app-local `ProjectedTerminal` and proves the real Host → Electron main →
preload → renderer path. Electron main owns the Host socket, record assembly, process launch, and
attachment routing. Preload forwards one allowlisted `MessagePort`; it exposes no generic IPC,
filesystem, executable, terminal-enumeration, creation, termination, or disposal capability. The
renderer runs sandboxed with context isolation and without Node integration.

The application intentionally does not import Workbench or model Project, Workspace, Tab, Panel,
history, updates, sharing, or network clients. Product composition and recovery remain in their
later accepted slices. Closing or reloading the renderer detaches its attachment only; it never
terminates or disposes the Host terminal.

Main validates and routes packed Host frames without VT parsing, row reconstruction, JSON/base64,
or per-cell work. Electron structured-clones each frame once from Main to Renderer; the route does
not claim zero-copy `ArrayBuffer` ownership transfer. Host projection backpressure still permits at
most one frame in flight for the attachment.

The Slice 5 verification surface uses `Menlo` at 15 points with a generic monospace fallback. This
is a controlled benchmark default, not the future product font-preference model.

## Development

Build the explicitly selected development Host first, then start Desktop:

```sh
pnpm --filter @orchbee/terminal-host build
pnpm desktop
```

`ORCHBEE_TERMINAL_HOST_BINARY` may select another absolute development/test Host binary.
`ORCHBEE_DESKTOP_RUNTIME_DIRECTORY` and `ORCHBEE_DESKTOP_ENDPOINT_ID` exist only for controlled
Slice 5 development and system tests. They are not the versioned installation or discovery design.
The default POSIX development runtime uses a short, per-home private directory under `/tmp` so its
Unix-domain socket stays inside platform path limits; the Host still enforces owner-only access.

Run focused checks from the repository root:

```sh
pnpm --filter @orchbee/desktop typecheck
pnpm --filter @orchbee/desktop test
pnpm --filter @orchbee/desktop build
pnpm --filter @orchbee/desktop test:system
```

Run the controlled throughput harness only after building Desktop and the Host. It requires at
least five Electron runs and accepts separately measured, geometry-matched Ghostty and Host-only
runs for comparison:

```sh
pnpm --filter @orchbee/desktop benchmark -- \
  --file /absolute/path/to/large-output.txt \
  --runs 5 \
	--measure-ghostty \
  --host-only-runs 18.4,18.5,18.3,18.6,18.4
```

On macOS, `--measure-ghostty` launches five isolated Ghostty surfaces with default config files
disabled and pins their grid, Menlo 15-point font, and 10,000,000-byte scrollback to the geometry
reported by Desktop. After the first Desktop run establishes geometry, Desktop and Ghostty order
alternates within paired samples so sustained-load or thermal drift is not assigned to only one
implementation. `--ghostty-app` may select another absolute `.app` path. Alternatively,
`--ghostty-runs` accepts an already controlled five-run reference; do not use both options.

The timer starts when Enter is sent and stops only after the final revision has produced renderer
frame telemetry. Each run reports Host protocol bytes/frame count, Main routing, renderer
commit/render counts and backend health, plus idle Electron-process and Host CPU/RSS observations.
The Electron CPU sample is primed and then measured after a one-second idle window; it is not the
first `getAppMetrics()` call, whose interval CPU values are zero by definition.
The summary reports median, median absolute deviation, incremental Host-to-Electron cost, and the
accepted `1.15 × Ghostty` target. That target applies only to fixtures of at least 500,000,000
bytes; smaller smoke fixtures still report measurements but cannot pass or fail the target. Omit
reference run lists when establishing raw local baselines; the harness then makes no comparative
pass claim.

The system test requires the Host build above. Raw benchmark inputs, traces, and screenshots stay
untracked.
