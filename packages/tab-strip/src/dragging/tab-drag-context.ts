import type { Rect } from '../geometry';
import type { TabGroupId, TabModel } from '../model';
import type { DragPoint } from './dragging-tabs-session';
export interface TabDragPositioningDelegateBase {
	updateAnimationTarget(tabSlotId: string, targetBounds: Readonly<Rect>): void;
	isDragSessionActive(): boolean;
	isAnimatingDragEnd(): boolean;
	completeEndDragAnimations(): void;
	getTabDragAreaWidth(): number;
	getContext(): TabDragContext;
}
export interface TabDragPositioningDelegate extends TabDragPositioningDelegateBase {
	getTabAt(index: number): string | null;
	isActiveDropTarget(): boolean;
	tabDragAreaEndX(): number;
	tabDragAreaBeginX(): number;
	getInsertionIndexForDraggedBounds(
		draggedBounds: Readonly<Rect>,
		draggedViewIds: readonly string[],
		numberOfDraggedTabs: number,
	): number;
	calculateBoundsForDraggedViews(viewIds: readonly string[]): Rect[];
	setBoundsForDrag(viewIds: readonly string[], bounds: readonly Readonly<Rect>[]): void;
	layoutDraggedViewsAt(
		viewIds: readonly string[],
		sourceViewId: string,
		location: Readonly<DragPoint>,
		initialDrag: boolean,
	): void;
	forceLayout(): void;
}

/** DOM/browser adapter contract corresponding to dragging/TabDragContext. */
export interface TabDragContext {
	readonly id: string;
	getContextForNewBrowser(browserId: string): TabDragContext | null;
	getTabForContents(contentsId: string): string | null;
	getContentsForTab(tabSlotId: string): string | null;
	isTabDetachable(tabSlotId: string): boolean;
	getTabGroupHeader(groupId: TabGroupId): string | null;
	getTabStripModel(): TabModel;
	startedDragging(viewIds: readonly string[]): void;
	draggedTabsDetached(): void;
	stoppedDragging(): void;
	getPositioningDelegate(): TabDragPositioningDelegate | null;
}
