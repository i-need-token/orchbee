import {
	keyboardShortcutChordKey,
	matchesKeyboardShortcutChord,
	type KeyboardShortcutChord,
	type KeyboardShortcutPlatform,
} from '@orchbee/keyboard-shortcuts';

/**
 * What a key asks of the tree; each is the VS Code list command of the same name, and the segment
 * commands are VS Code's compact folder commands, such as `previousCompressedFolder`.
 */
export type TreeKeyCommand =
	| 'clear'
	| 'closeFind'
	| 'collapse'
	| 'collapseAll'
	| 'expand'
	| 'expandSelectionDown'
	| 'expandSelectionUp'
	| 'find'
	| 'firstSegment'
	| 'focusAnyDown'
	| 'focusAnyFirst'
	| 'focusAnyLast'
	| 'focusAnyUp'
	| 'focusFirst'
	| 'focusLast'
	| 'focusNext'
	| 'focusPageDown'
	| 'focusPageUp'
	| 'focusPrevious'
	| 'lastSegment'
	| 'nextSegment'
	| 'previousSegment'
	| 'scrollDown'
	| 'scrollUp'
	| 'select'
	| 'selectAll'
	| 'toggleExpand'
	| 'toggleSelection';

interface TreeKeyBinding {
	readonly chord: KeyboardShortcutChord;
	readonly command: TreeKeyCommand;
}

function bind(
	code: string,
	command: TreeKeyCommand,
	modifiers: KeyboardShortcutChord['modifiers'] = [],
): TreeKeyBinding {
	return { chord: { code, modifiers }, command };
}

/**
 * VS Code's default list and tree keys on Windows and Linux. Alt with an arrow, Home or End is VS
 * Code's `focusAny` family, which lands on rows a find would pass over. Within a compact row, Left,
 * Right, Home and End first move among its items, as VS Code's Explorer binds them over its list's
 * keys, and Escape closes an open find before it clears anything.
 */
const STANDARD_KEYS: readonly TreeKeyBinding[] = [
	bind('ArrowDown', 'focusNext'),
	bind('ArrowUp', 'focusPrevious'),
	bind('ArrowDown', 'focusAnyDown', ['alt']),
	bind('ArrowUp', 'focusAnyUp', ['alt']),
	bind('PageDown', 'focusPageDown'),
	bind('PageUp', 'focusPageUp'),
	bind('Home', 'firstSegment'),
	bind('Home', 'focusFirst'),
	bind('End', 'lastSegment'),
	bind('End', 'focusLast'),
	bind('Home', 'focusAnyFirst', ['alt']),
	bind('End', 'focusAnyLast', ['alt']),
	bind('ArrowDown', 'expandSelectionDown', ['shift']),
	bind('ArrowUp', 'expandSelectionUp', ['shift']),
	bind('ArrowLeft', 'previousSegment'),
	bind('ArrowLeft', 'collapse'),
	bind('ArrowLeft', 'collapseAll', ['mod']),
	bind('ArrowRight', 'nextSegment'),
	bind('ArrowRight', 'expand'),
	bind('Enter', 'select'),
	bind('KeyA', 'selectAll', ['mod']),
	bind('Enter', 'toggleSelection', ['mod', 'shift']),
	bind('Space', 'toggleExpand'),
	bind('Escape', 'closeFind'),
	bind('Escape', 'clear'),
	bind('ArrowUp', 'scrollUp', ['mod']),
	bind('ArrowDown', 'scrollDown', ['mod']),
	bind('KeyF', 'find', ['mod', 'alt']),
	bind('F3', 'find'),
];

/**
 * VS Code's keys on macOS: the same, with Emacs-style Control+N and Control+P, and Cmd with the
 * arrows in place of Ctrl. Where VS Code binds one chord to two commands, the one its resolver
 * prefers comes first: Cmd+Up scrolls unless the tree is at the top, where it collapses, and
 * Cmd+Down always selects.
 */
