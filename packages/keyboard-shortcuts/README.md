# @orchbee/keyboard-shortcuts

Pure keyboard-chord mechanics shared by Orchbee product owners.

The package normalizes a single physical `KeyboardEvent.code` plus an exact modifier set, maps the
engineering modifier `Mod` to Meta on Apple platforms and Control elsewhere, captures supported
events, compares platform-effective identities, matches events, and derives platform display text.
It does not own product Actions, default bindings, context conditions, conflict policy,
persistence, UI, platform detection, or event-listener lifecycle.

Product features continue to define their default bindings once in their owning
`*-shortcuts.ts` module. An application may persist validated overrides and use
`keyboardShortcutChordKey(...)` with its own scope-overlap policy to diagnose conflicts.

Only single-step physical chords with at least one non-Shift modifier are supported. Text entry,
unmodified typing, modifier-only events, Escape, Tab, input-method composition, and repeated
keydown events are deliberately outside the capture contract.

```ts
import {
	captureKeyboardShortcutChord,
	formatKeyboardShortcutChord,
	matchesKeyboardShortcutChord,
} from '@orchbee/keyboard-shortcuts';

const result = captureKeyboardShortcutChord(event, 'standard');
if (result.kind === 'captured') {
	console.log(formatKeyboardShortcutChord(result.chord, 'standard'));
}

if (matchesKeyboardShortcutChord(event, chord, 'standard')) {
	// Resolve the owning semantic Action and consume only when it is available.
}
```

```sh
pnpm --filter @orchbee/keyboard-shortcuts typecheck
pnpm --filter @orchbee/keyboard-shortcuts test
pnpm --filter @orchbee/keyboard-shortcuts build
pnpm --filter @orchbee/keyboard-shortcuts test:exports
```
