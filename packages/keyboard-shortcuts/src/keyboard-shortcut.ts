export type KeyboardShortcutPlatform = 'apple' | 'standard';
type KeyboardShortcutModifier = 'alt' | 'control' | 'meta' | 'mod' | 'shift';

export interface KeyboardShortcutChord {
	readonly code: string;
	readonly modifiers: readonly KeyboardShortcutModifier[];
}

/**
 * Which chords a binding may hold. A window binding keeps at least one modifier other than Shift. A
 * widget binding, in a tree or list scope, may be unmodified or Shift-only and may hold Escape as a
 * declared default. The second step of a sequence may be unmodified but is never Escape. No rule
 * admits Tab.
 */
export type KeyboardShortcutChordRule = 'second-step' | 'widget' | 'window';

/** One chord, or the sequence leader `Mod+K` followed by a second step. */
export type KeyboardShortcutSequence =
	| readonly [KeyboardShortcutChord]
	| readonly [KeyboardShortcutChord, KeyboardShortcutChord];

export interface KeyboardShortcutEvent {
	readonly altKey: boolean;
	readonly code: string;
	readonly ctrlKey: boolean;
	readonly isComposing: boolean;
	readonly keyCode?: number | undefined;
	readonly metaKey: boolean;
	readonly repeat: boolean;
	readonly shiftKey: boolean;
}

type KeyboardShortcutCaptureResult =
	| { readonly chord: KeyboardShortcutChord; readonly kind: 'captured' }
	| {
			readonly kind: 'ignored';
			readonly reason:
				| 'composing'
				| 'missing-modifier'
				| 'modifier-only'
				| 'repeat'
				| 'unsupported-code';
	  };

const MODIFIER_ORDER: Readonly<Record<KeyboardShortcutModifier, number>> = Object.freeze({
	mod: 0,
	control: 1,
	alt: 2,
	shift: 3,
	meta: 4,
});
const MODIFIER_CODES = new Set([
	'AltLeft',
	'AltRight',
	'ControlLeft',
	'ControlRight',
	'MetaLeft',
	'MetaRight',
	'ShiftLeft',
	'ShiftRight',
]);
const NAMED_CODES = new Set([
	'ArrowDown',
	'ArrowLeft',
	'ArrowRight',
	'ArrowUp',
	'Backquote',
	'Backslash',
	'Backspace',
	'BracketLeft',
	'BracketRight',
	'Comma',
	'Delete',
	'End',
	'Enter',
	'Equal',
	'Home',
	'Insert',
	'Minus',
	'PageDown',
	'PageUp',
	'Period',
	'Quote',
	'Semicolon',
	'Slash',
	'Space',
]);
const KEY_LABELS: Readonly<Record<string, string>> = Object.freeze({
	ArrowDown: '↓',
	ArrowLeft: '←',
	ArrowRight: '→',
	ArrowUp: '↑',
	Backquote: '`',
	Backslash: '\\',
	Backspace: 'Backspace',
	BracketLeft: '[',
	BracketRight: ']',
	Comma: ',',
	Delete: 'Delete',
	End: 'End',
	Enter: 'Enter',
	Equal: '=',
	Home: 'Home',
	Insert: 'Insert',
	Minus: '-',
	PageDown: 'Page Down',
	PageUp: 'Page Up',
	Period: '.',
	Quote: "'",
	Semicolon: ';',
	Slash: '/',
	Space: 'Space',
});

/** `Mod+K`, the first step of every sequence. */
export const KEYBOARD_SHORTCUT_SEQUENCE_LEADER: KeyboardShortcutChord = Object.freeze({
	code: 'KeyK',
	modifiers: Object.freeze(['mod'] as const),
});

