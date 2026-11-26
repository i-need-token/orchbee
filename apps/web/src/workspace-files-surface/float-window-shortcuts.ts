import {
	formatKeyboardShortcutChord,
	matchesKeyboardShortcutChord,
	type KeyboardShortcutChord,
	type KeyboardShortcutEvent,
	type KeyboardShortcutPlatform,
} from '@orchbee/keyboard-shortcuts';

/**
 * The float window's own bindings. They apply only while the keyboard is inside the window, so a
 * terminal keeps every key it is given, including Ctrl+S for flow control.
 */
const FLOAT_WINDOW_SHORTCUTS = {
	/** Saves the file the active tab edits, as an editor does. */
	save: { code: 'KeyS', modifiers: ['mod'] },
} as const satisfies Readonly<Record<string, KeyboardShortcutChord>>;

type FloatWindowShortcut = keyof typeof FLOAT_WINDOW_SHORTCUTS;

export function matchesFloatWindowShortcut(
	event: KeyboardShortcutEvent,
	shortcut: FloatWindowShortcut,
	platform: KeyboardShortcutPlatform,
): boolean {
	return matchesKeyboardShortcutChord(event, FLOAT_WINDOW_SHORTCUTS[shortcut], platform);
}

export function formatFloatWindowShortcut(
	shortcut: FloatWindowShortcut,
	platform: KeyboardShortcutPlatform,
): string {
	return formatKeyboardShortcutChord(FLOAT_WINDOW_SHORTCUTS[shortcut], platform);
}
