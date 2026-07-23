import type { DragSessionData } from './drag-session-data';
import type { DragPoint } from './dragging-tabs-session';
import type { TabDragContext } from './tab-drag-context';

export interface TabDragTargetController<TDetachedTab = unknown> {
	detachTabAtForInsertion(dragIndex: number): TDetachedTab;
	getSessionData(): DragSessionData;
	getAttachedContext(): TabDragContext | null;
}

/** Platform-neutral port of dragging/TabDragTarget. */
export interface TabDragTarget<TDetachedTab = unknown> {
	onTabDragEntered(): void;
	onTabDragUpdated(
		controller: TabDragTargetController<TDetachedTab>,
		pointInScreen: Readonly<DragPoint>,
	): TabDragContext | null;
	onTabDragExited(pointInScreen: Readonly<DragPoint>): void;
	onTabDragEnded(): void;
	canDropTab(): boolean;
	handleTabDrop(controller: TabDragTargetController<TDetachedTab>): void;
	registerWillDestroyCallback(callback: () => void): () => void;
}

export interface TabDragPointResolver<TDetachedTab = unknown> {
	getDragTarget(
		browserId: string,
		pointInScreen: Readonly<DragPoint>,
	): TabDragTarget<TDetachedTab> | null;
}
