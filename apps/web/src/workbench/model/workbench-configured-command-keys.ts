import {
	formatKeyboardShortcutChord,
	keyboardShortcutChordKey,
	matchesKeyboardShortcutChord,
	normalizeKeyboardShortcutChord,
	type KeyboardShortcutChord,
	type KeyboardShortcutEvent,
} from '@orchbee/keyboard-shortcuts';

import type { WorkbenchConfiguredCommand } from '../types/workbench-configured-commands';
import type { WorkbenchShortcutClaim } from '../types/workbench-shortcut-claims';
import type { WorkbenchShortcutPlatform } from '../types/workbench-shortcut-platform';

/** Why a configured command's row says something about its keys. */
export type WorkbenchConfiguredCommandKeyNote =
	| {
			/** `took`: a window default's chord runs the command; `yielded`: the person's shortcut keeps it. */
			readonly kind: 'took' | 'yielded';
			readonly action: string;
			readonly shortcut: string;
	  }
	| { readonly kind: 'tui-only'; readonly shortcut: string };

/** Which configured commands the window's keys run, after its own shortcuts have had their say. */
export interface WorkbenchConfiguredCommandKeys {
	/** The chords that run each command now, in its configured order. */
	readonly chords: ReadonlyMap<string, readonly KeyboardShortcutChord[]>;
	/** The labels of those chords; no window shortcut shows one of them while a command holds it. */
	readonly heldLabels: ReadonlySet<string>;
	readonly notes: ReadonlyMap<string, WorkbenchConfiguredCommandKeyNote>;
	/** The command holding each window default it took, by the claim's id. */
	readonly taken: ReadonlyMap<string, string>;
}

/** The label order the TUI writes; a label out of this order did not come from it. */
const LABEL_MODIFIERS: readonly string[] = ['prefix', 'ctrl', 'alt', 'shift', 'super'];

const NAMED_CODES: ReadonlyMap<string, string> = new Map([
	['backspace', 'Backspace'],
	['delete', 'Delete'],
	['down', 'ArrowDown'],
	['end', 'End'],
	['home', 'Home'],
	['insert', 'Insert'],
	['left', 'ArrowLeft'],
	['pagedown', 'PageDown'],
	['pageup', 'PageUp'],
	['right', 'ArrowRight'],
	['space', 'Space'],
	['up', 'ArrowUp'],
]);

/**
 * The US-layout key that types each printable symbol and whether it needs Shift there, since the
 * window binds physical keys as its own shortcuts do. The TUI labels `+` as `plus`.
 */
const SYMBOL_CODES: ReadonlyMap<string, readonly [code: string, shift: boolean]> = new Map([
	['`', ['Backquote', false]],
	['~', ['Backquote', true]],
	['-', ['Minus', false]],
	['_', ['Minus', true]],
	['=', ['Equal', false]],
	['plus', ['Equal', true]],
	['[', ['BracketLeft', false]],
	['{', ['BracketLeft', true]],
	[']', ['BracketRight', false]],
	['}', ['BracketRight', true]],
	['\\', ['Backslash', false]],
	['|', ['Backslash', true]],
	[';', ['Semicolon', false]],
	[':', ['Semicolon', true]],
	["'", ['Quote', false]],
	['"', ['Quote', true]],
	[',', ['Comma', false]],
	['<', ['Comma', true]],
	['.', ['Period', false]],
	['>', ['Period', true]],
	['/', ['Slash', false]],
	['?', ['Slash', true]],
	['!', ['Digit1', true]],
	['@', ['Digit2', true]],
	['#', ['Digit3', true]],
	['$', ['Digit4', true]],
	['%', ['Digit5', true]],
	['^', ['Digit6', true]],
	['&', ['Digit7', true]],
	['*', ['Digit8', true]],
	['(', ['Digit9', true]],
	[')', ['Digit0', true]],
]);

function keyCode(key: string): readonly [code: string, shift: boolean] | null {
	if (/^[a-z]$/u.test(key)) return [`Key${key.toUpperCase()}`, false];
	if (/^\d$/u.test(key)) return [`Digit${key}`, false];
	if (/^f(?:[1-9]|1\d|2[0-4])$/u.test(key)) return [`F${key.slice(1)}`, false];
	const named = NAMED_CODES.get(key);
	if (named !== undefined) return [named, false];
	return SYMBOL_CODES.get(key) ?? null;
}

