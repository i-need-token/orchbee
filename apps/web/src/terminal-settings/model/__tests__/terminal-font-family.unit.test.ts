import { describe, expect, it } from 'vitest';

import { leadingTerminalFontFamily, terminalFontFamilyList } from '../terminal-font-family';

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
});
