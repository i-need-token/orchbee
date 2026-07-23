import { describe, expect, it } from 'vitest';

import { requireDefined } from '../requireDefined';

describe('requireDefined', () => {
	it('returns present values and rejects missing invariant values', () => {
		expect(requireDefined('tab')).toBe('tab');
		expect(() => requireDefined(undefined, 'missing tab')).toThrow('missing tab');
	});
});
