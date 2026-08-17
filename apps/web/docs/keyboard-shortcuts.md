# Keyboard shortcuts

`@orchbee/web` owns client-local overrides for eight frequent semantic shortcuts. Workbench and
projected-terminal features continue to own their Action inputs and platform defaults. The Web
composition combines those declarations, validates one complete effective table, and injects that
same source into the mounted Workbench and projected terminals.

## Initial catalog

| Shortcut ID                     | Scope                             | Apple default | Standard default |
| ------------------------------- | --------------------------------- | ------------- | ---------------- |
| `workbench.search.actions.open` | Workbench                         | `Cmd+Shift+P` | `Ctrl+Shift+P`   |
| `workbench.terminal.create`     | Workbench                         | `Cmd+N`       | `Ctrl+Shift+N`   |
| `workbench.search.all.open`     | Workbench                         | `Cmd+K`       | `Ctrl+Shift+K`   |
| `workbench.settings.open`       | Workbench                         | `Cmd+,`       | `Ctrl+,`         |
| `workbench.sidebar.toggle`      | Workbench                         | `Cmd+B`       | `Ctrl+Shift+B`   |
| `terminal.search.open`          | Focused terminal or Find          | `Cmd+F`       | `Ctrl+Shift+F`   |
| `terminal.selection.copy`       | Focused terminal with a selection | `Cmd+C`       | `Ctrl+Shift+C`   |
| `terminal.selection.select-all` | Focused terminal                  | `Cmd+A`       | `Ctrl+Shift+A`   |

Standard defaults deliberately leave unshifted `Ctrl+B/N/K/F/A` and `Ctrl+C` available to the
shell, terminal applications, and multiplexers. A user may deliberately record another supported
custom chord. Product conflicts remain prohibited even when an Action is temporarily unavailable.

## Chord and conflict model

`@orchbee/keyboard-shortcuts` provides only the pure physical-chord mechanism. A chord contains one
supported `KeyboardEvent.code` and a normalized exact modifier set. `Mod` maps to Command on Apple
and Control on Standard platforms. Matching rejects extra modifiers, composition (including legacy
key code 229), repeats, unsupported codes, and modifier-only events. Formatting and conflict
identity come from the same normalized chord.

All eight initial declarations can overlap: a Workbench shortcut and the shortcut for its focused
terminal can both be applicable. Their effective chords must therefore be globally unique. A
conflicting edit changes neither memory nor storage and identifies the conflicting Action, scope,
binding source, and availability condition. During dispatch, the owning Action adapter resolves
current availability before execution or `preventDefault()`; an unavailable binding falls through
to the terminal or browser.

## Persistence and recovery

Overrides use `orchbee:web:keyboard-shortcuts:v1` and a strict record:

```json
{
	"version": 1,
	"overrides": [
		{
			"shortcutId": "workbench.search.all.open",
			"chord": { "code": "KeyG", "modifiers": ["mod", "shift"] }
		}
	]
}
```

Only Custom values are stored, ordered by shortcut ID. Unknown fields or IDs, duplicates, unsorted
entries, noncanonical modifiers, unsupported codes, stored defaults, excessive entries, and an
effective conflict invalidate the complete record and restore platform defaults. A write failure
keeps the immutable session binding active, reports that it was not saved, and retries on the next
legal change. Restoring one item removes its override.

Browser Preview, Gateway Browser, and Electron each create and dispose one controller against their
own origin-local storage. Gateway and Electron pass the controller's narrow binding source through
`ProjectedTerminalSurfaceCapability`. Event handlers read the current table at dispatch time, so a
change does not remount Workbench, replace an attachment, or reopen a Host or Gateway connection.

## Maintenance checks

```sh
pnpm --filter @orchbee/keyboard-shortcuts typecheck
pnpm --filter @orchbee/keyboard-shortcuts test
pnpm --filter @orchbee/workbench test
pnpm --filter @orchbee/web typecheck
pnpm --filter @orchbee/web test
```
