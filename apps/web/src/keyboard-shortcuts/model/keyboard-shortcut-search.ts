import {
	formatKeyboardShortcutChord,
	keyboardShortcutChordKey,
	normalizeKeyboardShortcutChord,
	type KeyboardShortcutChord,
	type KeyboardShortcutPlatform,
} from '@orchbee/keyboard-shortcuts';

export interface KeyboardShortcutSearchEntry {
	readonly actionId: string;
	readonly actionName: string;
	readonly availability: string;
	readonly chord: KeyboardShortcutChord;
	readonly scope: string;
	readonly shortcutId: string;
	readonly source: string;
}

export type KeyboardShortcutSearchQuery =
	| { readonly kind: 'chord'; readonly chord: KeyboardShortcutChord }
	| { readonly kind: 'text'; readonly value: string };

export interface KeyboardShortcutSearchState {
	readonly mode: 'recording' | 'text';
	readonly query: KeyboardShortcutSearchQuery;
	readonly recordingIssue: 'invalid-chord' | null;
}

export type KeyboardShortcutSearchEvent =
	| { readonly type: 'cleared' }
	| { readonly type: 'recording-started' }
	| { readonly type: 'recording-stopped' }
	| { readonly type: 'shortcut-captured'; readonly chord: KeyboardShortcutChord }
	| { readonly type: 'shortcut-rejected' }
	| { readonly type: 'text-changed'; readonly value: string };

export interface KeyboardShortcutSearchResult {
	readonly matchingShortcutIds: readonly string[];
	readonly totalCount: number;
}

const EMPTY_TEXT_QUERY: KeyboardShortcutSearchQuery = Object.freeze({
	kind: 'text',
	value: '',
});

export const INITIAL_KEYBOARD_SHORTCUT_SEARCH_STATE: KeyboardShortcutSearchState = Object.freeze({
	mode: 'text',
	query: EMPTY_TEXT_QUERY,
	recordingIssue: null,
});

const MODIFIER_SEARCH_TERMS = new Set([
	'alt',
	'cmd',
	'command',
	'control',
	'ctrl',
	'meta',
	'mod',
	'option',
	'shift',
	'super',
	'win',
	'windows',
]);

const NAMED_KEY_ALIASES: Readonly<Record<string, readonly string[]>> = Object.freeze({
	ArrowDown: Object.freeze(['arrow', 'down', 'arrowdown', 'down arrow', '↓']),
	ArrowLeft: Object.freeze(['arrow', 'left', 'arrowleft', 'left arrow', '←']),
	ArrowRight: Object.freeze(['arrow', 'right', 'arrowright', 'right arrow', '→']),
	ArrowUp: Object.freeze(['arrow', 'up', 'arrowup', 'up arrow', '↑']),
	Backquote: Object.freeze(['backquote', 'grave', '`']),
	Backslash: Object.freeze(['backslash', '\\']),
	Backspace: Object.freeze(['backspace']),
	BracketLeft: Object.freeze(['bracket', 'left', 'bracketleft', 'left bracket', '[']),
	BracketRight: Object.freeze(['bracket', 'right', 'bracketright', 'right bracket', ']']),
	Comma: Object.freeze(['comma', ',']),
	Delete: Object.freeze(['delete', 'del']),
	End: Object.freeze(['end']),
	Enter: Object.freeze(['enter', 'return']),
	Equal: Object.freeze(['equal', 'equals', '=']),
	Home: Object.freeze(['home']),
	Insert: Object.freeze(['insert', 'ins']),
	Minus: Object.freeze(['minus', '-']),
	PageDown: Object.freeze(['page', 'down', 'pagedown', 'page down']),
	PageUp: Object.freeze(['page', 'up', 'pageup', 'page up']),
	Period: Object.freeze(['period', 'dot', '.']),
	Quote: Object.freeze(['quote', "'"]),
	Semicolon: Object.freeze(['semicolon', ';']),
	Slash: Object.freeze(['slash', '/']),
	Space: Object.freeze(['space', 'spacebar']),
});

