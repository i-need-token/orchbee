# Desktop

Sandboxed Electron client and scoped local broker for Orchbee's native Terminal Host.

## Current scope

Slice 6 mounts one app-local `ProjectedTerminal` and binds live projection, bounded history,
Host-owned selection, client-side find, copy, and approved-link activation through the real Host →
Electron main → preload → renderer path. Electron main owns the Host socket, record assembly,
process launch, attachment routing, platform clipboard write, and external-navigation policy.
Preload forwards one allowlisted `MessagePort`; it exposes no generic IPC, filesystem, executable,
terminal-enumeration, creation, termination, or disposal capability. The renderer runs sandboxed
with context isolation and without Node integration.

The application intentionally does not import Workbench or model Project, Workspace, Tab, Panel,
updates, sharing, or network clients. Product composition and recovery remain in their later
accepted slices. Closing or reloading the renderer detaches its attachment only; it never
terminates or disposes the Host terminal.

Main validates and routes packed Host frames and row windows without VT parsing, row
reconstruction, JSON/base64, or per-cell work. Electron structured-clones each binary projection
once from Main to Renderer; the route does not claim zero-copy `ArrayBuffer` ownership transfer.
Host projection backpressure still permits at most one frame in flight for the attachment, and the
renderer issues at most one bounded row read at a time.
Format-1.1 text windows use an implicit mapping for verified narrow ASCII rows, so search does not
transport or materialize a 12-byte mapping record per character. Unicode and non-identity rows keep
their explicit cell mappings.

Normal bottom-following use creates neither a history cache nor a search Worker. Entering history
pauses live projection work, commits one contiguous packed window with bounded overscan, and falls
back to the exact viewport if overscan exceeds the negotiated 2 MiB row-window limit. Find lazily
streams text-detail rows through a source-neutral reader into one disposable Worker; the query
never enters Main or Host. The Host remains the selection authority. Main assembles at most one
bounded selection export, writes it to the platform clipboard, and reports only completion metadata
to the renderer. Main opens only `http`, `https`, and `mailto` URL targets; file and unknown targets
remain unavailable.

One app-owned Actions runtime binds find, navigation, select-all, copy, and link activation. The
private canonical find surface owns its query history and localized UI in `en-US` and `zh-CN`.
`Command+A`/`Command+C` are desktop selection actions on macOS; non-Apple clients keep `Ctrl+A` and
`Ctrl+C` available to the PTY and use `Ctrl+Shift+C` for copy.

The controlled verification surface uses `Menlo` at 15 points with a generic monospace fallback. This
is a controlled benchmark default, not the future product font-preference model.

## Development

Build the explicitly selected development Host first, then start Desktop:

```sh
pnpm --filter @orchbee/terminal-host build
pnpm desktop
```

`ORCHBEE_TERMINAL_HOST_BINARY` may select another absolute development/test Host binary.
`ORCHBEE_DESKTOP_RUNTIME_DIRECTORY` and `ORCHBEE_DESKTOP_ENDPOINT_ID` exist only for controlled
foundation development and system tests. They are not the versioned installation or discovery design.
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
  --slice5-median 28.351 \
  --host-only-runs 18.4,18.5,18.3,18.6,18.4
```

On macOS, `--measure-ghostty` launches five isolated Ghostty surfaces with default config files
disabled and pins their grid, Menlo 15-point font, and 10,000,000-byte scrollback to the geometry
reported by Desktop. After the first Desktop run establishes geometry, Desktop and Ghostty order
alternates within paired samples so sustained-load or thermal drift is not assigned to only one
implementation. `--ghostty-app` may select another absolute `.app` path. Alternatively,
`--ghostty-runs` accepts an already controlled five-run reference; do not use both options.
`--slice5-median` supplies the same-machine Slice 5 baseline and activates its no-more-than-5%
regression gate. The harness executes `apps/terminal-host/zig-out/bin/terminal-host`, so rerun the
Host `build` command after native source changes; the focused Host `benchmark` target does not
replace that service binary.

The timer starts when Enter is sent and stops only after the final revision has produced renderer
frame telemetry. Each run reports Host protocol bytes/frame count, Main routing, renderer
commit/render counts and backend health, plus idle Electron-process and Host CPU/RSS observations.
After command completion it measures one exact `113 × 38` history draw, a retained-content literal
search, and select-all/copy through the platform clipboard. Renderer event timestamps exclude
automation round trips; `PerformanceObserver` records long tasks, Host RSS is sampled every 20 ms
during copy, and Electron reports Main's peak working set. The active gates are history p95 at most
50 ms, search p95 at most 750 ms, copy broker p95 at most 1,000 ms, no renderer task of at least
50 ms, and one lazily created Worker that is terminated after search.
The Electron CPU sample is primed and then measured after a one-second idle window; it is not the
first `getAppMetrics()` call, whose interval CPU values are zero by definition.
The summary reports median, median absolute deviation, incremental Host-to-Electron cost, and the
accepted `1.15 × Ghostty` and `1.05 × Slice 5` targets. Comparative and active targets apply only to
fixtures of at least 500,000,000 bytes on macOS; smaller smoke fixtures still report measurements
but cannot pass or fail those targets. Idle evidence also requires zero row-window, selection-export,
clipboard, and search-Worker work before the feature interactions. Omit reference run lists when
establishing raw local baselines; the harness then makes no comparative pass claim.

The system test requires the Host build above. Raw benchmark inputs, traces, and screenshots stay
untracked.
