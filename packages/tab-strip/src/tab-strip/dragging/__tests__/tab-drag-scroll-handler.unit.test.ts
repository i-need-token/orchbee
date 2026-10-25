import { afterEach, describe, expect, it, vi } from 'vitest';
import { TabDragScrollHandler, type TabDragScrollable } from '../tab-drag-scroll-handler';

function scrollable(): TabDragScrollable & {
	gradient: Parameters<NonNullable<TabDragScrollable['setDragOverflowGradient']>>[0];
} {
	return {
		scrollTop: 0,
		scrollHeight: 500,
		clientHeight: 100,
		gradient: 'vertical',
		scrollBy({ top }) {
			this.scrollTop += top;
		},
		setDragOverflowGradient(gradient) {
			this.gradient = gradient;
		},
	};
}

afterEach(() => vi.useRealTimers());

describe('TabDragScrollHandler', () => {
	it('scrolls down by five every 20ms while the dragged tab crosses the bottom edge', () => {
		vi.useFakeTimers();
		const view = scrollable();
		const onContentsScrolled = vi.fn<() => void>();
		const handler = new TabDragScrollHandler(onContentsScrolled);
		handler.onDraggedTabPositionUpdated(view, { x: 0, y: 90, width: 100, height: 20 });
		expect(handler.getSnapshot()).toEqual({
			scrolling: true,
			increment: 5,
			gradient: 'vertical-leading',
		});
		expect(view.gradient).toBe('vertical-leading');
		vi.advanceTimersByTime(20 * 3);
		expect(view.scrollTop).toBe(15);
		expect(onContentsScrolled).toHaveBeenCalledTimes(3);
		handler.destroy();
	});

	it('switches direction at the top and stops away from either edge', () => {
		vi.useFakeTimers();
		const view = scrollable();
		view.scrollTop = 50;
		const handler = new TabDragScrollHandler();
		handler.onDraggedTabPositionUpdated(view, { x: 0, y: 0, width: 100, height: 20 });
		expect(handler.getSnapshot().increment).toBe(-5);
		expect(view.gradient).toBe('vertical-trailing');
		vi.advanceTimersByTime(20);
		expect(view.scrollTop).toBe(45);
		handler.onDraggedTabPositionUpdated(view, { x: 0, y: 30, width: 100, height: 20 });
		expect(handler.getSnapshot().scrolling).toBe(false);
		expect(view.gradient).toBe('vertical');
	});

	it('does not scroll past content boundaries', () => {
		vi.useFakeTimers();
		const view = scrollable();
		view.scrollTop = 400;
		const handler = new TabDragScrollHandler();
		handler.onDraggedTabPositionUpdated(view, { x: 0, y: 100, width: 100, height: 20 });
		expect(handler.getSnapshot().scrolling).toBe(false);
	});
});
