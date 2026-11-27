# @orchbee/keyboard-shortcuts

Private package entries export only symbols consumed in this repository, enforced by Knip `includeEntryExports`. Internal contracts, generated assets and independent conformance evidence retain their owners; their presence does not automatically expand the public entry. New clients add exports for admitted uses.

Pure keyboard-chord mechanics shared by Orchbee product owners.

The package normalizes a single physical `KeyboardEvent.code` plus an exact modifier set, maps the
engineering modifier `Mod` to Meta on Apple platforms and Control elsewhere, captures supported
events, compares platform-effective identities, matches events, and derives platform display text.
It does not own product Actions, default bindings, context conditions, conflict policy,
persistence, UI, platform detection, or event-listener lifecycle.

Product features continue to define their default bindings once in their owning
`*-shortcuts.ts` module. An application may persist validated overrides and use
`keyboardShortcutChordKey(...)` with its own scope-overlap policy to diagnose conflicts.

A chord's admission rule depends on where its owner binds it:

| Rule          | Admits                                                                     |
| ------------- | -------------------------------------------------------------------------- |
| `window`      | At least one modifier other than Shift; the default everywhere             |
| `region`      | As `window`, but a function key may also be unmodified or Shift-only       |
| `widget`      | Also unmodified and Shift-only keys, and Escape as a declared default only |
| `second-step` | Also unmodified and Shift-only keys, as the step after `Mod+K`; no Escape  |

No rule admits Tab, and capture never records Escape, so Escape and Tab keep cancelling recording
and moving focus. Text entry, modifier-only events, input-method composition, and repeated keydown
events stay outside matching and capture. A widget rule exists for tree and list keys such as F2 or
Delete; the owner, not this package, decides that focus is on the widget and not in a text field.

A binding is one chord or a two-step sequence whose first step is
`KEYBOARD_SHORTCUT_SEQUENCE_LEADER` (`Mod+K`). `normalizeKeyboardShortcutSequence(...)` validates a
lone chord under the caller's rule and a second step under `second-step`, and
`keyboardShortcutSequenceKey(...)` gives a sequence the platform-effective identity that
`keyboardShortcutChordKey(...)` gives a chord. The package formats sequences one platform chord per
step (`⌘K ⌘W`, `Ctrl+K Ctrl+W`) and formats a raw press for a key that names nothing; waiting for
the second step, its timeout, and where sequences apply belong to the resolving application.

Apple display text follows the platform menu order Control, Option, Shift, Command (`⌃⌥⇧⌘`);
non-Apple display text uses named modifiers separated by `+`.

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
