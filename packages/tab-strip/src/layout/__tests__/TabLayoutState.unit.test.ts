import { describe, expect, it } from 'vitest';
import { TabLayoutState } from '../TabLayoutState';

describe('TabLayoutState', () => {
	it('is exercised by tab-strip-layout behavior tests', () => {
		expect(TabLayoutState).toBeTypeOf('function');
	});
});
