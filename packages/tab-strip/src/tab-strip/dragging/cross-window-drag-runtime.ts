import type { TabDragRestoreState, DetachedTabBundle } from '../model/tab-model';
import { DragSlotViewType, type DragSessionData } from './drag-session-data';
import type { DragPoint } from './dragging-tabs-session';
import type { TabDragContext } from './tab-drag-context';
import { getDragStartAnimationProgress } from './dragged-tabs-container-layout';
import type {
	TabDragPointResolver,
	TabDragTarget,
	TabDragTargetController,
} from './tab-drag-target';

export const CrossWindowDragState = {
	NotStarted: 'not-started',
	DraggingTabs: 'dragging-tabs',
	DraggingWindow: 'dragging-window',
	Stopped: 'stopped',
} as const;

export type CrossWindowDragState = (typeof CrossWindowDragState)[keyof typeof CrossWindowDragState];

export type CrossWindowDragEndReason = 'complete' | 'cancel' | 'tab-destroyed';

export interface CrossWindowDragHost {
	getBrowserIdAtPoint(pointInScreen: Readonly<DragPoint>): string | null;
	getContextAtPoint?(
		browserId: string,
		pointInScreen: Readonly<DragPoint>,
		request: Readonly<{
			sessionData: DragSessionData;
			sourceContext: TabDragContext;
		}>,
	): TabDragContext | null;
	createBrowserContext?(
		pointInScreen: Readonly<DragPoint>,
		sessionData: DragSessionData,
	): TabDragContext | null;
	getInsertionIndex?(
		context: TabDragContext,
		pointInScreen: Readonly<DragPoint>,
		sessionData: DragSessionData,
	): number;
	getTargetGroup?(
		context: TabDragContext,
		pointInScreen: Readonly<DragPoint>,
		sessionData: DragSessionData,
	): string | null | undefined;
	toContextPoint?(context: TabDragContext, pointInScreen: Readonly<DragPoint>): DragPoint;
	onContextEmptied?(context: TabDragContext): void;
}

export interface CrossWindowDragSnapshot {
	revision: number;
	state: CrossWindowDragState;
	sourceContextId: string;
	attachedContextId: string | null;
	targetActive: boolean;
	destination: 'same-window' | 'existing-window' | 'new-window' | 'abandoned' | null;
	endReason: CrossWindowDragEndReason | null;
}

type Listener = () => void;

/**
 * DOM-hosted execution layer for TabDragController's context/target handoff.
 * Native capture, move loops, and window discovery stay behind CrossWindowDragHost.
 */
export class CrossWindowTabDragRuntime implements TabDragTargetController<DetachedTabBundle> {
	private state: CrossWindowDragState = CrossWindowDragState.NotStarted;
	private attachedContext: TabDragContext | null;
	private currentTarget: TabDragTarget<DetachedTabBundle> | null = null;
	private targetDestroySubscription: (() => void) | null = null;
	private attachedModelSubscription: (() => void) | null = null;
	private internalModelMutationDepth = 0;
	private readonly tabIds: readonly string[];
	private readonly sourceInsertionIndex: number;
	private readonly sourceRestoreState: TabDragRestoreState;
	private readonly restoreStates = new Map<TabDragContext, TabDragRestoreState>();
	private readonly createdContexts = new Set<TabDragContext>();
	private revision = 0;
	private destination: CrossWindowDragSnapshot['destination'] = null;
	private endReason: CrossWindowDragEndReason | null = null;
	private snapshot: CrossWindowDragSnapshot;
	private readonly listeners = new Set<Listener>();
	private readonly sourceContext: TabDragContext;
	private readonly sessionData: DragSessionData;
	private readonly pointResolver: TabDragPointResolver<DetachedTabBundle>;
	private readonly host: CrossWindowDragHost;

	public constructor(
		sourceContext: TabDragContext,
		sessionData: DragSessionData,
		pointResolver: TabDragPointResolver<DetachedTabBundle>,
		host: CrossWindowDragHost,
	) {
		this.sourceContext = sourceContext;
		this.sessionData = sessionData;
		this.pointResolver = pointResolver;
		this.host = host;
		this.attachedContext = sourceContext;
		this.tabIds = sessionData.tabDragData.flatMap((datum) => {
			if (datum.viewType !== DragSlotViewType.Tab) return [];
			const id =
				datum.contentsId === null
					? datum.attachedViewId
					: sourceContext.getTabForContents(datum.contentsId);
			return id ? [id] : [];
		});
		if (this.tabIds.length !== sessionData.numDraggingTabs) {
			throw new Error('Every dragged tab must resolve in the source context.');
		}
		this.sourceInsertionIndex = Math.min(
			...this.tabIds.map((id) => sourceContext.getTabStripModel().getIndexOfTab(id) ?? 0),
		);
		this.sourceRestoreState = sourceContext.getTabStripModel().captureDragRestoreState();
		this.restoreStates.set(sourceContext, this.sourceRestoreState);
		this.snapshot = this.createSnapshot();
	}

