import { describe, expect, it, vi } from 'vitest';
import { TabModel, type DetachedTabBundle } from '../../model';
import { DragSessionData, DragSlotViewType } from '../drag-session-data';
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
		return null;
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

function session(tabIds: string[]) {
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
});
