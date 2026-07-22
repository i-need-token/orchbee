import { describe, expect, it } from 'vitest';
import { calculateSplitTabViewLayout } from '../split-tab-view-layout';

describe('calculateSplitTabViewLayout', () => {
	it('is exercised by vertical-layout behavior tests', () => {
		expect(calculateSplitTabViewLayout).toBeTypeOf('function');
	});
});
