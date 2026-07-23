import { describe, expect, it, vi } from 'vitest';
import { requireDefined } from '../requireDefined';
import type { ProposedLayout } from '../../layout/proposed-layout';
import type { TabSlotViewState } from '../../model/TabSlotViewState';
import { TabCollectionAnimationRuntime, type LiveTabSlot } from '../TabCollectionAnimationRuntime';

function row(ids: readonly string[]): ProposedLayout {
	return {
		hostSize: { width: 240, height: 41 },
		children: ids.map((id, index) => ({
			id,
			visible: true,
			bounds: { x: index * 120, y: 0, width: 120, height: 41 },
		})),
	};
}

function slots(ids: readonly string[]): LiveTabSlot[] {
	return ids.map((id) => ({ id, group: null, dragging: false }));
}

describe('TabCollectionAnimationRuntime', () => {
	it('initializes immediately, then paints source-manager reorder frames', () => {
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

	it('carries live group and drag state and animates additions from zero width', () => {
		const runtime = new TabCollectionAnimationRuntime();
		runtime.sync(row(['a']), slots(['a']));
		const withGroup: LiveTabSlot[] = [
			{ id: 'a', group: 'group-1', dragging: true },
			{ id: 'b', group: 'group-1', dragging: false },
		];
		runtime.sync(row(['a', 'b']), withGroup);

		expect(runtime.getSlot('a')).toMatchObject({
			group: 'group-1',
			dragging: true,
		});
		expect(
			runtime.currentFrame.layout.children.find((child) => child.id === 'b')?.bounds.width,
		).toBe(0);
		expect(runtime.setProgress(0.5).opacityById.get('b')).toBe(0.5);
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
});