export function normalizeKeyboardShortcutChord(
	input: unknown,
	platform: KeyboardShortcutPlatform,
	rule: KeyboardShortcutChordRule = 'window',
): KeyboardShortcutChord | null {
	if (!isRecord(input) || !hasExactKeys(input, ['code', 'modifiers'])) return null;
	if (typeof input['code'] !== 'string' || !isAdmittedCode(input['code'], rule)) return null;
	if (!Array.isArray(input['modifiers'])) return null;
	if (rule === 'window' && input['modifiers'].length === 0) return null;
	const inputModifiers: KeyboardShortcutModifier[] = [];
	for (const value of input['modifiers']) {
		if (!isKeyboardShortcutModifier(value) || inputModifiers.includes(value)) return null;
		inputModifiers.push(value);
	}
	const effectiveModifiers = inputModifiers.map((modifier) =>
		normalizeModifier(modifier, platform),
	);
	if (new Set(effectiveModifiers).size !== effectiveModifiers.length) return null;
	if (rule === 'window' && !effectiveModifiers.some((modifier) => modifier !== 'shift')) {
		return null;
	}
	return Object.freeze({
		code: input['code'],
		modifiers: Object.freeze(
			[...effectiveModifiers].sort((left, right) => MODIFIER_ORDER[left] - MODIFIER_ORDER[right]),
		),
	});
}

/**
 * Captures a pressed chord under `rule`. Escape and Tab are never captured: a recorder lets Tab move
 * focus and Escape cancel recording.
 */
export function captureKeyboardShortcutChord(
	event: KeyboardShortcutEvent,
	platform: KeyboardShortcutPlatform,
	rule: KeyboardShortcutChordRule = 'window',
): KeyboardShortcutCaptureResult {
	if (event.isComposing || event.keyCode === 229) {
		return { kind: 'ignored', reason: 'composing' };
	}
	if (event.repeat) return { kind: 'ignored', reason: 'repeat' };
	if (isKeyboardShortcutModifierCode(event.code)) {
		return { kind: 'ignored', reason: 'modifier-only' };
	}
	if (!isSupportedCode(event.code)) return { kind: 'ignored', reason: 'unsupported-code' };
	const modifiers: KeyboardShortcutModifier[] = [];
	if (platform === 'apple' ? event.metaKey : event.ctrlKey) modifiers.push('mod');
	if (platform === 'apple' && event.ctrlKey) modifiers.push('control');
	if (platform === 'standard' && event.metaKey) modifiers.push('meta');
	if (event.altKey) modifiers.push('alt');
	if (event.shiftKey) modifiers.push('shift');
	const chord = normalizeKeyboardShortcutChord({ code: event.code, modifiers }, platform, rule);
	return chord === null
		? { kind: 'ignored', reason: 'missing-modifier' }
		: { chord, kind: 'captured' };
}

export function matchesKeyboardShortcutChord(
	event: KeyboardShortcutEvent,
	chord: KeyboardShortcutChord,
	platform: KeyboardShortcutPlatform,
): boolean {
	if (event.isComposing || event.keyCode === 229 || event.repeat) return false;
	const normalized = canonicalChord(chord, platform);
	if (normalized === null || event.code !== normalized.code) return false;
	const expected = effectiveModifierFlags(normalized, platform);
	return (
		event.altKey === expected.altKey &&
		event.ctrlKey === expected.ctrlKey &&
		event.metaKey === expected.metaKey &&
		event.shiftKey === expected.shiftKey
	);
}

export function keyboardShortcutChordKey(
	chord: KeyboardShortcutChord,
	platform: KeyboardShortcutPlatform,
): string {
	const normalized = canonicalChord(chord, platform);
	if (normalized === null) throw new TypeError('Invalid keyboard shortcut chord.');
	const flags = effectiveModifierFlags(normalized, platform);
	return `${flags.altKey ? 1 : 0}:${flags.ctrlKey ? 1 : 0}:${flags.metaKey ? 1 : 0}:${flags.shiftKey ? 1 : 0}:${normalized.code}`;
}

export function formatKeyboardShortcutChord(
	chord: KeyboardShortcutChord,
	platform: KeyboardShortcutPlatform,
): string {
	const normalized = canonicalChord(chord, platform);
	if (normalized === null) throw new TypeError('Invalid keyboard shortcut chord.');
	return formatPress(effectiveModifierFlags(normalized, platform), normalized.code, platform);
}

/**
 * Formats the key a person pressed as the chord formatter would, any key included: a status message
 * names a second step that no binding holds, such as Escape.
 */