	public getSnapshot = (): CrossWindowDragSnapshot => this.snapshot;

	public subscribe = (listener: Listener): (() => void) => {
		this.listeners.add(listener);
		return () => this.listeners.delete(listener);
	};

	public start(): void {
		if (this.state !== CrossWindowDragState.NotStarted || !this.attachedContext) return;
		this.sessionData.markDragStarted();
		this.state = CrossWindowDragState.DraggingTabs;
		this.observeAttachedModel();
		this.attachedContext.startedDragging(this.getAttachedViewIds(this.attachedContext));
		this.commit();
	}

	public update(pointInScreen: Readonly<DragPoint>): void {
		if (this.state === CrossWindowDragState.Stopped) return;
		if (this.state === CrossWindowDragState.NotStarted) this.start();
		const browserId = this.host.getBrowserIdAtPoint(pointInScreen);
		const nextTarget =
			browserId === null ? null : this.pointResolver.getDragTarget(browserId, pointInScreen);
		this.updateTarget(nextTarget, pointInScreen);

		let targetContext: TabDragContext | null = null;
		if (this.currentTarget) {
			targetContext = this.currentTarget.onTabDragUpdated(this, pointInScreen);
			if (!targetContext && this.state === CrossWindowDragState.DraggingWindow)
				targetContext = this.attachedContext;
		} else if (browserId !== null) {
			if (this.host.getContextAtPoint) {
				targetContext = this.host.getContextAtPoint(browserId, pointInScreen, {
					sessionData: this.sessionData,
					sourceContext: this.sourceContext,
				});
			} else {
				targetContext =
					this.attachedContext?.id === browserId
						? this.attachedContext
						: this.sourceContext.getContextForNewBrowser(browserId);
			}
		} else if (this.host.createBrowserContext) {
			targetContext = this.host.createBrowserContext(pointInScreen, this.sessionData);
			if (targetContext) this.createdContexts.add(targetContext);
		}

		if (!targetContext) {
			if (this.state !== CrossWindowDragState.DraggingWindow) {
				this.state = CrossWindowDragState.DraggingWindow;
				this.commit();
			}
			return;
		}

		const justAttached =
			targetContext !== this.attachedContext
				? this.attachToContext(targetContext, pointInScreen)
				: false;
		if (targetContext !== this.attachedContext) {
			this.state = CrossWindowDragState.DraggingWindow;
			this.commit();
			return;
		}
		this.state = CrossWindowDragState.DraggingTabs;
		this.layoutAttached(pointInScreen);
		if (justAttached) targetContext.getPositioningDelegate()?.forceLayout();
		this.commit();
	}

	public end(reason: CrossWindowDragEndReason): void {
		if (this.state === CrossWindowDragState.Stopped) return;
		this.endReason = reason;
		if (reason === 'cancel') {
			this.revertDrag();
			this.destination = 'abandoned';
		} else if (reason === 'complete') {
			if (this.currentTarget?.canDropTab()) this.currentTarget.handleTabDrop(this);
			if (this.attachedContext === this.sourceContext) this.destination = 'same-window';
			else if (this.attachedContext && this.createdContexts.has(this.attachedContext)) {
				this.destination = 'new-window';
			} else this.destination = 'existing-window';
		} else {
			this.destination = 'abandoned';
		}

		this.state = CrossWindowDragState.Stopped;
		this.attachedContext?.stoppedDragging();
		this.stopObservingAttachedModel();
		if (this.currentTarget) this.currentTarget.onTabDragEnded();
		this.resetTarget();
		this.commit();
	}

	public detachTabAtForInsertion(dragIndex: number): DetachedTabBundle {
		const datum = this.sessionData.tabDragData[dragIndex];
		if (!datum || datum.viewType !== DragSlotViewType.Tab || !this.attachedContext) {
			throw new Error('dragIndex must identify an attached dragged tab.');
		}
		const tabId =
			datum.contentsId === null
				? datum.attachedViewId
				: this.attachedContext.getTabForContents(datum.contentsId);
		if (!tabId) throw new Error('The dragged tab no longer exists in the attached context.');
		const bundle = this.runInternalModelMutation(() =>
			this.attachedContext?.getTabStripModel().detachTabsForTransfer([tabId]),
		);
		if (!bundle) throw new Error('The dragged tab could not be detached.');
		return bundle;
	}

	public getSessionData(): DragSessionData {
		return this.sessionData;
	}

	public getAttachedContext(): TabDragContext | null {
		return this.attachedContext;
	}

