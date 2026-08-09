import { describe, expect, it } from 'vitest';

import { containsUnpairedSurrogate } from '../containsUnpairedSurrogate';

describe('containsUnpairedSurrogate', () => {
	it('accepts Unicode scalar values and complete surrogate pairs', () => {
		expect(containsUnpairedSurrogate('Orchbee 中文 🐝')).toBe(false);
		expect(containsUnpairedSurrogate('\ud83d\udc1d')).toBe(false);
	});

	it('rejects lone and mismatched surrogate code units', () => {
		expect(containsUnpairedSurrogate('\ud800')).toBe(true);
		expect(containsUnpairedSurrogate('\udc00')).toBe(true);
		expect(containsUnpairedSurrogate('\ud800A')).toBe(true);
		expect(containsUnpairedSurrogate('\ud800\ud800')).toBe(true);
	});
});
