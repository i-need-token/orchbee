import { cleanup, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import type { TabData } from '../../../../../../../../model/tab-model';
import { useTabIcon } from '../useTabIcon';

afterEach(cleanup);

describe('useTabIcon', () => {
	it('does not animate a tab that is already crashed on mount', () => {
		const tab: TabData = {
			id: 'tab',
			title: 'Tab',
			pinned: false,
			group: null,
			crashed: true,
		};
		const { result } = renderHook(() => useTabIcon({ tab }));

		expect(result.current.crashTransition).toBe(false);
		expect(result.current.visual.showingCrashedFavicon).toBe(true);
	});
});
