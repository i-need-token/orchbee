import { cleanup, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { TabModel } from '../../../../../model';
import { useVerticalContents } from '../useVerticalContents';

afterEach(cleanup);

describe('useVerticalContents', () => {
	it('initializes both scroll regions without an active drag', () => {
		const model = new TabModel({
			tabs: [{ id: 'tab', title: 'Tab', pinned: false, group: null }],
			groups: [],
			activeTabId: 'tab',
		});
		const { result } = renderHook(() =>
			useVerticalContents({ availableWidth: 240, drag: null, snapshot: model.getSnapshot() }),
		);

		expect(result.current.regionsSize).toEqual({ width: 0, height: 0 });
		expect(result.current.isAnimatingSize).toBe(false);
	});
});
