import {
	ORCHBEE_TERMINAL_FONT_FAMILY,
	ORCHBEE_TERMINAL_SYMBOL_FONT_FAMILY,
	ORCHBEE_TERMINAL_TEXT_FONT_FAMILY,
} from '../../terminal-presentation';

/** The family the default font list leads with, which the font list names as built in. */
export const BUILT_IN_TERMINAL_FONT_FAMILY = ORCHBEE_TERMINAL_TEXT_FONT_FAMILY;

/** The default font list, which the setting stores as unset. */
export const BUILT_IN_TERMINAL_FONT_LIST = ORCHBEE_TERMINAL_FONT_FAMILY;

/** The longest font list the setting stores, in UTF-8 bytes, as the settings file admits it. */
const MAXIMUM_FONT_LIST_BYTES = 256;

const UTF8_ENCODER = new TextEncoder();

const GENERIC_FONT_FAMILY_KEYWORDS = new Set([
	'monospace',
	'ui-monospace',
	'serif',
	'sans-serif',
	'ui-serif',
	'ui-sans-serif',
	'ui-rounded',
	'system-ui',
	'cursive',
	'fantasy',
	'math',
	'emoji',
	'fangsong',
]);

/** Words a font list may not name unquoted: the CSS-wide keywords and default. */
const RESERVED_FONT_FAMILY_WORDS = new Set([
	'default',
	'inherit',
	'initial',
	'revert',
	'revert-layer',
	'unset',
]);

/** One unquoted word of a family name, which CSS reads as an identifier. */
const FONT_FAMILY_WORD = /^-?[A-Za-z_\u0080-￿][\w\u0080-￿-]*$/;

/**
 * The text as the font list the setting stores: the trimmed text, which is '' when the text is blank
 * and means the default list, or null when the text does not parse as a CSS font-family list or is
 * over 256 UTF-8 bytes.
 */
export function terminalFontListFromText(text: string): string | null {
	const list = text.trim();
	if (list === '') return '';
	if (parseFontList(list) === null) return null;
	return UTF8_ENCODER.encode(list).length <= MAXIMUM_FONT_LIST_BYTES ? list : null;
}

/**
 * The font list that moves the family to its front and keeps the rest of the text's list in order.
 * A blank text starts from the default list, and the result is null when the text does not parse.
 * The result is not checked against the stored size; {@link terminalFontListFromText} does that.
 */
export function terminalFontListWithFamilyFirst(text: string, family: string): string | null {
	const entries = parseFontList(text.trim() === '' ? BUILT_IN_TERMINAL_FONT_LIST : text);
	if (entries === null) return null;
	const folded = family.toLowerCase();
	const rest = entries.filter((entry) => entry.family.toLowerCase() !== folded);
	return [quoteFamily(family), ...rest.map((entry) => entry.spelling)].join(', ');
}

/**
 * A list that names no generic family gets Orchbee's symbols and any monospace font after it, so
 * a missing family never falls back to the browser's serif.
 */
export function terminalFontListWithFallback(list: string): string {
	const lastEntry = list
		.slice(list.lastIndexOf(',') + 1)
		.trim()
		.toLowerCase();
	if (GENERIC_FONT_FAMILY_KEYWORDS.has(lastEntry)) return list;
	return `${list}, ${quoteFamily(ORCHBEE_TERMINAL_SYMBOL_FONT_FAMILY)}, monospace`;
}

/** The first family a CSS font list names, without its quotes; empty for an empty list. */
export function leadingTerminalFontFamily(list: string): string {
	const text = list.trimStart();
	const quote = text[0];
	if (quote !== '"' && quote !== "'") return (text.split(',')[0] ?? '').trim();
	let family = '';
	for (let index = 1; index < text.length; index += 1) {
		const character = text[index];
		if (character === quote) return family;
		if (character === '\\') {
			index += 1;
			family += text[index] ?? '';
		} else {
			family += character;
		}
	}
	return family;
}

interface FontListEntry {
	/** The family the entry names, unquoted and unescaped. */
	readonly family: string;
	/** The entry as the list spells it, which a list keeps when another family moves to its front. */
	readonly spelling: string;
}

/**
 * The entries of a CSS font-family list, or null when the text is not one. An entry is a quoted
 * family name or identifiers separated by spaces, entries are separated by commas, and none is empty.
 */
function parseFontList(text: string): readonly FontListEntry[] | null {
	const pieces: string[] = [];
	let piece = '';
	let quote: string | null = null;
	for (let index = 0; index < text.length; index += 1) {
		const character = text.charAt(index);
		if (quote !== null) {
			piece += character;
			if (character === '\\') {
				piece += text.charAt(index + 1);
				index += 1;
			} else if (character === quote) {
				quote = null;
			}
		} else if (character === '"' || character === "'") {
			quote = character;
			piece += character;
		} else if (character === ',') {
			pieces.push(piece);
			piece = '';
		} else {
			piece += character;
		}
	}
	if (quote !== null) return null;
	pieces.push(piece);
	const entries: FontListEntry[] = [];
	for (const candidate of pieces) {
		const entry = fontListEntry(candidate);
		if (entry === null) return null;
		entries.push(entry);
	}
	return entries;
}

function fontListEntry(piece: string): FontListEntry | null {
	const spelling = piece.trim();
	const first = spelling.charAt(0);
	return first === '"' || first === "'" ? quotedEntry(spelling) : wordsEntry(spelling);
}

/** A quoted family name: one string, closed by its own quote at the end of the entry. */
function quotedEntry(spelling: string): FontListEntry | null {
	const quote = spelling.charAt(0);
	let family = '';
	for (let index = 1; index < spelling.length; index += 1) {
		const character = spelling.charAt(index);
		if (character === '\\') {
			index += 1;
			if (index === spelling.length) return null;
			family += spelling.charAt(index);
		} else if (character === quote) {
			return index === spelling.length - 1 && family !== '' ? { family, spelling } : null;
		} else {
			family += character;
		}
	}
	return null;
}

/** An unquoted family name: identifiers separated by spaces, none of them a reserved word. */
function wordsEntry(spelling: string): FontListEntry | null {
	const words = spelling.split(/\s+/);
	const admitted = words.every(
		(word) => FONT_FAMILY_WORD.test(word) && !RESERVED_FONT_FAMILY_WORDS.has(word.toLowerCase()),
	);
	return admitted ? { family: words.join(' '), spelling } : null;
}

function quoteFamily(family: string): string {
	return `"${family.replaceAll('\\', '\\\\').replaceAll('"', '\\"')}"`;
}