const KEY_SEARCH_TERMS = new Set([
	...'abcdefghijklmnopqrstuvwxyz',
	...'0123456789',
	...'abcdefghijklmnopqrstuvwx'.split('').map((_, index) => `f${index + 1}`),
	...Object.values(NAMED_KEY_ALIASES).flatMap((aliases) =>
		aliases.flatMap((alias) => normalizeSearchText(alias).split(/[\s+]+/u)),
	),
]);

export function transitionKeyboardShortcutSearch(
	state: KeyboardShortcutSearchState,
	event: KeyboardShortcutSearchEvent,
	platform: KeyboardShortcutPlatform,
): KeyboardShortcutSearchState {
	switch (event.type) {
		case 'cleared':
			return {
				...state,
				query: EMPTY_TEXT_QUERY,
				recordingIssue: null,
			};
		case 'recording-started':
			return state.mode === 'recording'
				? state
				: { ...state, mode: 'recording', recordingIssue: null };
		case 'recording-stopped':
			return state.mode === 'text' && state.recordingIssue === null
				? state
				: { ...state, mode: 'text', recordingIssue: null };
		case 'shortcut-captured': {
			const chord = normalizeKeyboardShortcutChord(event.chord, platform);
			return chord === null
				? { ...state, recordingIssue: 'invalid-chord' }
				: { ...state, query: { chord, kind: 'chord' }, recordingIssue: null };
		}
		case 'shortcut-rejected':
			return { ...state, recordingIssue: 'invalid-chord' };
		case 'text-changed':
			return state.mode === 'recording'
				? state
				: {
						...state,
						query: { kind: 'text', value: event.value },
						recordingIssue: null,
					};
	}
}

export function keyboardShortcutSearchQueryValue(
	query: KeyboardShortcutSearchQuery,
	platform: KeyboardShortcutPlatform,
): string {
	return query.kind === 'text' ? query.value : formatKeyboardShortcutChord(query.chord, platform);
}

export function resolveKeyboardShortcutSearch(
	entries: readonly KeyboardShortcutSearchEntry[],
	query: KeyboardShortcutSearchQuery,
	platform: KeyboardShortcutPlatform,
): KeyboardShortcutSearchResult {
	const matchingShortcutIds =
		query.kind === 'chord'
			? resolveExactChord(entries, query.chord, platform)
			: resolveText(entries, query.value, platform);
	return Object.freeze({
		matchingShortcutIds: Object.freeze(matchingShortcutIds),
		totalCount: entries.length,
	});
}

function resolveExactChord(
	entries: readonly KeyboardShortcutSearchEntry[],
	chord: KeyboardShortcutChord,
	platform: KeyboardShortcutPlatform,
): string[] {
	const normalized = normalizeKeyboardShortcutChord(chord, platform);
	if (normalized === null) return [];
	const queryKey = keyboardShortcutChordKey(normalized, platform);
	return entries
		.filter((entry) => keyboardShortcutChordKey(entry.chord, platform) === queryKey)
		.map((entry) => entry.shortcutId);
}

function resolveText(
	entries: readonly KeyboardShortcutSearchEntry[],
	value: string,
	platform: KeyboardShortcutPlatform,
): string[] {
	const normalizedQuery = normalizeSearchText(value);
	if (normalizedQuery.length === 0) return entries.map((entry) => entry.shortcutId);
	const terms = normalizedQuery.split(/[\s+]+/u).filter((term) => term.length > 0);
	const chordLike = isChordLikeQuery(value, terms);
	const requestedModifierFamilies = chordLike
		? modifierFamiliesForTerms(terms, platform)
		: new Set<string>();
	const hasKeyTerm = terms.some((term) => !MODIFIER_SEARCH_TERMS.has(term));
	return entries
		.filter((entry) => {
			const chordText = chordSearchText(entry.chord, platform);
			if (chordLike) {
				const chordTerms = new Set(chordText.split(/[\s+]+/u));
				if (!terms.every((term) => chordTerms.has(term))) return false;
				return (
					!hasKeyTerm ||
					setsEqual(requestedModifierFamilies, modifierFamiliesForChord(entry.chord, platform))
				);
			}
			const searchableText = normalizeSearchText(
				[
					entry.actionId,
					entry.actionName,
					entry.availability,
					entry.scope,
					entry.shortcutId,
					entry.source,
					chordText,
				].join(' '),
			);
			return terms.every((term) => searchableText.includes(term));
		})
		.map((entry) => entry.shortcutId);
}

