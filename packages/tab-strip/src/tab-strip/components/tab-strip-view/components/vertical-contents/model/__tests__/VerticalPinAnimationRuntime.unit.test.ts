import { describe, expect, it } from 'vitest';
import type { TabModelSnapshot } from '../../../../../../model/tab-model';
import { VerticalPinAnimationRuntime } from '../VerticalPinAnimationRuntime';

function snapshot(pinned: boolean, group: string | null): TabModelSnapshot {
	return {
		revision: Number(pinned),
		tabs: [{ id: 'tab', title: 'Tab', pinned, group }],
		groups: group === null ? [] : [{ id: group, title: 'Group', color: 'blue', collapsed: false }],
		activeTabId: 'tab',
		selectedTabIds: new Set(['tab']),
		selectionAnchorId: 'tab',
		mostRecentGroupId: group,
	};
}

describe('VerticalPinAnimationRuntime', () => {
	it('keeps a grouped unpinned source while its pinned destination grows and fades in', () => {
		const runtime = new VerticalPinAnimationRuntime();
		expect(runtime.sync(snapshot(false, 'group'), 100)).toBe(false);
		expect(runtime.sync(snapshot(true, null), 200)).toBe(true);
		expect(runtime.currentAnimations).toEqual([
			expect.objectContaining({
				tabId: 'tab',
				direction: 'pinning',
				sourceTab: expect.objectContaining({ pinned: false, group: 'group' }),
				sourceGroup: expect.objectContaining({ id: 'group' }),
				sourceMainAxisScale: 1,
				sourceOpacity: 1,
				destinationMainAxisScale: 0,
				destinationOpacity: 0,
			}),
		]);

		expect(runtime.advance(1_200, 2_000)).toEqual([
			expect.objectContaining({
				progress: 0.5,
				sourceMainAxisScale: 0.5,
				sourceOpacity: 0.5,
				destinationMainAxisScale: 0.5,
				destinationOpacity: 0.5,
			}),
		]);
	});

	it('retains a pinned source during unpin and deletes it only at completion', () => {
		const runtime = new VerticalPinAnimationRuntime();
		runtime.sync(snapshot(true, null), 0);
		runtime.sync(snapshot(false, null), 10);
		expect(runtime.currentAnimations[0]).toMatchObject({
			direction: 'unpinning',
			sourceTab: { pinned: true },
		});

		expect(runtime.advance(2_009, 2_000)).toHaveLength(1);
		expect(runtime.advance(2_010, 2_000)).toHaveLength(0);
		expect(runtime.isAnimating).toBe(false);
	});
});
