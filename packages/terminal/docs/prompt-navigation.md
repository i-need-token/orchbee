# Prompt navigation

## Scope

On the primary screen, previous/next navigation jumps from the current visible top to the nearest strict `PROMPT_START` row in the requested direction. Defaults are `Cmd+Up` and `Cmd+Down` on Apple and `Ctrl+Up` and `Ctrl+Down` elsewhere. Durable prompt memory, configurable prompt counts, scroll offsets, pixel scrolling, output pagers, automatic shell-integration injection and alternate-screen navigation are excluded.

## Domain contract

A terminal user with OSC 133 shell integration needs to move between prior command boundaries
without searching or paging manually. `ProjectedTerminal` owns this local history-navigation
workflow. Terminal Host remains authoritative for retained rows, generation and revision, and the
OSC 133-derived row `promptClass`; the browser never reparses escape sequences or infers prompts
from text.

The operation consumes:

- direction `previous` or `next`;
- the main screen's current absolute visible-start row as an exclusive anchor, or a same-intent
  transient requested visible-start anchor while that physical viewport's revision is catching up;
- one generation/revision/total-row snapshot;
- the current local history-intent epoch;
- an abort signal and the existing general-buffer row-reader capability.

It returns `target`, `cancelled`, or an intentional `unchanged` reason. A target is committed only
when the screen is still primary, the pinned generation, revision, and total rows remain current,
the local history-intent epoch still matches, and the selected anchor authority has not changed. A
visible anchor requires an exact visible/current Host snapshot match. A transient anchor requires
the same local intent epoch, generation, total rows, and current viewport bounds; every invocation
still scans the latest Host revision. The transient value is only revision-lag repair and is never
used instead of a current physical viewport, so it does not implement remembered-last-prompt
state. The commit asks the existing retained-history viewport owner to reveal the target row; it does
not mutate Host history or prompt semantics.

## Decisions and invariants

| Decision        | Previous                                                                          | Next                                                                              |
| --------------- | --------------------------------------------------------------------------------- | --------------------------------------------------------------------------------- |
| Search domain   | Rows strictly less than the visible-start anchor                                  | Rows strictly greater than the visible-start anchor                               |
| Order           | Newest page first, rows descending                                                | Oldest page first, rows ascending                                                 |
| Eligible row    | `promptClass === prompt`                                                          | `promptClass === prompt`                                                          |
| Target          | First eligible row encountered                                                    | First eligible row encountered                                                    |
| Viewport result | Target becomes the requested visible top, clamped by the existing bottom boundary | Target becomes the requested visible top, clamped by the existing bottom boundary |

The workflow preserves these invariants:

- prompt continuations and unmarked rows are never navigation targets;
- alternate-screen, empty, boundary, no-match, source-limit, unavailable, malformed, stale, and
  cancelled cases do not change the viewport;
- one scan has at most one general-buffer row request in flight and never occupies the independent
  viewport or search lanes;
- every page is generation/revision pinned, contiguous, and bounded by protocol row, cell, and byte
  limits;
- the complete scan admits at most 16 MiB of encoded row-window source, with each request capped by
  the remaining budget;
- replacement, another history intent, disposal, or a newer Host snapshot during a scan retires
  commit authority;
- a successful scan publishes at most one viewport intent and never commits a target from an older
  invocation.

The deterministic locator requests at most 1,024 rows per page, additionally capped by the current
column count and the protocol cell limit. A `too-large` page is retried with half as many rows while
preserving nearest-first order. A single row that cannot fit the remaining source budget ends as
`source-limit`; the operation never skips an uninspected gap.

## Actions and shortcuts

Workbench owns the semantic Actions `terminal.history.prompt-previous` and
`terminal.history.prompt-next`. Web registers them with replace-latest history-navigation
execution and exposes terminal-focused shortcuts. The defaults are `Mod+Up` and `Mod+Down`, Command
on Apple and Control on Standard, as in VS Code's terminal. The binding is consumed only while the Action is available on a focused primary-screen
terminal. During a transient viewport flight or Host history refresh, an invocation uses the
current physical anchor when its snapshot is exact. If only the physical revision is lagging, it may
instead rescan the latest revision from the same-intent transient anchor while generation, total
rows, viewport bounds, and history-intent epoch still match. Otherwise it completes as an
unannounced no-op. The product chord never leaks into the PTY. An execution that finds no target is
likewise unchanged and silent.

Adding the two shortcut IDs extends Web's strict durable keyboard catalog. Version 6 migrates an
exact valid v5 record into the expanded catalog, removes overrides equal to current defaults,
rejects migration conflicts, and preserves v5 as the rollback snapshot. Invalid current records do
not fall back to older versions.

## Workflow and cancellation

| State        | Event                                                                      | Guard                                                                                            | Result                                                                                 |
| ------------ | -------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------- |
| Ready        | Invoke                                                                     | Primary screen, readable non-empty domain, and a current visible or same-intent transient anchor | Capture snapshot, anchor authority, and history-intent epoch; request the nearest page |
| Reading      | Page contains a prompt                                                     | Page and captured authority are current                                                          | Return the nearest strict target                                                       |
| Reading      | Page contains no prompt                                                    | More domain and source budget remain                                                             | Advance outward without overlap or gaps                                                |
| Reading      | Page is too large                                                          | More than one row was requested                                                                  | Halve the page while retaining the same nearest boundary                               |
| Reading      | Abort, stale source, malformed page, lane unavailable, or exhausted budget | Any                                                                                              | Return cancelled or unchanged without a viewport intent                                |
| Target ready | Commit                                                                     | Snapshot, selected anchor authority, epoch, and primary screen are still current                 | Publish exactly one immediate viewport intent                                          |
| Target ready | Replacement or authority change                                            | Commit guard fails                                                                               | Return cancelled or unchanged without mutation                                         |

The general-buffer lane's existing abort listener settles a replaced read. Action cancellation is
observable only as a cancelled Action result; boundary and unavailable outcomes remain silent so a
keyboard navigation attempt does not produce status chatter.

## Verification

| Claim                                                                                      | Evidence                                                              | Limit                                                                           |
| ------------------------------------------------------------------------------------------ | --------------------------------------------------------------------- | ------------------------------------------------------------------------------- |
| Direction, strict anchor, nearest target, prompt-only eligibility, paging, and byte bounds | `src/projected/__tests__/projected-prompt-navigation.unit.test.ts`    | Deterministic mocked row-reader pages                                           |
| Stale, replacement, cancellation, local-history-intent, and single-commit safety           | controlled implementation tests plus `ProjectedTerminal` unit tests   | Controlled traces and synthetic adapter state; no exhaustive interleaving proof |
| Action availability, replace-latest execution, no-op, and cancellation mapping             | Web projected-terminal Action unit tests                              | In-process Action runtime                                                       |
| Physical defaults, conflict freedom, customization, and v5-to-v6 migration                 | Web shortcut declaration, catalog, controller, and storage unit tests | Synthetic keyboard events and storage                                           |

The `terminal-parity/SI-02-prompt-navigation` trace through a real Desktop attachment and Terminal
Host retired with the Desktop Electron suite.