/**
 * The window chord for a key label the TUI wrote, or null where the window cannot bind it: a prefix
 * chord, a key without Ctrl, Alt or Super, which stays with the terminal, and a key no US-layout key
 * types, such as Esc or a letter outside ASCII.
 */
function workbenchConfiguredCommandChord(
	label: string,
	platform: WorkbenchShortcutPlatform,
): KeyboardShortcutChord | null {
	const parts = label.split('+');
	const key = parts.pop();
	if (key === undefined || key.length === 0) return null;
	let next = 0;
	for (const part of parts) {
		const position = LABEL_MODIFIERS.indexOf(part);
		if (position < next) return null;
		next = position + 1;
	}
	if (parts.includes('prefix')) return null;
	const code = keyCode(key);
	if (code === null) return null;
	const [physicalCode, needsShift] = code;
	const modifiers: ('alt' | 'control' | 'meta' | 'shift')[] = [];
	if (parts.includes('ctrl')) modifiers.push('control');
	if (parts.includes('alt')) modifiers.push('alt');
	if (parts.includes('shift') || needsShift) modifiers.push('shift');
	if (parts.includes('super')) modifiers.push('meta');
	return normalizeKeyboardShortcutChord({ code: physicalCode, modifiers }, platform);
}

/**
 * Decides which chords run configured commands: a command takes a chord from a window default, yields
 * it to a shortcut the person set, and the first command in the list keeps a chord two commands share.
 * Each command keeps one note for Search: a chord it took or yielded, else, when the window binds none
 * of its keys, the first key that works only in the TUI.
 */
export function resolveWorkbenchConfiguredCommandKeys(
	commands: readonly WorkbenchConfiguredCommand[],
	claims: readonly WorkbenchShortcutClaim[],
	platform: WorkbenchShortcutPlatform,
): WorkbenchConfiguredCommandKeys {
	const claimed = new Map(
		claims.map((claim) => [keyboardShortcutChordKey(claim.chord, platform), claim]),
	);
	const held = new Set<string>();
	const heldLabels = new Set<string>();
	const chords = new Map<string, readonly KeyboardShortcutChord[]>();
	const notes = new Map<string, WorkbenchConfiguredCommandKeyNote>();
	const taken = new Map<string, string>();
	for (const command of commands) {
		const bound: KeyboardShortcutChord[] = [];
		let note: WorkbenchConfiguredCommandKeyNote | undefined;
		let tuiOnly: string | undefined;
		for (const label of command.bindings) {
			const chord = workbenchConfiguredCommandChord(label, platform);
			if (chord === null) {
				tuiOnly ??= label;
				continue;
			}
			const key = keyboardShortcutChordKey(chord, platform);
			if (held.has(key)) continue;
			const shortcut = formatKeyboardShortcutChord(chord, platform);
			const claim = claimed.get(key);
			if (claim?.personal === true) {
				note ??= { action: claim.action, kind: 'yielded', shortcut };
				continue;
			}
			if (claim !== undefined) {
				note ??= { action: claim.action, kind: 'took', shortcut };
				taken.set(claim.id, command.id);
			}
			held.add(key);
			heldLabels.add(shortcut);
			bound.push(chord);
		}
		chords.set(command.id, bound);
		if (note === undefined && bound.length === 0 && tuiOnly !== undefined) {
			note = { kind: 'tui-only', shortcut: tuiOnly };
		}
		if (note !== undefined) notes.set(command.id, note);
	}
	return { chords, heldLabels, notes, taken };
}

/** The configured command a key press runs, if any. */
export function findWorkbenchConfiguredCommandForKey(
	keys: WorkbenchConfiguredCommandKeys,
	event: KeyboardShortcutEvent,
	platform: WorkbenchShortcutPlatform,
): string | null {
	for (const [commandId, chords] of keys.chords) {
		if (chords.some((chord) => matchesKeyboardShortcutChord(event, chord, platform))) {
			return commandId;
		}
	}
	return null;
}
