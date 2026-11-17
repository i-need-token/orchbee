import {
	ORCHBEE_TERMINAL_SYMBOL_FONT_FAMILY,
	ORCHBEE_TERMINAL_TEXT_FONT_FAMILY,
} from '../../terminal-presentation';

/** The family the default font list leads with, which the font list names as built in. */
export const BUILT_IN_TERMINAL_FONT_FAMILY = ORCHBEE_TERMINAL_TEXT_FONT_FAMILY;

/**
 * The font list stored for an installed family the person picks: the family, then Orchbee's
 * symbols so prompt glyphs keep drawing, then any monospace font where the family is missing.
 */
export function terminalFontFamilyList(family: string): string {
	return `${quoteFamily(family)}, ${quoteFamily(ORCHBEE_TERMINAL_SYMBOL_FONT_FAMILY)}, monospace`;
}

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

function quoteFamily(family: string): string {
	return `"${family.replaceAll('\\', '\\\\').replaceAll('"', '\\"')}"`;
}