const APPLE_KEYS: readonly TreeKeyBinding[] = [
	bind('ArrowDown', 'focusNext'),
	bind('KeyN', 'focusNext', ['control']),
	bind('ArrowUp', 'focusPrevious'),
	bind('KeyP', 'focusPrevious', ['control']),
	bind('ArrowDown', 'focusAnyDown', ['alt']),
	bind('KeyN', 'focusAnyDown', ['control', 'alt']),
	bind('ArrowUp', 'focusAnyUp', ['alt']),
	bind('KeyP', 'focusAnyUp', ['control', 'alt']),
	bind('PageDown', 'focusPageDown'),
	bind('PageUp', 'focusPageUp'),
	bind('Home', 'firstSegment'),
	bind('Home', 'focusFirst'),
	bind('End', 'lastSegment'),
	bind('End', 'focusLast'),
	bind('Home', 'focusAnyFirst', ['alt']),
	bind('End', 'focusAnyLast', ['alt']),
	bind('ArrowDown', 'expandSelectionDown', ['shift']),
	bind('ArrowUp', 'expandSelectionUp', ['shift']),
	bind('ArrowLeft', 'previousSegment'),
	bind('ArrowLeft', 'collapse'),
	bind('ArrowUp', 'scrollUp', ['mod']),
	bind('ArrowUp', 'collapse', ['mod']),
	bind('ArrowLeft', 'collapseAll', ['mod']),
	bind('ArrowUp', 'collapseAll', ['mod', 'shift']),
	bind('ArrowRight', 'nextSegment'),
	bind('ArrowRight', 'expand'),
	bind('Enter', 'select'),
	bind('ArrowDown', 'select', ['mod']),
	bind('KeyA', 'selectAll', ['mod']),
	bind('Enter', 'toggleSelection', ['mod', 'shift']),
	bind('Space', 'toggleExpand'),
	bind('Escape', 'closeFind'),
	bind('Escape', 'clear'),
	bind('KeyF', 'find', ['mod', 'alt']),
	bind('F3', 'find'),
];

const KEYS: Readonly<Record<KeyboardShortcutPlatform, readonly TreeKeyBinding[]>> = {
	apple: APPLE_KEYS,
	standard: STANDARD_KEYS,
};

/** Shift+F10 asks for the context menu of the focused row, as the context menu key does. */
const CONTEXT_MENU_CHORD: KeyboardShortcutChord = { code: 'F10', modifiers: ['shift'] };

/** The commands only a tree that finds rows answers. */
const FIND_COMMANDS: ReadonlySet<TreeKeyCommand> = new Set(['closeFind', 'find']);

function chordsOf(
	bindings: readonly TreeKeyBinding[],
	platform: KeyboardShortcutPlatform,
	find: boolean,
): readonly KeyboardShortcutChord[] {
	const chords = new Map<string, KeyboardShortcutChord>();
	const bound = bindings.filter((binding) => find || !FIND_COMMANDS.has(binding.command));
	for (const chord of [...bound.map((binding) => binding.chord), CONTEXT_MENU_CHORD]) {
		chords.set(keyboardShortcutChordKey(chord, platform), chord);
	}
	return Object.freeze([...chords.values()]);
}

const KEY_CHORDS: Readonly<
	Record<
		KeyboardShortcutPlatform,
		Readonly<Record<'find' | 'plain', readonly KeyboardShortcutChord[]>>
	>
> = {
	apple: { find: chordsOf(APPLE_KEYS, 'apple', true), plain: chordsOf(APPLE_KEYS, 'apple', false) },
	standard: {
		find: chordsOf(STANDARD_KEYS, 'standard', true),
		plain: chordsOf(STANDARD_KEYS, 'standard', false),
	},
};

/**
 * The chords a focused tree handles itself on a platform, with the find's when it finds rows. The
 * tree registers them as its keyboard scope's `yields`, so no wider binding, such as the window's
 * `Mod+Shift+Enter`, takes them from the tree.
 */
export function virtualTreeKeyChords(
	platform: KeyboardShortcutPlatform,
	find: boolean,
): readonly KeyboardShortcutChord[] {
	return find ? KEY_CHORDS[platform].find : KEY_CHORDS[platform].plain;
}

/** Numeric-pad keys that move as their arrows do while Num Lock is off. */
const NUMPAD_NAVIGATION_KEYS = new Set([
	'ArrowDown',
	'ArrowLeft',
	'ArrowRight',
	'ArrowUp',
	'End',
	'Enter',
	'Home',
	'PageDown',
	'PageUp',
]);

/** The parts of a key press that decide its commands. */
type TreeKeyPress = Pick<
	KeyboardEvent,
	'altKey' | 'code' | 'ctrlKey' | 'key' | 'metaKey' | 'shiftKey'
>;

/**
 * The commands a key press asks for, in the order to try them: the first one that applies takes the
 * key. A held key repeats its command, and the numeric pad's arrows and Enter count as the others.
 */
export function treeKeyCommands(
	event: TreeKeyPress,
	platform: KeyboardShortcutPlatform,
): readonly TreeKeyCommand[] {
	const code =
		event.code.startsWith('Numpad') && NUMPAD_NAVIGATION_KEYS.has(event.key)
			? event.key
			: event.code;
	const press = {
		altKey: event.altKey,
		code,
		ctrlKey: event.ctrlKey,
		isComposing: false,
		metaKey: event.metaKey,
		repeat: false,
		shiftKey: event.shiftKey,
	};
	const commands: TreeKeyCommand[] = [];
	for (const { chord, command } of KEYS[platform]) {
		if (matchesKeyboardShortcutChord(press, chord, platform)) commands.push(command);
	}
	return commands;
}

/** Whether a key press asks for the context menu: the context menu key, or Shift+F10. */
export function isContextMenuKey(event: KeyboardEvent): boolean {
	return event.code === 'ContextMenu' || (event.shiftKey && event.code === 'F10');
}
