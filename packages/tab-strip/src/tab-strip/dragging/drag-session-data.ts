import { requireDefined } from '../utils/requireDefined';
import type { TabGroupId } from '../model/tab-types';
export const DragSlotViewType = {
	Tab: 'tab',
	GroupHeader: 'group-header',
} as const;
export type DragSlotViewType = (typeof DragSlotViewType)[keyof typeof DragSlotViewType];
export interface GroupHeaderDragData {
	group: TabGroupId;
	activeTabIndexWithinGroup: number;
	wasCollapsedFromDrag: boolean;
}
export interface TabDragGroupData<TVisualData = unknown> {
	groupId: TabGroupId;
	groupVisualData: TVisualData;
	indexInGroup: number;
}
export interface TabDragData<TVisualData = unknown> {
	sourceModelIndex: number | null;
	viewType: DragSlotViewType;
	contentsId: string | null;
	attachedViewId: string | null;
	pinned: boolean;
	tabGroupData: TabDragGroupData<TVisualData> | null;
}
export interface DragSessionDataInit<TVisualData = unknown> {
	tabDragData: readonly TabDragData<TVisualData>[];
	groupHeaderDragData?: GroupHeaderDragData | null;
	draggingGroups?: ReadonlySet<TabGroupId>;
	sourceViewIndex: number;
	mouseOffsetToSizeRatios: {
		x: number;
		y: number;
	};
	dragStartTime?: number;
	dragStartAnimationRequired?: boolean;
}

/** Data-only port of dragging/DragSessionData and its convenience helpers. */
export class DragSessionData<TVisualData = unknown> {
	public readonly tabDragData: readonly Readonly<TabDragData<TVisualData>>[];
	public readonly groupHeaderDragData: Readonly<GroupHeaderDragData> | null;
	public readonly draggingGroups: ReadonlySet<TabGroupId>;
	public readonly sourceViewIndex: number;
	public readonly mouseOffsetToSizeRatios: Readonly<{
		x: number;
		y: number;
	}>;
	private dragStarted: boolean;
	private dragStartedAt: number;
	private shouldAnimateDragStart: boolean;
	public constructor(init: DragSessionDataInit<TVisualData>) {
		if (init.tabDragData.length === 0)
			throw new Error('A drag session requires at least one dragged view.');
		if (init.sourceViewIndex < 0 || init.sourceViewIndex >= init.tabDragData.length) {
			throw new Error('sourceViewIndex must identify one of the dragged views.');
		}
		this.tabDragData = init.tabDragData.map((data) =>
			Object.freeze({
				...data,
				tabGroupData: data.tabGroupData
					? Object.freeze({
							...data.tabGroupData,
						})
					: null,
			}),
		);
		this.groupHeaderDragData = init.groupHeaderDragData
			? Object.freeze({
					...init.groupHeaderDragData,
				})
			: null;
		this.draggingGroups = new Set(init.draggingGroups ?? []);
		this.sourceViewIndex = init.sourceViewIndex;
		this.mouseOffsetToSizeRatios = Object.freeze({
			...init.mouseOffsetToSizeRatios,
		});
		this.dragStarted = init.dragStartTime !== undefined;
		this.dragStartedAt = init.dragStartTime ?? 0;
		this.shouldAnimateDragStart =
			init.dragStartTime !== undefined && (init.dragStartAnimationRequired ?? true);
	}
	public get dragStartTime(): number {
		return this.dragStartedAt;
	}
	public get hasDragStarted(): boolean {
		return this.dragStarted;
	}
	public get dragStartAnimationRequired(): boolean {
		return this.shouldAnimateDragStart;
	}
	public markDragStarted(now = performance.now(), dragStartAnimationRequired = true): void {
		if (this.dragStarted) return;
		this.dragStarted = true;
		this.dragStartedAt = now;
		this.shouldAnimateDragStart = dragStartAnimationRequired;
	}
	public get groupHeaderId(): TabGroupId | null {
		return this.groupHeaderDragData?.group ?? null;
	}
	public get sourceViewDragData(): Readonly<TabDragData<TVisualData>> {
		return requireDefined(this.tabDragData[this.sourceViewIndex]);
	}
	public get sourceDraggedContentsId(): string | null {
		return this.sourceViewDragData.contentsId;
	}
	public get numDraggingTabs(): number {
		return this.tabDragData.filter((data) => data.viewType === DragSlotViewType.Tab).length;
	}
	public get attachedViewIds(): string[] {
		return this.tabDragData.flatMap((data) => (data.attachedViewId ? [data.attachedViewId] : []));
	}
}
