import { describe, expect, it } from 'vitest';
import { calculateRoundedScrollbarThumb, roundedScrollbarThickness } from '../rounded-scrollbar';

describe('rounded scrollbar calculations', () => {
	it('exports the calculations exercised by shared-primitives behavior tests', () => {
		expect(calculateRoundedScrollbarThumb).toBeTypeOf('function');
		expect(roundedScrollbarThickness).toBeTypeOf('function');
	});
});
