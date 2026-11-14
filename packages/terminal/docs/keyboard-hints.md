# Keyboard hints

## Scope

One semantic Action selects visible URLs, paths, file locations and hashes using keyboard labels and prefix filtering. It supports cancellation, URL opening and exact-text insertion. Configurable matchers, custom processors, multiple selection, mouse selection, clipboard modes, editor launch and arbitrary programs are excluded.

## Domain contract

`ProjectedTerminal` owns one hint session over the mounted visible viewport. A session is available
only while the terminal can accept input and has a live rendered surface. It reads one
generation/revision-pinned visible row window through the existing bounded row API, excludes a
soft-wrapped logical line clipped by either viewport edge, and detects at most 512 targets in
screen order:

- permitted `http`, `https`, and `mailto` URLs, including admitted explicit hyperlinks;
- detected or explicit paths and filenames;
- detected `path:line` and `path:line:column` locations; and
- lowercase hexadecimal hashes containing 7 through 64 characters.

URL and file matches use the shared terminal-link detector, so explicit metadata and longer file
locations retain precedence and a hash-shaped substring cannot duplicate another admitted target.
Labels are equal-width, prefix-free base-36 strings derived from screen order. Detection does not
read beyond the current viewport, synchronously inspect the whole buffer, test filesystem
existence, or grant the terminal package an external-effect capability.

## Session and effect model

The session transitions from `loading` to either `active`, `empty`, `stale`, `unavailable`, or
`cancelled`. While active, it owns key-down and key-up delivery so label input never reaches the
PTY. Characters narrow the visible prefix, Backspace removes one prefix character, Escape cancels,
and Enter or Space selects the sole remaining target. IME composition cancels without consuming
the composition event.

A frame or row revision, viewport transition, resize, presentation-geometry change, focus loss,
surface release, disposal, replacement invocation, or caller cancellation retires the session.
The selected result alone crosses into the application-owned Action boundary:

| Target kind               | Effect                                                                 |
| ------------------------- | ---------------------------------------------------------------------- |
| Permitted URL             | Normalize again and request the application-owned external URL opener  |
| Path, file location, hash | Insert the exact visible match through the ordinary terminal paste API |

Insertion never appends Enter and does not replace the visible spelling with the path detector's
resolved absolute target. An admitted effect that reports success crosses the Action commit point;
a later replacement cannot relabel that completed effect as cancelled.

## Action, binding, and persistence

Workbench owns `terminal.hints.open`, named Quick Select in the product; Web registers the
projected-terminal implementation and projects it into Action search, shortcut Settings, and
localized result feedback. The terminal context menu does not offer it. Apple and Standard platforms default to `Mod+Shift+O`. Standard `Ctrl+Shift+E` already owns Split Below. The catalog admits one effective binding per Action.

Adding this Action extends Web's strict device-local shortcut record from version 8 to version 9.
The version-9 reader migrates an exact v8 catalog and defaults, rejects a new-default conflict,
writes only the canonical v9 record, and preserves v8 unchanged as a rollback snapshot.

## Verification

| Claim                                                               | Evidence                                                            | Limit                                                            |
| ------------------------------------------------------------------- | ------------------------------------------------------------------- | ---------------------------------------------------------------- |
| Detection, overlap precedence, clipping, bounds, and prefix labels  | `src/__tests__/terminal-hints.unit.test.ts`                         | Synthetic rendered rows                                          |
| Pinned visible read, overlay ownership, selection, and invalidation | `src/projected/__tests__/projected-terminal.unit.test.ts`           | Deterministic projected terminal and mocked Host row window      |
| Effects, cancellation, commit, binding, labels, and v8 migration    | Web Action, shortcut, Settings, i18n, controller, and storage tests | In-process effects, keyboard events, and device-local storage    |
| Four target kinds work through the production Canvas surface        | `terminal-parity/FS-07-keyboard-hints`                              | Headed Chromium with synthetic pinned rows and a captured opener |
