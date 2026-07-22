import { describe, expect, it, vi } from 'vitest';
import { TabModel } from '../../model';
import type { TabDragContext, TabDragPositioningDelegate } from '../tab-drag-context';
import type { TabDragTarget, TabDragTargetController } from '../tab-drag-target';
import {
	DomTabDragRegistry,
	MOUSE_VERTICAL_DETACH_MAGNETISM,
	TOUCH_VERTICAL_DETACH_MAGNETISM,
	domRectToScreenRect,
} from '../dom-tab-drag-registry';

function context(id: string, begin = 0, end = 200): TabDragContext {
	const model = new TabModel({
		tabs: [{ id: `${id}-tab`, title: id, pinned: false, group: null, split: null }],
	});
	const delegate = {
		tabDragAreaBeginX: () => begin,
		tabDragAreaEndX: () => end,
	} as TabDragPositioningDelegate;
	return {
		id,
		getContextForNewBrowser: () => null,
		getTabForContents: (contentsId) => contentsId,
		getContentsForTab: (tabId) => tabId,
		isTabDetachable: () => true,
		getTabGroupHeader: () => null,
		getTabStripModel: () => model,
		startedDragging: vi.fn<() => void>(),
		draggedTabsDetached: vi.fn<() => void>(),
		stoppedDragging: vi.fn<() => void>(),
		getPositioningDelegate: () => delegate,
	};
}

const dragTarget: TabDragTarget = {
	onTabDragEntered: vi.fn<() => void>(),
	onTabDragUpdated: vi.fn<
		(
			controller: TabDragTargetController,
			point: Readonly<{ x: number; y: number }>,
		) => TabDragContext | null
	>(() => null),
	onTabDragExited: vi.fn<(point: Readonly<{ x: number; y: number }>) => void>(),
	onTabDragEnded: vi.fn<() => void>(),
	canDropTab: () => false,
	handleTabDrop: vi.fn<() => void>(),
	registerWillDestroyCallback: () => () => {},
};

describe('DomTabDragRegistry', () => {
	it('uses topmost window order and the mouse detach magnetism', () => {
		const registry = new DomTabDragRegistry();
		registry.registerBrowser({
			browserId: 'back',
			context: context('back'),
			getWindowBounds: () => ({ x: 0, y: 0, width: 500, height: 400 }),
			getTabStripBounds: () => ({ x: 0, y: 0, width: 200, height: 40 }),
		});
		registry.registerBrowser({
			browserId: 'front',
			context: context('front', 10, 190),
			getWindowBounds: () => ({ x: 100, y: 100, width: 300, height: 250 }),
			getTabStripBounds: () => ({ x: 100, y: 100, width: 200, height: 40 }),
			getDragTarget: () => dragTarget,
		});

		expect(registry.getBrowserIdAtPoint({ x: 150, y: 120 })).toBe('front');
		expect(registry.getContextAtPoint('front', { x: 110, y: 100 })?.id).toBe('front');
		expect(registry.getContextAtPoint('front', { x: 109, y: 100 })).toBeNull();
		expect(
			registry.getContextAtPoint('front', { x: 150, y: 140 + MOUSE_VERTICAL_DETACH_MAGNETISM - 1 })
				?.id,
		).toBe('front');
		expect(
			registry.getContextAtPoint('front', { x: 150, y: 140 + MOUSE_VERTICAL_DETACH_MAGNETISM }),
		).toBeNull();
		expect(registry.getDragTarget('front', { x: 150, y: 200 })).toBe(dragTarget);
	});

	it('uses the wider touch magnetism and unregisters browser hosts', () => {
		const registry = new DomTabDragRegistry();
		const unregister = registry.registerBrowser({
			browserId: 'touch',
			context: context('touch'),
			touch: true,
			getWindowBounds: () => ({ x: 0, y: 0, width: 300, height: 300 }),
			getTabStripBounds: () => ({ x: 0, y: 50, width: 200, height: 40 }),
		});
		expect(
			registry.getContextAtPoint('touch', { x: 50, y: 90 + TOUCH_VERTICAL_DETACH_MAGNETISM - 1 })
				?.id,
		).toBe('touch');
		unregister();
		expect(registry.getBrowserIdAtPoint({ x: 50, y: 50 })).toBeNull();
	});

	it('converts viewport bounds into cross-window screen coordinates', () => {
		const rect = { left: 20, top: 30, width: 400, height: 40 } as DOMRect;
		expect(domRectToScreenRect(rect, { screenX: 100, screenY: 200 })).toEqual({
			x: 120,
			y: 230,
			width: 400,
			height: 40,
		});
	});
});
