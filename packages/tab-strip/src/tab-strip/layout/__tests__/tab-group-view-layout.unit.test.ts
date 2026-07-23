import { describe, expect, it } from 'vitest';
import {
	calculateTabGroupViewLayout,
	calculateTabGroupViewMinimumSize,
} from '../tab-group-view-layout';

describe('tab group view layout', () => {
	it('exports the calculations exercised by collection-layout behavior tests', () => {
		expect(calculateTabGroupViewLayout).toBeTypeOf('function');
		expect(calculateTabGroupViewMinimumSize).toBeTypeOf('function');
	});
});
