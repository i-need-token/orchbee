import { describe, expect, it } from 'vitest';
import {
	calculatePinnedContainerMainAxisSize,
	calculateTabStripViewLayout,
} from '../tab-strip-view-layout';

describe('tab strip view layout', () => {
	it('exports the calculations exercised by collection-layout behavior tests', () => {
		expect(calculatePinnedContainerMainAxisSize).toBeTypeOf('function');
		expect(calculateTabStripViewLayout).toBeTypeOf('function');
	});
});