function chordSearchText(chord: KeyboardShortcutChord, platform: KeyboardShortcutPlatform): string {
	const normalized = normalizeKeyboardShortcutChord(chord, platform);
	if (normalized === null) return '';
	return normalizeSearchText(
		[
			formatKeyboardShortcutChord(normalized, platform),
			normalized.code,
			...normalized.modifiers.flatMap((modifier) => modifierAliases(modifier, platform)),
			...keyAliases(normalized.code),
		].join(' '),
	);
}

function modifierAliases(
	modifier: KeyboardShortcutChord['modifiers'][number],
	platform: KeyboardShortcutPlatform,
): readonly string[] {
	switch (modifier) {
		case 'alt':
			return ['alt', 'option', '⌥'];
		case 'control':
			return ['control', 'ctrl', '⌃'];
		case 'meta':
			return ['meta', 'super', 'win', 'windows', 'command', 'cmd'];
		case 'mod':
			return platform === 'apple'
				? ['mod', 'meta', 'command', 'cmd', '⌘']
				: ['mod', 'control', 'ctrl'];
		case 'shift':
			return ['shift', '⇧'];
	}
}

function keyAliases(code: string): readonly string[] {
	if (code.startsWith('Key')) return [code, code.slice(3), `key ${code.slice(3)}`];
	if (code.startsWith('Digit')) return [code, code.slice(5), `digit ${code.slice(5)}`];
	if (/^F(?:[1-9]|1\d|2[0-4])$/u.test(code)) return [code];
	return NAMED_KEY_ALIASES[code] ?? [code];
}

function normalizeSearchText(value: string): string {
	return value
		.normalize('NFKC')
		.toLocaleLowerCase('en-US')
		.replaceAll('⌃', ' control ctrl ')
		.replaceAll('⌥', ' option alt ')
		.replaceAll('⇧', ' shift ')
		.replaceAll('⌘', ' command cmd meta ')
		.replace(/\s+/gu, ' ')
		.trim();
}

function isChordLikeQuery(value: string, terms: readonly string[]): boolean {
	if (!terms.some((term) => MODIFIER_SEARCH_TERMS.has(term))) return false;
	const nonModifierTerms = terms.filter((term) => !MODIFIER_SEARCH_TERMS.has(term));
	return /[+⌃⌥⇧⌘]/u.test(value) || nonModifierTerms.every((term) => KEY_SEARCH_TERMS.has(term));
}

function modifierFamiliesForTerms(
	terms: readonly string[],
	platform: KeyboardShortcutPlatform,
): Set<string> {
	const families = new Set<string>();
	for (const term of terms) {
		if (term === 'shift') families.add('shift');
		if (term === 'alt' || term === 'option') families.add('alt');
		if (term === 'control' || term === 'ctrl') {
			families.add(platform === 'apple' ? 'control' : 'primary');
		}
		if (term === 'mod') families.add('primary');
		if (
			term === 'cmd' ||
			term === 'command' ||
			term === 'meta' ||
			term === 'super' ||
			term === 'win' ||
			term === 'windows'
		) {
			families.add(platform === 'apple' ? 'primary' : 'meta');
		}
	}
	return families;
}

function modifierFamiliesForChord(
	chord: KeyboardShortcutChord,
	platform: KeyboardShortcutPlatform,
): Set<string> {
	const normalized = normalizeKeyboardShortcutChord(chord, platform);
	const families = new Set<string>();
	if (normalized === null) return families;
	for (const modifier of normalized.modifiers) {
		if (modifier === 'mod') families.add('primary');
		if (modifier === 'control') families.add('control');
		if (modifier === 'meta') families.add('meta');
		if (modifier === 'alt') families.add('alt');
		if (modifier === 'shift') families.add('shift');
	}
	return families;
}

function setsEqual(left: ReadonlySet<string>, right: ReadonlySet<string>): boolean {
	return left.size === right.size && [...left].every((value) => right.has(value));
}
