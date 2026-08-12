import { eastAsianWidth } from 'get-east-asian-width';
import { describe, expect, it } from 'vitest';

import { ghosttyCodepointWidth } from '../ghosttyCodepointWidth';

describe('ghosttyCodepointWidth', () => {
	it.each([
		[0x61, 1, 'ASCII'],
		[0x10_ffff, 1, 'maximum code point'],
		[0x00, 0, 'NUL'],
		[0x07, 0, 'BEL'],
		[0x1b, 0, 'ESC'],
		[0x7f, 0, 'DEL'],
		[0x80, 0, 'C1 PAD'],
		[0x301, 0, 'combining acute'],
		[0x200b, 0, 'zero-width space'],
		[0x200d, 0, 'zero-width joiner'],
		[0xfe0f, 0, 'variation selector 16'],
		[0xd800, 0, 'surrogate'],
		[0x4e00, 2, 'CJK ideograph'],
		[0xff21, 2, 'fullwidth A'],
		[0xac00, 2, 'Hangul syllable'],
		[0x1f600, 2, 'emoji'],
		[0x1f1e6, 2, 'regional indicator'],
		[0x2e3b, 2, 'clamped three-em dash'],
		[0x20_000, 2, 'CJK Extension B'],
		[0x30_000, 2, 'CJK Extension G'],
	] as const)('maps %s to %s cells for %s', (codepoint, width, _name) => {
		expect(ghosttyCodepointWidth(codepoint, eastAsianWidth)).toBe(width);
	});

	it('treats out-of-range inputs as one standalone cell', () => {
		expect(ghosttyCodepointWidth(0x11_0000, eastAsianWidth)).toBe(1);
		expect(ghosttyCodepointWidth(Number.NaN, eastAsianWidth)).toBe(1);
	});
});
