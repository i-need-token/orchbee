import { describe, expect, it, vi } from 'vitest';
import { TabModel, type DetachedTabBundle } from '../../model/tab-model';
import { requireDefined } from '../../utils/requireDefined';
import { DragSessionData, DragSlotViewType } from '../drag-session-data';
import { DRAG_START_ANIMATION_DURATION_MS } from '../dragged-tabs-container-layout';
import {
	CrossWindowDragState,
	CrossWindowTabDragRuntime,
	type CrossWindowDragHost,
} from '../cross-window-drag-runtime';
import type { TabDragContext, TabDragPositioningDelegate } from '../tab-drag-context';
import type {
	TabDragPointResolver,
	TabDragTarget,
	TabDragTargetController,
} from '../tab-drag-target';

function model(ids: string[], active = ids[0] ?? null) {
	return new TabModel({
		tabs: ids.map((id) => ({ id, title: id, pinned: false, group: null })),
		activeTabId: active,
	});
}

class FakeContext implements TabDragContext {
	public readonly startedDragging = vi.fn<(viewIds: readonly string[]) => void>();
	public readonly draggedTabsDetached = vi.fn<() => void>();
	public readonly stoppedDragging = vi.fn<() => void>();
	public readonly id: string;
	private readonly tabModel: TabModel;
	private readonly contexts: Map<string, FakeContext>;
	public positioningDelegate: TabDragPositioningDelegate | null = null;

	public constructor(id: string, tabModel: TabModel, contexts: Map<string, FakeContext>) {
		this.id = id;
		this.tabModel = tabModel;
		this.contexts = contexts;
	}

	public getContextForNewBrowser(browserId: string): TabDragContext | null {
		return this.contexts.get(browserId) ?? null;
	}
	public getTabForContents(contentsId: string): string | null {
		return this.tabModel.getIndexOfTab(contentsId) === null ? null : contentsId;
	}
	public getContentsForTab(tabSlotId: string): string | null {
		return this.getTabForContents(tabSlotId);
	}
	public isTabDetachable(tabSlotId: string): boolean {
		return this.tabModel.getIndexOfTab(tabSlotId) !== null;
	}
	public getTabGroupHeader(groupId: string): string | null {
		return `group-${groupId}`;
	}
	public getTabStripModel(): TabModel {
		return this.tabModel;
	}
	public getPositioningDelegate(): TabDragPositioningDelegate | null {
		return this.positioningDelegate;
	}
}

class FakeTarget implements TabDragTarget<DetachedTabBundle> {
	public readonly entered = vi.fn<() => void>();
	public readonly updated =
		vi.fn<
			(
				controller: TabDragTargetController<DetachedTabBundle>,
				point: { x: number; y: number },
			) => void
		>();
	public readonly exited = vi.fn<(point: { x: number; y: number }) => void>();
	public readonly ended = vi.fn<() => void>();
	public readonly dropped = vi.fn<(bundle: DetachedTabBundle) => void>();
	private destroyCallback: (() => void) | null = null;
	private readonly context: TabDragContext | null;
	private readonly droppable: boolean;

	public constructor(context: TabDragContext | null, droppable = false) {
		this.context = context;
		this.droppable = droppable;
	}
	public onTabDragEntered(): void {
		this.entered();
	}
	public onTabDragUpdated(
		controller: TabDragTargetController<DetachedTabBundle>,
		point: { x: number; y: number },
	): TabDragContext | null {
		this.updated(controller, point);
		return this.context;
	}
	public onTabDragExited(point: { x: number; y: number }): void {
		this.exited(point);
	}
	public onTabDragEnded(): void {
		this.ended();
	}
	public canDropTab(): boolean {
		return this.droppable;
	}
	public handleTabDrop(controller: TabDragTargetController<DetachedTabBundle>): void {
		this.dropped(controller.detachTabAtForInsertion(0));
	}
	public registerWillDestroyCallback(callback: () => void): () => void {
		this.destroyCallback = callback;
		return () => {
			this.destroyCallback = null;
		};
	}
	public destroy(): void {
		this.destroyCallback?.();
	}
}

function session(tabIds: string[], dragStartTime?: number) {
	return new DragSessionData({
		tabDragData: tabIds.map((id, index) => ({
			sourceModelIndex: index,
			viewType: DragSlotViewType.Tab,
			contentsId: id,
			attachedViewId: id,
			pinned: false,
			tabGroupData: null,
		})),
		sourceViewIndex: 0,
		mouseOffsetToSizeRatios: { x: 0.5, y: 0.5 },
		...(dragStartTime === undefined ? {} : { dragStartTime }),
	});
}

