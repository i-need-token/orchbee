import { describe, expect, it } from 'vitest';

import {
	parseScrollbackBytes,
	parseTerminalFontWeight,
	parseTerminalPadding,
} from '../terminal-settings-draft-parsing';

describe('terminal settings draft parsing', () => {
	it('admits only supported integer-hundred font weights', () => {
		expect(parseTerminalFontWeight('100')).toBe(100);
		expect(parseTerminalFontWeight('900')).toBe(900);
		expect(parseTerminalFontWeight('450')).toBeNull();
		expect(parseTerminalFontWeight('400.0')).toBe(400);
		expect(parseTerminalFontWeight('not-a-weight')).toBeNull();
	});

	it('admits canonical integer terminal padding within the product range', () => {
		expect(parseTerminalPadding('0')).toBe(0);
		expect(parseTerminalPadding('32')).toBe(32);
		expect(parseTerminalPadding('')).toBeNull();
		expect(parseTerminalPadding('01')).toBeNull();
		expect(parseTerminalPadding('33')).toBeNull();
	});

	it('admits canonical decimal scrollback bytes within the Host range', () => {
		expect(parseScrollbackBytes('0')).toBe(0);
		expect(parseScrollbackBytes('1000000000')).toBe(1_000_000_000);
		expect(parseScrollbackBytes('01')).toBeNull();
		expect(parseScrollbackBytes('-1')).toBeNull();
		expect(parseScrollbackBytes('1000000001')).toBeNull();
	});
});
