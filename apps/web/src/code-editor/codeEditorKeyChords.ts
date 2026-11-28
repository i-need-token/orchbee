import {
	normalizeKeyboardShortcutChord,
	type KeyboardShortcutChord,
	type KeyboardShortcutPlatform,
} from '@orchbee/keyboard-shortcuts';

/**
 * The single keys Monaco binds by default in the editor as `monaco-entry.ts` composes it, at the
 * pinned version, by Monaco's own operating system. On Apple, `mod` is Command and `control` is
 * Control; elsewhere `mod` is Control. A key whose every binding needs something this composition
 * never shows is left out: Monaco's quick input, the code action menu, reference search, a
 * definition, implementation or reference provider, or the marker and symbol navigation that only
 * those start. Tab and Shift+Tab are left out too, as the keyboard model names Tab only with
 * Control, and no wider binding holds them. Monaco binds no `Mod+K` sequence of its own here: the
 * Workbench owns them, and its scoped shortcuts run their second steps through the editor.
 *
 * The table comes from Monaco's default keybinding registry, read once for each operating system
 * with the pinned package; a new Monaco version reads it again.
 */
const SHARED = [
	'ArrowDown',
	'ArrowLeft',
	'ArrowRight',
	'ArrowUp',
	'Backspace',
	'Delete',
	'End',
	'Enter',
	'Escape',
	'F3',
	'F7',
	'F8',
	'Home',
	'PageDown',
	'PageUp',
	'alt+ArrowDown',
	'alt+ArrowUp',
	'alt+Enter',
	'alt+F8',
	'alt+shift+ArrowDown',
	'alt+shift+ArrowLeft',
	'alt+shift+ArrowRight',
	'alt+shift+ArrowUp',
	'alt+shift+F8',
	'alt+shift+KeyI',
	'mod+ArrowDown',
	'mod+ArrowLeft',
	'mod+ArrowRight',
	'mod+ArrowUp',
	'mod+Backspace',
	'mod+BracketLeft',
	'mod+BracketRight',
	'mod+Delete',
	'mod+Enter',
	'mod+F2',
	'mod+F3',
	'mod+KeyA',
	'mod+KeyD',
	'mod+KeyF',
	'mod+KeyL',
	'mod+KeyU',
	'mod+KeyZ',
	'mod+Slash',
	'mod+alt+Backspace',
	'mod+alt+Enter',
	'mod+alt+shift+ArrowDown',
	'mod+alt+shift+ArrowUp',
	'mod+shift+ArrowDown',
	'mod+shift+ArrowLeft',
	'mod+shift+ArrowRight',
	'mod+shift+ArrowUp',
	'mod+shift+Backslash',
	'mod+shift+Comma',
	'mod+shift+Digit1',
	'mod+shift+Enter',
	'mod+shift+F3',
	'mod+shift+KeyK',
	'mod+shift+KeyL',
	'mod+shift+KeyZ',
	'mod+shift+Period',
	'shift+ArrowDown',
	'shift+ArrowLeft',
	'shift+ArrowRight',
	'shift+ArrowUp',
	'shift+Backspace',
	'shift+End',
	'shift+Enter',
	'shift+Escape',
	'shift+F3',
	'shift+F7',
	'shift+F8',
	'shift+Home',
	'shift+PageDown',
	'shift+PageUp',
] as const;

