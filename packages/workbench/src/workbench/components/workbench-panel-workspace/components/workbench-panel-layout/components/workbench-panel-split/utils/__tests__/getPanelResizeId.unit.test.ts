import { describe, expect, it } from 'vitest';

import { getPanelResizeId } from '../getPanelResizeId';

describe('getPanelResizeId', () => {
	it('names a resizable slot within its owning split', () => {
		expect(getPanelResizeId('split-2', 'panel-1')).toBe('split-2--panel-1');
	});
});
