import { cleanup, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { TabModelSnapshot } from '../../../../../../model/tab-model';
import { useVerticalPinAnimation } from '../useVerticalPinAnimation';

afterEach(() => {
	cleanup();
	vi.restoreAllMocks();
});

function snapshot(pinned: boolean): TabModelSnapshot {
	return {
		revision: Number(pinned),
		tabs: [{ id: 'tab', title: 'Tab', pinned, group: null }],
		groups: [],
		activeTabId: 'tab',
		selectedTabIds: new Set(['tab']),
		selectionAnchorId: 'tab',
		mostRecentGroupId: null,
	};
}

describe('useVerticalPinAnimation', () => {
	it('prepares both collection presentations before the pin frame paints', () => {
		vi.spyOn(performance, 'now').mockReturnValue(100);
		const { result, rerender } = renderHook(
			({ current }) =>
				useVerticalPinAnimation({
					snapshot: current,
				}),
			{ initialProps: { current: snapshot(false) } },
		);

		rerender({ current: snapshot(true) });
		expect(result.current.pinned).toEqual([
			expect.objectContaining({ phase: 'destination', mainAxisScale: 0, opacity: 0 }),
		]);
		expect(result.current.unpinned).toEqual([
			expect.objectContaining({ phase: 'source', mainAxisScale: 1, opacity: 1 }),
		]);
	});
});