export function formatKeyboardShortcutPress(
	event: Pick<KeyboardShortcutEvent, 'altKey' | 'code' | 'ctrlKey' | 'metaKey' | 'shiftKey'>,
	platform: KeyboardShortcutPlatform,
): string {
	return formatPress(event, event.code, platform);
}

/** Whether `code` names a modifier key, which never completes a chord by itself. */
export function isKeyboardShortcutModifierCode(code: string): boolean {
	return MODIFIER_CODES.has(code);
}

function formatPress(
	flags: Pick<KeyboardShortcutEvent, 'altKey' | 'ctrlKey' | 'metaKey' | 'shiftKey'>,
	code: string,
	platform: KeyboardShortcutPlatform,
): string {
	const keyLabel = keyLabelFor(code);
	if (platform === 'apple') {
		return `${flags.ctrlKey ? '⌃' : ''}${flags.altKey ? '⌥' : ''}${flags.shiftKey ? '⇧' : ''}${flags.metaKey ? '⌘' : ''}${keyLabel}`;
	}
	return [
		flags.ctrlKey ? 'Ctrl' : null,
		flags.altKey ? 'Alt' : null,
		flags.shiftKey ? 'Shift' : null,
		flags.metaKey ? 'Super' : null,
		keyLabel,
	]
		.filter((value): value is string => value !== null)
		.join('+');
}

/** Apple `⌘K ⌘W`, Standard `Ctrl+K Ctrl+W`: each step in the platform's chord format. */
export function formatKeyboardShortcutSequence(
	sequence: KeyboardShortcutSequence,
	platform: KeyboardShortcutPlatform,
): string {
	return sequence.map((step) => formatKeyboardShortcutChord(step, platform)).join(' ');
}

/** Any chord some rule admits: matching, identity and display take chords validated already. */
function canonicalChord(
	chord: KeyboardShortcutChord,
	platform: KeyboardShortcutPlatform,
): KeyboardShortcutChord | null {
	return normalizeKeyboardShortcutChord(chord, platform, 'widget');
}

function normalizeModifier(
	modifier: KeyboardShortcutModifier,
	platform: KeyboardShortcutPlatform,
): KeyboardShortcutModifier {
	if (platform === 'apple' && modifier === 'meta') return 'mod';
	if (platform === 'standard' && modifier === 'control') return 'mod';
	return modifier;
}

function effectiveModifierFlags(
	chord: KeyboardShortcutChord,
	platform: KeyboardShortcutPlatform,
): Pick<KeyboardShortcutEvent, 'altKey' | 'ctrlKey' | 'metaKey' | 'shiftKey'> {
	const modifiers = new Set(chord.modifiers);
	return {
		altKey: modifiers.has('alt'),
		ctrlKey: modifiers.has('control') || (platform === 'standard' && modifiers.has('mod')),
		metaKey: modifiers.has('meta') || (platform === 'apple' && modifiers.has('mod')),
		shiftKey: modifiers.has('shift'),
	};
}

function keyLabelFor(code: string): string {
	if (code.startsWith('Key')) return code.slice(3);
	if (code.startsWith('Digit')) return code.slice(5);
	return KEY_LABELS[code] ?? code;
}

function isAdmittedCode(code: string, rule: KeyboardShortcutChordRule): boolean {
	return isSupportedCode(code) || (rule === 'widget' && code === 'Escape');
}

function isSupportedCode(code: string): boolean {
	return (
		/^Key[A-Z]$/u.test(code) ||
		/^Digit\d$/u.test(code) ||
		/^F(?:[1-9]|1\d|2[0-4])$/u.test(code) ||
		NAMED_CODES.has(code)
	);
}

function isKeyboardShortcutModifier(value: unknown): value is KeyboardShortcutModifier {
	return (
		value === 'alt' ||
		value === 'control' ||
		value === 'meta' ||
		value === 'mod' ||
		value === 'shift'
	);
}

function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
	return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function hasExactKeys(value: Readonly<Record<string, unknown>>, keys: readonly string[]): boolean {
	const actual = Object.keys(value).sort();
	const expected = [...keys].sort();
	return actual.length === expected.length && actual.every((key, index) => key === expected[index]);
}
