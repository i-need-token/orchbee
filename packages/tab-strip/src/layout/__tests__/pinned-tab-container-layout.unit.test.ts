import { describe, expect, it } from 'vitest';
import {
	calculatePinnedTabContainerLayout,
	calculatePinnedTabContainerMinimumSize,
	getPinnedTabContainerDragAxes,
} from '../pinned-tab-container-layout';

describe('pinned tab container layout', () => {
	it('exports the calculations exercised by collection-layout behavior tests', () => {
		expect(calculatePinnedTabContainerLayout).toBeTypeOf('function');
		expect(calculatePinnedTabContainerMinimumSize).toBeTypeOf('function');
		expect(getPinnedTabContainerDragAxes).toBeTypeOf('function');
	});
});
