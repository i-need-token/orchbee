import { describe, expect, it } from 'vitest';

import {
	leadingTerminalFontFamily,
	terminalFontFamilyList,
	terminalFontListWithFallback,
} from '../terminal-font-family';

describe('terminal font family lists', () => {
	it('names the picked family again from the list it stored, whatever its characters', () => {
		for (const family of [
			'Cascadia Code',
			'Ubuntu Sans Mono',
			'Odd "Quoted" Face',
			'Back\\slash',
		]) {
			expect(leadingTerminalFontFamily(terminalFontFamilyList(family))).toBe(family);
		}
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
