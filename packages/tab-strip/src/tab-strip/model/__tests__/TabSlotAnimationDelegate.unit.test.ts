import { describe, expect, it, vi } from 'vitest';
import { TabSlotAnimationDelegate } from '../TabSlotAnimationDelegate';
import { TabSlotViewState } from '../TabSlotViewState';

describe('TabSlotAnimationDelegate', () => {
	it('marks the slot animating and preserves Chromium completion order', () => {
		const calls: string[] = [];
		const slot = new TabSlotViewState();
		const delegate = new TabSlotAnimationDelegate(
			{
				onTabSlotAnimationProgressed: (value) => calls.push(`progress:${value?.animating}`),
				layoutTabSlotImmediately: () => calls.push('layout'),
			},
			slot,
		);
		expect(slot.animating).toBe(true);
		delegate.animationEnded();
		expect(slot.animating).toBe(false);
		expect(calls).toEqual(['progress:false', 'layout']);
	});

	it('treats cancellation as completion', () => {
		const slot = new TabSlotViewState();
		const progressed = vi.fn<() => void>();
		const delegate = new TabSlotAnimationDelegate(
			{ onTabSlotAnimationProgressed: progressed },
			slot,
		);
		delegate.animationCanceled();
		expect(slot.animating).toBe(false);
		expect(progressed).toHaveBeenCalledWith(slot);
	});

	it('stops touching a deleted view while still reporting progress', () => {
		const slot = new TabSlotViewState();
		const progressed = vi.fn<() => void>();
		const layout = vi.fn<() => void>();
		const delegate = new TabSlotAnimationDelegate(
			{ onTabSlotAnimationProgressed: progressed, layoutTabSlotImmediately: layout },
			slot,
		);
		delegate.onViewDeleting(slot);
		delegate.animationEnded();
		expect(progressed).toHaveBeenCalledWith(null);
		expect(layout).not.toHaveBeenCalled();
		delegate.stopObserving();
		expect(delegate.isObserving).toBe(false);
	});
});