const MAC = [
	'alt+ArrowLeft',
	'alt+ArrowRight',
	'alt+Backspace',
	'alt+Delete',
	'alt+shift+KeyA',
	'control+Backspace',
	'control+Delete',
	'control+KeyA',
	'control+KeyB',
	'control+KeyD',
	'control+KeyE',
	'control+KeyF',
	'control+KeyH',
	'control+KeyJ',
	'control+KeyK',
	'control+KeyN',
	'control+KeyO',
	'control+KeyP',
	'control+KeyT',
	'control+PageDown',
	'control+PageUp',
	'control+alt+ArrowLeft',
	'control+alt+ArrowRight',
	'control+alt+Backspace',
	'control+alt+Delete',
	'control+alt+shift+ArrowLeft',
	'control+alt+shift+ArrowRight',
	'control+shift+ArrowLeft',
	'control+shift+ArrowRight',
	'control+shift+KeyA',
	'control+shift+KeyE',
	'control+shift+KeyM',
	'mod+KeyE',
	'mod+KeyG',
	'mod+PageDown',
	'mod+PageUp',
	'mod+alt+ArrowDown',
	'mod+alt+ArrowUp',
	'mod+alt+BracketLeft',
	'mod+alt+BracketRight',
	'mod+alt+KeyC',
	'mod+alt+KeyF',
	'mod+alt+KeyL',
	'mod+alt+KeyP',
	'mod+alt+KeyR',
	'mod+alt+KeyW',
	'mod+alt+shift+ArrowLeft',
	'mod+alt+shift+ArrowRight',
	'mod+alt+shift+PageDown',
	'mod+alt+shift+PageUp',
	'mod+control+shift+ArrowLeft',
	'mod+control+shift+ArrowRight',
	'mod+shift+KeyG',
] as const;

/** Windows and Linux alike. */
const STANDARD = [
	'alt+KeyC',
	'alt+KeyL',
	'alt+KeyP',
	'alt+KeyR',
	'alt+KeyW',
	'alt+PageDown',
	'alt+PageUp',
	'mod+End',
	'mod+Home',
	'mod+KeyH',
	'mod+KeyM',
	'mod+KeyY',
	'mod+shift+BracketLeft',
	'mod+shift+BracketRight',
	'mod+shift+End',
	'mod+shift+Home',
] as const;

const WINDOWS = [
	'alt+shift+KeyA',
	'mod+alt+ArrowDown',
	'mod+alt+ArrowUp',
	'mod+alt+shift+ArrowLeft',
	'mod+alt+shift+ArrowRight',
	'mod+alt+shift+PageDown',
	'mod+alt+shift+PageUp',
] as const;

const LINUX = ['mod+shift+KeyA'] as const;

type CodeEditorOperatingSystem = 'linux' | 'mac' | 'windows';

const TABLES: Readonly<Record<CodeEditorOperatingSystem, readonly string[]>> = {
	linux: [...SHARED, ...STANDARD, ...LINUX],
	mac: [...SHARED, ...MAC],
	windows: [...SHARED, ...STANDARD, ...WINDOWS],
};

const chordsByTable = new Map<string, readonly KeyboardShortcutChord[]>();

/**
 * The keys the editor binds itself on `platform`, for a code editor's keyboard scope to yield. The
 * operating system is Monaco's: Apple keys on Apple, and on the standard platform Windows' keys
 * where the user agent names Windows, else Linux's. The same array returns for the same table.
 */
export function codeEditorKeyChords(
	platform: KeyboardShortcutPlatform,
	userAgent: string,
): readonly KeyboardShortcutChord[] {
	const system = operatingSystem(platform, userAgent);
	const cached = chordsByTable.get(`${platform}:${system}`);
	if (cached !== undefined) return cached;
	const chords = Object.freeze(TABLES[system].map((text) => parseChord(text, platform)));
	chordsByTable.set(`${platform}:${system}`, chords);
	return chords;
}

function operatingSystem(
	platform: KeyboardShortcutPlatform,
	userAgent: string,
): CodeEditorOperatingSystem {
	if (platform === 'apple') return 'mac';
	return userAgent.includes('Windows') ? 'windows' : 'linux';
}

function parseChord(text: string, platform: KeyboardShortcutPlatform): KeyboardShortcutChord {
	const modifiers = text.split('+');
	const code = modifiers.pop();
	const chord = normalizeKeyboardShortcutChord({ code, modifiers }, platform, 'widget');
	if (chord === null)
		throw new Error(`The code editor key table names a key it cannot hold: ${text}.`);
	return chord;
}
