import { describe, expect, it } from 'vitest';
import {
	calculateUnpinnedTabContainerLayout,
	calculateUnpinnedTabContainerMinimumSize,
} from '../unpinned-tab-container-view-layout';

describe('unpinned tab container layout', () => {
	it('exports the calculations exercised by collection-layout behavior tests', () => {
		expect(calculateUnpinnedTabContainerLayout).toBeTypeOf('function');
		expect(calculateUnpinnedTabContainerMinimumSize).toBeTypeOf('function');
	});
});
