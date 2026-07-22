import { describe, expect, it } from 'vitest';
import { TabWidthConstraints } from '../TabWidthConstraints';

describe('TabWidthConstraints', () => {
	it('is exercised by tab-strip-layout behavior tests', () => {
		expect(TabWidthConstraints).toBeTypeOf('function');
	});
});
