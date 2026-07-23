import { cleanup, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { TabStripOrientation } from '../../types/tab-strip-types';
import { TabModel } from '../../model/tab-model';
import { useTabStrip } from '../useTabStrip';

afterEach(cleanup);

describe('useTabStrip', () => {
	it('prepares the horizontal view model without rendering JSX', () => {
		const model = new TabModel({
			tabs: [{ id: 'tab', title: 'Tab', pinned: false, group: null }],
			groups: [],
			activeTabId: 'tab',
		});
		const { result } = renderHook(() => useTabStrip({ model }));

		expect(result.current.orientation).toBe(TabStripOrientation.Horizontal);
		expect(result.current.snapshot.activeTabId).toBe('tab');
		expect(result.current.focusedGroupColor).toBeUndefined();
	});
});