	private attachToContext(
		targetContext: TabDragContext,
		pointInScreen: Readonly<DragPoint>,
	): boolean {
		const previous = this.attachedContext;
		if (!previous) return false;
		if (!this.restoreStates.has(targetContext)) {
			this.restoreStates.set(
				targetContext,
				targetContext.getTabStripModel().captureDragRestoreState(),
			);
		}
		const targetIndex =
			this.host.getInsertionIndex?.(targetContext, pointInScreen, this.sessionData) ??
			targetContext.getTabStripModel().getSnapshot().tabs.length;
		const targetGroup = this.host.getTargetGroup?.(targetContext, pointInScreen, this.sessionData);
		const transferOptions =
			targetGroup === undefined ? { index: targetIndex } : { index: targetIndex, targetGroup };
		const moved = this.runInternalModelMutation(() =>
			previous
				.getTabStripModel()
				.transferTabsTo(targetContext.getTabStripModel(), this.tabIds, transferOptions),
		);
		if (!moved) return false;
		previous.draggedTabsDetached();
		if (previous.getTabStripModel().getSnapshot().tabs.length === 0)
			this.host.onContextEmptied?.(previous);
		this.attachedContext = targetContext;
		this.observeAttachedModel();
		targetContext.startedDragging(this.getAttachedViewIds(targetContext));
		return true;
	}

	private layoutAttached(pointInScreen: Readonly<DragPoint>): void {
		const context = this.attachedContext;
		const delegate = context?.getPositioningDelegate();
		if (!context || !delegate) return;
		const ids = this.getAttachedViewIds(context);
		const sourceDatum = this.sessionData.sourceViewDragData;
		const sourceId =
			sourceDatum.contentsId === null
				? sourceDatum.attachedViewId
				: context.getTabForContents(sourceDatum.contentsId);
		if (!sourceId) return;
		delegate.layoutDraggedViewsAt(
			ids,
			sourceId,
			this.host.toContextPoint?.(context, pointInScreen) ?? pointInScreen,
			false,
			this.sessionData.dragStartAnimationRequired
				? getDragStartAnimationProgress(this.sessionData.dragStartTime, performance.now())
				: 1,
		);
	}

	private revertDrag(): void {
		const attached = this.attachedContext;
		if (attached && attached !== this.sourceContext) {
			this.runInternalModelMutation(() =>
				attached
					.getTabStripModel()
					.transferTabsTo(this.sourceContext.getTabStripModel(), this.tabIds, {
						index: this.sourceInsertionIndex,
					}),
			);
			attached.draggedTabsDetached();
			this.attachedContext = this.sourceContext;
			this.observeAttachedModel();
			this.sourceContext.startedDragging(this.getAttachedViewIds(this.sourceContext));
		}
		for (const [context, restoreState] of this.restoreStates) {
			context.getTabStripModel().restoreCanceledDrag(restoreState);
		}
	}

	private getAttachedViewIds(context: TabDragContext): string[] {
		return this.sessionData.tabDragData.flatMap((datum) => {
			if (datum.viewType === DragSlotViewType.GroupHeader) {
				const header = datum.tabGroupData
					? context.getTabGroupHeader(datum.tabGroupData.groupId)
					: null;
				return header ? [header] : [];
			}
			const tab =
				datum.contentsId === null
					? datum.attachedViewId
					: context.getTabForContents(datum.contentsId);
			return tab ? [tab] : [];
		});
	}

	private updateTarget(
		nextTarget: TabDragTarget<DetachedTabBundle> | null,
		point: Readonly<DragPoint>,
	): void {
		if (nextTarget === this.currentTarget) return;
		if (this.currentTarget) this.currentTarget.onTabDragExited(point);
		this.resetTarget();
		this.currentTarget = nextTarget;
		if (!nextTarget) return;
		nextTarget.onTabDragEntered();
		this.targetDestroySubscription = nextTarget.registerWillDestroyCallback(() =>
			this.resetTarget(),
		);
	}

	private resetTarget(): void {
		this.targetDestroySubscription?.();
		this.targetDestroySubscription = null;
		this.currentTarget = null;
	}

	private observeAttachedModel(): void {
		this.stopObservingAttachedModel();
		const context = this.attachedContext;
		if (!context) return;
		this.attachedModelSubscription = context.getTabStripModel().subscribe(() => {
			if (
				this.internalModelMutationDepth > 0 ||
				this.state === CrossWindowDragState.NotStarted ||
				this.state === CrossWindowDragState.Stopped
			) {
				return;
			}
			const liveIds = new Set(
				context
					.getTabStripModel()
					.getSnapshot()
					.tabs.map((tab) => tab.id),
			);
			if (this.tabIds.every((id) => liveIds.has(id))) return;
			this.end('tab-destroyed');
		});
	}

	private stopObservingAttachedModel(): void {
		this.attachedModelSubscription?.();
		this.attachedModelSubscription = null;
	}

	private runInternalModelMutation<T>(mutation: () => T): T {
		this.internalModelMutationDepth += 1;
		try {
			return mutation();
		} finally {
			this.internalModelMutationDepth -= 1;
		}
	}

	private createSnapshot(): CrossWindowDragSnapshot {
		return Object.freeze({
			revision: this.revision,
			state: this.state,
			sourceContextId: this.sourceContext.id,
			attachedContextId: this.attachedContext?.id ?? null,
			targetActive: this.currentTarget !== null,
			destination: this.destination,
			endReason: this.endReason,
		});
	}

	private commit(): void {
		this.revision += 1;
		this.snapshot = this.createSnapshot();
		this.listeners.forEach((listener) => listener());
	}
}