function setup(targetOverride: TabDragTarget<DetachedTabBundle> | null = null) {
	const contexts = new Map<string, FakeContext>();
	const sourceModel = model(['a', 'b'], 'a');
	const targetModel = model(['x'], 'x');
	const source = new FakeContext('source', sourceModel, contexts);
	const target = new FakeContext('target', targetModel, contexts);
	contexts.set(source.id, source);
	contexts.set(target.id, target);
	const resolver: TabDragPointResolver = {
		getDragTarget: (browserId) => (browserId === 'target' ? targetOverride : null),
	};
	const host: CrossWindowDragHost = {
		getBrowserIdAtPoint: (point) => (point.x < 100 ? 'source' : 'target'),
		getInsertionIndex: (_context, point) => point.y,
	};
	return { source, target, sourceModel, targetModel, resolver, host };
}

describe('CrossWindowTabDragRuntime', () => {
	it('hands tabs to an existing context and reports the destination', () => {
		const setupData = setup();
		const runtime = new CrossWindowTabDragRuntime(
			setupData.source,
			session(['a']),
			setupData.resolver,
			setupData.host,
		);
		runtime.start();
		runtime.update({ x: 200, y: 1 });
		expect(setupData.sourceModel.getSnapshot().tabs.map((tab) => tab.id)).toEqual(['b']);
		expect(setupData.targetModel.getSnapshot().tabs.map((tab) => tab.id)).toEqual(['x', 'a']);
		expect(setupData.source.draggedTabsDetached).toHaveBeenCalledOnce();
		expect(setupData.target.startedDragging).toHaveBeenCalledOnce();
		expect(runtime.getSnapshot()).toMatchObject({
			state: CrossWindowDragState.DraggingTabs,
			attachedContextId: 'target',
		});

		runtime.end('complete');
		expect(runtime.getSnapshot()).toMatchObject({
			state: CrossWindowDragState.Stopped,
			destination: 'existing-window',
		});
		expect(setupData.target.stoppedDragging).toHaveBeenCalledOnce();
	});

	it('continues drag-start compaction from global elapsed time after attachment', () => {
		const setupData = setup();
		const forceLayout = vi.fn<TabDragPositioningDelegate['forceLayout']>();
		const layoutDraggedViewsAt =
			vi.fn<
				(
					viewIds: readonly string[],
					sourceViewId: string,
					location: Readonly<{ x: number; y: number }>,
					initialDrag: boolean,
					dragStartAnimationProgress: number,
				) => void
			>();
		setupData.target.positioningDelegate = {
			updateAnimationTarget: vi.fn<TabDragPositioningDelegate['updateAnimationTarget']>(),
			isDragSessionActive: () => true,
			isAnimatingDragEnd: () => false,
			completeEndDragAnimations: vi.fn<TabDragPositioningDelegate['completeEndDragAnimations']>(),
			getTabDragAreaWidth: () => 400,
			getContext: () => setupData.target,
			getTabAt: () => null,
			isActiveDropTarget: () => true,
			tabDragAreaEndX: () => 400,
			tabDragAreaBeginX: () => 0,
			getInsertionIndexForDraggedBounds: () => 0,
			calculateBoundsForDraggedViews: () => [],
			setBoundsForDrag: vi.fn<TabDragPositioningDelegate['setBoundsForDrag']>(),
			layoutDraggedViewsAt,
			forceLayout,
		};
		const dragStartedAt = 100;
		const now = vi
			.spyOn(performance, 'now')
			.mockReturnValue(dragStartedAt + DRAG_START_ANIMATION_DURATION_MS / 2);
		const runtime = new CrossWindowTabDragRuntime(
			setupData.source,
			session(['a'], dragStartedAt),
			setupData.resolver,
			setupData.host,
		);

		runtime.update({ x: 200, y: 1 });
		expect(layoutDraggedViewsAt).toHaveBeenLastCalledWith(['a'], 'a', { x: 200, y: 1 }, false, 0.5);
		expect(forceLayout).toHaveBeenCalledOnce();
		expect(layoutDraggedViewsAt.mock.invocationCallOrder[0]).toBeLessThan(
			requireDefined(forceLayout.mock.invocationCallOrder[0]),
		);

		now.mockReturnValue(dragStartedAt + DRAG_START_ANIMATION_DURATION_MS + 1);
		runtime.update({ x: 210, y: 1 });
		expect(layoutDraggedViewsAt).toHaveBeenLastCalledWith(['a'], 'a', { x: 210, y: 1 }, false, 1);
		expect(forceLayout).toHaveBeenCalledOnce();
		now.mockRestore();
	});

	it('keeps drag-start compaction skipped after attachment', () => {
		const setupData = setup();
		const layoutDraggedViewsAt =
			vi.fn<
				(
					viewIds: readonly string[],
					sourceViewId: string,
					location: Readonly<{ x: number; y: number }>,
					initialDrag: boolean,
					dragStartAnimationProgress: number,
				) => void
			>();
		setupData.target.positioningDelegate = {
			updateAnimationTarget: vi.fn<TabDragPositioningDelegate['updateAnimationTarget']>(),
			isDragSessionActive: () => true,
			isAnimatingDragEnd: () => false,
			completeEndDragAnimations: vi.fn<TabDragPositioningDelegate['completeEndDragAnimations']>(),
			getTabDragAreaWidth: () => 400,
			getContext: () => setupData.target,
			getTabAt: () => null,
			isActiveDropTarget: () => true,
			tabDragAreaEndX: () => 400,
			tabDragAreaBeginX: () => 0,
			getInsertionIndexForDraggedBounds: () => 0,
			calculateBoundsForDraggedViews: () => [],
			setBoundsForDrag: vi.fn<TabDragPositioningDelegate['setBoundsForDrag']>(),
			layoutDraggedViewsAt,
			forceLayout: vi.fn<TabDragPositioningDelegate['forceLayout']>(),
		};
		const dragSession = session(['a']);
		dragSession.markDragStarted(100, false);
		const runtime = new CrossWindowTabDragRuntime(
			setupData.source,
			dragSession,
			setupData.resolver,
			setupData.host,
		);

		runtime.update({ x: 200, y: 1 });
		expect(layoutDraggedViewsAt).toHaveBeenLastCalledWith(['a'], 'a', { x: 200, y: 1 }, false, 1);
	});

	it('round-trips through the source snapshots when canceled', () => {
		const setupData = setup();
		const runtime = new CrossWindowTabDragRuntime(
			setupData.source,
			session(['a']),
			setupData.resolver,
			setupData.host,
		);
		runtime.update({ x: 200, y: 0 });
		runtime.end('cancel');
		expect(setupData.sourceModel.getSnapshot().tabs.map((tab) => tab.id)).toEqual(['a', 'b']);
		expect(setupData.sourceModel.getSnapshot().activeTabId).toBe('a');
		expect(setupData.targetModel.getSnapshot().tabs.map((tab) => tab.id)).toEqual(['x']);
		expect(runtime.getSnapshot()).toMatchObject({
			attachedContextId: 'source',
			destination: 'abandoned',
		});
	});

	it('runs target enter/update/drop/end lifecycle and supports target-owned detachment', () => {
		const preliminary = setup();
		const dragTarget = new FakeTarget(preliminary.target, true);
		const setupData = setup(dragTarget);
		// The target must return the context from the same setup used by runtime.
		const target = new FakeTarget(setupData.target, true);
		const resolver: TabDragPointResolver = {
			getDragTarget: (browserId) => (browserId === 'target' ? target : null),
		};
		const runtime = new CrossWindowTabDragRuntime(
			setupData.source,
			session(['a']),
			resolver,
			setupData.host,
		);

		runtime.update({ x: 200, y: 1 });
		runtime.update({ x: 210, y: 1 });
		expect(target.entered).toHaveBeenCalledOnce();
		expect(target.updated).toHaveBeenCalledTimes(2);
		expect(runtime.getSnapshot().targetActive).toBe(true);
		runtime.end('complete');
		expect(target.dropped).toHaveBeenCalledWith(
			expect.objectContaining({ tabs: [expect.objectContaining({ id: 'a' })] }),
		);
		expect(target.ended).toHaveBeenCalledOnce();
		expect(setupData.targetModel.getIndexOfTab('a')).toBeNull();
	});

	it('creates a detached browser context and labels it as a new-window drop', () => {
		const setupData = setup();
		const contexts = new Map<string, FakeContext>();
		const newModel = model([]);
		const created = new FakeContext('created', newModel, contexts);
		const host: CrossWindowDragHost = {
			getBrowserIdAtPoint: () => null,
			createBrowserContext: () => created,
		};
		const runtime = new CrossWindowTabDragRuntime(
			setupData.source,
			session(['a']),
			setupData.resolver,
			host,
		);
		runtime.update({ x: 500, y: 40 });
		runtime.end('complete');
		expect(newModel.getSnapshot().tabs.map((tab) => tab.id)).toEqual(['a']);
		expect(runtime.getSnapshot().destination).toBe('new-window');
	});

	it('ends immediately when a dragged tab is removed from the attached model', () => {
		const setupData = setup();
		const runtime = new CrossWindowTabDragRuntime(
			setupData.source,
			session(['a']),
			setupData.resolver,
			setupData.host,
		);
		runtime.update({ x: 200, y: 1 });

		setupData.targetModel.closeTab('a');

		expect(runtime.getSnapshot()).toMatchObject({
			state: CrossWindowDragState.Stopped,
			attachedContextId: 'target',
			destination: 'abandoned',
		});
		expect(setupData.target.stoppedDragging).toHaveBeenCalledOnce();
	});
});
