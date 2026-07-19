import { describe, expect, it } from 'vitest';

import { resolveTabDropEdge } from '../resolveTabDropEdge';

describe('resolveTabDropEdge', () => {
	it('uses the leading half before the midpoint', () => {
		expect(resolveTabDropEdge(149, 100, 100)).toBe('before');
	});

	it('uses the trailing half from the midpoint onward', () => {
		expect(resolveTabDropEdge(150, 100, 100)).toBe('after');
		expect(resolveTabDropEdge(175, 100, 100)).toBe('after');
	});
});
