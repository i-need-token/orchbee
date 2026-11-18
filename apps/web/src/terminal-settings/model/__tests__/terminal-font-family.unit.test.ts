import { describe, expect, it } from 'vitest';

import {
	BUILT_IN_TERMINAL_FONT_LIST,
	leadingTerminalFontFamily,
	terminalFontListFromText,
	terminalFontListWithFallback,
	terminalFontListWithFamilyFirst,
} from '../terminal-font-family';

describe('terminal font family lists', () => {
	it('names a picked family again from the list it stores, whatever its characters', () => {
		for (const family of [
			'Cascadia Code',
			'Ubuntu Sans Mono',
			'Odd "Quoted" Face',
			'Back\\slash',
		]) {
			const list = terminalFontListWithFamilyFirst('monospace', family) ?? '';
			expect(terminalFontListFromText(list)).toBe(list);
			expect(leadingTerminalFontFamily(list)).toBe(family);
		}
	});

	it('moves a picked family to the front and keeps the rest of the list in order', () => {
		expect(terminalFontListWithFamilyFirst('Menlo, "Fira Code", monospace', 'Fira Code')).toBe(
			'"Fira Code", Menlo, monospace',
		);
		expect(terminalFontListWithFamilyFirst('Fira Code', 'Menlo')).toBe('"Menlo", Fira Code');
	});

	it('lists a picked family once when the list already names it', () => {
		expect(terminalFontListWithFamilyFirst('"Fira Code", Fira Code, monospace', 'fira code')).toBe(
			'"fira code", monospace',
		);
	});

	it('starts a blank list from the default list', () => {
		expect(terminalFontListWithFamilyFirst('  ', 'Menlo')).toBe(
			`"Menlo", ${BUILT_IN_TERMINAL_FONT_LIST}`,
		);
	});

	it('admits a list that parses and fits the stored size, and refuses the rest', () => {
		expect(terminalFontListFromText(`  "Fira Code", 'Menlo', monospace  `)).toBe(
			`"Fira Code", 'Menlo', monospace`,
		);
		expect(terminalFontListFromText('   ')).toBe('');
		expect(terminalFontListFromText('Fira Code,')).toBeNull();
		expect(terminalFontListFromText('"Fira Code')).toBeNull();
		expect(terminalFontListFromText('"Fira" Code')).toBeNull();
		expect(terminalFontListFromText('Noto 3D')).toBeNull();
		expect(terminalFontListFromText('inherit')).toBeNull();
		expect(terminalFontListFromText('a'.repeat(256))).toBe('a'.repeat(256));
		expect(terminalFontListFromText('a'.repeat(257))).toBeNull();
		// Stored sizes count UTF-8 bytes: 85 three-byte characters fit and 86 do not.
		expect(terminalFontListFromText('中'.repeat(85))).toBe('中'.repeat(85));
		expect(terminalFontListFromText('中'.repeat(86))).toBeNull();
	});

	it('refuses to move a family in a list that does not parse', () => {
		expect(terminalFontListWithFamilyFirst('"Broken', 'Menlo')).toBeNull();
	});

	it('names the first family of a list the person typed', () => {
		expect(leadingTerminalFontFamily('Fira Code, Menlo, monospace')).toBe('Fira Code');
		expect(leadingTerminalFontFamily(" 'JetBrains Mono', monospace")).toBe('JetBrains Mono');
	});

	it('maps a list naming no generic family to Orchbee symbols and a monospace font', () => {
		expect(terminalFontListWithFallback('Orchbee Test Mono')).toBe(
			'Orchbee Test Mono, "Orchbee Symbols Nerd Font", monospace',
		);
		expect(terminalFontListWithFallback('Fira Code, Monospace')).toBe('Fira Code, Monospace');
		expect(terminalFontListWithFallback('Fira Code, SANS-SERIF')).toBe('Fira Code, SANS-SERIF');
		expect(terminalFontListWithFallback('Fira Code, "monospace"')).toBe(
			'Fira Code, "monospace", "Orchbee Symbols Nerd Font", monospace',
		);
	});
});
