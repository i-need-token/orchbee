import { describe, expect, it, vi } from 'vitest';
import { requireDefined } from '../requireDefined';
import type { ProposedLayout } from '../../layout/proposed-layout';
import type { TabSlotViewState } from '../../model/TabSlotViewState';
import { TabCollectionAnimationRuntime, type LiveTabSlot } from '../TabCollectionAnimationRuntime';

function row(ids: readonly string[]): ProposedLayout {
	return {
		hostSize: { width: 240, height: 40 },
		children: ids.map((id, index) => ({
			id,
			visible: true,
			bounds: { x: index * 120, y: 0, width: 120, height: 40 },
		})),
	};
}

function slots(ids: readonly string[]): LiveTabSlot[] {
	return ids.map((id) => ({ id, group: null, dragging: false, pinned: false }));
}

describe('TabCollectionAnimationRuntime', () => {
	it('initializes immediately, then paints per-slot reorder frames', () => {
		const progressed = vi.fn<(slot: TabSlotViewState | null) => void>();
		const laidOut = vi.fn<(slot: TabSlotViewState) => void>();
		const runtime = new TabCollectionAnimationRuntime({
			onTabSlotAnimationProgressed: progressed,
			layoutTabSlotImmediately: laidOut,
		});

		expect(runtime.sync(row(['a', 'b']), slots(['a', 'b'])).changed).toBe(false);
		const reordered = runtime.sync(row(['b', 'a']), slots(['b', 'a']));
		expect(reordered.changed).toBe(true);
		expect(runtime.getSlot('a')?.animating).toBe(true);

		const halfway = runtime.setProgress(0.5);
		expect(halfway.layout.children.find((child) => child.id === 'a')?.bounds.x).toBe(60);
		expect(progressed).toHaveBeenCalled();

		runtime.complete();
		expect(runtime.getSlot('a')?.animating).toBe(false);
		expect(laidOut).toHaveBeenCalled();
	});

	it('keeps unchanged slot targets on their original timelines during a retarget', () => {
		const runtime = new TabCollectionAnimationRuntime();
		runtime.sync(row(['a', 'b', 'c', 'd']), slots(['a', 'b', 'c', 'd']));
		runtime.sync(row(['d', 'a', 'b', 'c']), slots(['d', 'a', 'b', 'c']));

		runtime.setProgress(0.64);
		runtime.sync(row(['d', 'a', 'c', 'b']), slots(['d', 'a', 'c', 'b']));
		const afterRemainingOriginalTime = runtime.setProgress(0.84);

		expect(
			afterRemainingOriginalTime.layout.children.find((child) => child.id === 'd')?.bounds.x,
		).toBe(0);
		expect(
			afterRemainingOriginalTime.layout.children.find((child) => child.id === 'a')?.bounds.x,
		).toBe(120);
		expect(
			afterRemainingOriginalTime.layout.children.find((child) => child.id === 'b')?.bounds.x,
		).toBeLessThan(360);
		expect(runtime.getSlot('a')?.animating).toBe(false);
		expect(runtime.getSlot('b')?.animating).toBe(true);
	});

	it('carries live group and drag state while additions use target bounds', () => {
		const runtime = new TabCollectionAnimationRuntime();
		runtime.sync(row(['a']), slots(['a']));
		const withGroup: LiveTabSlot[] = [
			{ id: 'a', group: 'group-1', dragging: true, pinned: false },
			{ id: 'b', group: 'group-1', dragging: false, pinned: false },
		];
		runtime.sync(row(['a', 'b']), withGroup);

		expect(runtime.getSlot('a')).toMatchObject({
			group: 'group-1',
			dragging: true,
		});
		expect(
			runtime.currentFrame.layout.children.find((child) => child.id === 'b')?.bounds.width,
		).toBe(120);
	});

	it("starts an added group header at its first member's current bounds", () => {
		const runtime = new TabCollectionAnimationRuntime();
		runtime.sync(row(['a', 'b']), slots(['a', 'b']));
		const groupedLayout: ProposedLayout = {
			hostSize: { width: 240, height: 40 },
			children: [
				{
					id: 'group-work',
					visible: true,
					bounds: { x: 0, y: 0, width: 48, height: 40 },
				},
				{
					id: 'a',
					visible: true,
					bounds: { x: 30, y: 0, width: 105, height: 40 },
				},
				{
					id: 'b',
					visible: true,
					bounds: { x: 117, y: 0, width: 123, height: 40 },
				},
			],
		};
		const groupedSlots: LiveTabSlot[] = [
			{ id: 'group-work', group: 'work', dragging: false, pinned: null },
			{ id: 'a', group: 'work', dragging: false, pinned: false },
			{ id: 'b', group: null, dragging: false, pinned: false },
		];

		const started = runtime.sync(groupedLayout, groupedSlots);

		expect(
			started.frame.layout.children.find((child) => child.id === 'group-work')?.bounds,
		).toEqual({
			x: 0,
			y: 0,
			width: 120,
			height: 40,
		});
		expect(runtime.getSlot('group-work')?.animating).toBe(true);
		expect(
			runtime.complete().layout.children.find((child) => child.id === 'group-work')?.bounds,
		).toEqual({
			x: 0,
			y: 0,
			width: 48,
			height: 40,
		});
	});

	it('detaches deleted slots and severs animation observation', () => {
		const runtime = new TabCollectionAnimationRuntime();
		runtime.sync(row(['a', 'b']), slots(['a', 'b']));
		runtime.sync(row(['b', 'a']), slots(['b', 'a']));
		const removed = requireDefined(runtime.getSlot('a'));

		runtime.sync(row(['b']), slots(['b']));
		expect(removed.detached).toBe(true);
		expect(removed.animating).toBe(true);
		expect(runtime.getSlot('a')).toBeUndefined();
	});

	it('retains pin and unpin presentation state until the matching bounds animation ends', () => {
		const runtime = new TabCollectionAnimationRuntime();
		const unpinned: ProposedLayout = {
			hostSize: { width: 240, height: 40 },
			children: [
				{
					id: 'a',
					visible: true,
					bounds: { x: 0, y: 0, width: 120, height: 40 },
				},
			],
		};
		const pinned: ProposedLayout = {
			...unpinned,
			children: [
				{
					id: 'a',
					visible: true,
					bounds: { x: 0, y: 0, width: 64, height: 40 },
				},
			],
		};

		runtime.sync(unpinned, slots(['a']));
		runtime.sync(pinned, [{ id: 'a', group: null, dragging: false, pinned: true }]);
		expect(runtime.getSlot('a')).toMatchObject({
			pinned: true,
			pinTransition: 'pinning',
			animating: true,
		});

		runtime.complete();
		expect(runtime.getSlot('a')).toMatchObject({
			pinned: true,
			pinTransition: null,
			animating: false,
		});

		runtime.sync(unpinned, slots(['a']));
		expect(runtime.getSlot('a')).toMatchObject({
			pinned: false,
			pinTransition: 'unpinning',
			animating: true,
		});
		runtime.complete();
		expect(runtime.getSlot('a')?.pinTransition).toBeNull();
	});
});
