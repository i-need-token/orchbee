import { describe, expect, it } from 'vitest';

import { TabStripImplementation } from '../tab-strip-implementation';

describe('TabStripImplementation', () => {
	it('provides the private implementation exercised through TabStrip', () => {
		expect(TabStripImplementation).toBeTypeOf('function');
	});
});
