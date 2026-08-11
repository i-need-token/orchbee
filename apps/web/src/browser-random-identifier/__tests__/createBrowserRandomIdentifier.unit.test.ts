import { describe, expect, it, vi } from 'vitest';

import { createBrowserRandomIdentifier } from '../createBrowserRandomIdentifier';

describe('createBrowserRandomIdentifier', () => {
	it('uses insecure-context-compatible random values for a 128-bit identifier', () => {
		const fillRandomValues = vi.fn<(values: Uint8Array<ArrayBuffer>) => void>((values) => {
			values.set([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 255]);
		});

		expect(createBrowserRandomIdentifier(fillRandomValues)).toBe(
			'000102030405060708090a0b0c0d0eff',
		);
		expect(fillRandomValues).toHaveBeenCalledOnce();
		expect(fillRandomValues.mock.calls[0]?.[0]).toBeInstanceOf(Uint8Array);
		expect(fillRandomValues.mock.calls[0]?.[0]).toHaveLength(16);
	});
});
