import { act, cleanup, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { DragSessionData, DragSlotViewType } from '../../../../../../dragging/drag-session-data';
import { TAB_DRAG_SCROLL_INTERVAL_MS } from '../../../../../../dragging/tab-drag-scroll-handler';
import { TabModel, type TabGroupData } from '../../../../../../model/tab-model';
import type { DragState } from '../../../../../../utils/tab-strip-internals';
import { useVerticalContents } from '../useVerticalContents';

afterEach(() => {
	cleanup();
	vi.useRealTimers();
});

function activeDrag(model: TabModel, y: number): DragState {
	const dragSessionData = new DragSessionData<Readonly<TabGroupData>>({
		tabDragData: [
			{
				sourceModelIndex: 0,
				viewType: DragSlotViewType.Tab,
				contentsId: 'tab',
				attachedViewId: 'tab',
				pinned: false,
				tabGroupData: null,
			},
		],
		sourceViewIndex: 0,
		mouseOffsetToSizeRatios: { x: 0.5, y: 0.5 },
	});
	dragSessionData.markDragStarted(0, false);
	return {
		sourceId: 'tab',
		tabIds: ['tab'],
		pointerId: 1,
		pointerType: 'mouse',
		touchLongPressRecognized: false,
		startX: 100,
		startY: 40,
		screenX: 100,
		screenY: y,
		shiftPressedOnPointerDown: false,
		x: 100,
		y,
		started: true,
		targetIndex: 0,
		targetGroup: null,
		targetIdealLeft: null,
		lastReorderCoordinate: y,
		restoreState: model.captureDragRestoreState(),
		groupHeaderId: null,
		groupWasCollapsedBeforeDrag: false,
		horizontalGroupDrag: null,
		label: 'Tab',
		visualViews: [{ id: 'tab', kind: 'tab', x: 0, y: 0, width: 200, height: 32 }],
		pointerOffsetRatios: { x: 0.5, y: 0.5 },
		pointerOffsetFromVisualOrigin: { x: 100, y: 16 },
		sourceViewOffsetFromSlot: { x: 0, y: 0 },
		sourceSlotSize: { width: 200, height: 32 },
		groupUnderlineBounds: {},
		dragAreaBounds: { left: 0, top: 0, width: 240, height: 100 },
		dragSessionData,
		dragStartAnimationRequired: false,
		dragStartAnimationStartedAt: 0,
		dragStartAnimationProgress: 1,
		crossWindowRuntime: null,
		settling: false,
	};
}

describe('useVerticalContents', () => {
	it('initializes both scroll regions without an active drag', () => {
		const model = new TabModel({
			tabs: [{ id: 'tab', title: 'Tab', pinned: false, group: null }],
			groups: [],
			activeTabId: 'tab',
		});
		const { result } = renderHook(() =>
			useVerticalContents({
				availableWidth: 240,
				collapsed: false,
				drag: null,
				onDragScrolled: () => {},
				snapshot: model.getSnapshot(),
			}),
		);

		expect(result.current.regionsSize).toEqual({ width: 0, height: 0 });
		expect(result.current.isAnimatingSize).toBe(false);
	});

	it('replays drag positioning after each product scroll step and cleans up the gradient', () => {
		vi.useFakeTimers();
		const model = new TabModel({
			tabs: [{ id: 'tab', title: 'Tab', pinned: false, group: null }],
			activeTabId: 'tab',
		});
		const onDragScrolled = vi.fn<() => void>();
		const initialDrag = activeDrag(model, 90);
		const { result, rerender } = renderHook(
			({ drag }: { drag: DragState }) =>
				useVerticalContents({
					availableWidth: 240,
					collapsed: false,
					drag,
					onDragScrolled,
					snapshot: model.getSnapshot(),
				}),
			{ initialProps: { drag: initialDrag } },
		);
		const scroll = document.createElement('div');
		Object.defineProperties(scroll, {
			clientHeight: { configurable: true, value: 100 },
			scrollHeight: { configurable: true, value: 500 },
			getBoundingClientRect: {
				configurable: true,
				value: () => ({
					x: 0,
					y: 0,
					left: 0,
					top: 0,
					right: 240,
					bottom: 100,
					width: 240,
					height: 100,
					toJSON: () => ({}),
				}),
			},
			scrollBy: {
				configurable: true,
				value: (options: ScrollToOptions) => {
					scroll.scrollTop += options.top ?? 0;
				},
			},
		});
		result.current.scrollRef.current = scroll;
		rerender({ drag: { ...initialDrag, y: 91 } });

		expect(scroll.dataset['dragScrollGradient']).toBe('vertical-leading');
		act(() => vi.advanceTimersByTime(TAB_DRAG_SCROLL_INTERVAL_MS));
		expect(scroll.scrollTop).toBe(5);
		expect(onDragScrolled).toHaveBeenCalledOnce();

		rerender({ drag: { ...initialDrag, y: 40 } });
		expect(scroll.dataset['dragScrollGradient']).toBeUndefined();
	});
});
