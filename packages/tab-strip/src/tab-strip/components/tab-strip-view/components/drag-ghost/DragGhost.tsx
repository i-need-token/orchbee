import styles from './DragGhost.module.css';
import { type CSSProperties, type ReactNode } from 'react';
import { type TabModelSnapshot } from '../../../../model/tab-model';
import {
	buildDraggedTabsLayout,
	clampDraggedViewPosition,
	getAnimatedDraggedViewPosition,
	getDragStartAnimationOffset,
} from '../../../../dragging/dragged-tabs-container-layout';
import { TabStripOrientation } from '../../../../types/tab-strip-types';
import { LAYOUT_CONSTANTS } from '../../../../layout/layout-constants';
import { tabCollectionEaseInOut } from '../../../../layout/tab-collection-animating-layout-manager';
import {
	GROUP_COLORS,
	getSourceVisualId,
	getVerticalDragVisualViews,
	type DragState,
} from '../../../../utils/tab-strip-internals';
import { VerticalDragTabVisual } from './components/vertical-drag-tab-visual/VerticalDragTabVisual';
import { VerticalDragGroupVisual } from './components/vertical-drag-group-visual/VerticalDragGroupVisual';

export function DragGhost({
	drag,
	snapshot,
	orientation,
	collapsed,
	touchUi,
	closeButtonAllowed,
}: {
	drag: DragState;
	snapshot: TabModelSnapshot;
	orientation: TabStripOrientation;
	collapsed: boolean;
	touchUi: boolean;
	closeButtonAllowed: boolean;
}): ReactNode {
	const tabs = drag.tabIds.flatMap((id) => {
		const tab = snapshot.tabs.find((candidate) => candidate.id === id);
		return tab ? [tab] : [];
	});
	const vertical = orientation === TabStripOrientation.Vertical;
	const sourceTab = snapshot.tabs.find((tab) => tab.id === drag.sourceId) ?? null;
	const squash = vertical && (collapsed || sourceTab?.pinned === true);
	const sourceVisualId = getSourceVisualId(drag);
	const group =
		drag.groupHeaderId === null
			? null
			: (snapshot.groups.find((candidate) => candidate.id === drag.groupHeaderId) ?? null);
	const tabById = new Map(tabs.map((tab) => [tab.id, tab]));

	if (vertical) {
		const verticalVisualViews = getVerticalDragVisualViews(drag);
		const layout = buildDraggedTabsLayout({
			layout: squash ? 'squash' : 'vertical',
			pointerOffsetRatios: drag.pointerOffsetRatios,
			sourceSlotSize: drag.sourceSlotSize,
			sourceViewOffsetFromSlot: drag.sourceViewOffsetFromSlot,
			views: verticalVisualViews.map((view) => ({
				id: view.id,
				bounds: { x: view.x, y: view.y, width: view.width, height: view.height },
				source: view.id === sourceVisualId,
			})),
		});
		const visualData = new Map(layout.views.map((view) => [view.id, view]));
		const shownViews = verticalVisualViews.filter(
			(view) => !visualData.get(view.id)?.shouldHide || drag.dragStartAnimationProgress < 1,
		);
		const sourceView =
			verticalVisualViews.find((view) => view.id === sourceVisualId) ?? verticalVisualViews[0];
		const sourceVisual = sourceView ? visualData.get(sourceView.id) : undefined;
		const dragStartAnimationValue = tabCollectionEaseInOut(drag.dragStartAnimationProgress);
		const canMoveHorizontally = sourceTab?.pinned === true && !collapsed;
		const unclampedLeft = canMoveHorizontally
			? drag.x + layout.boundsRelativeToPointer.x
			: drag.startX - drag.pointerOffsetFromVisualOrigin.x;
		const unclampedTop = drag.y + layout.boundsRelativeToPointer.y;
		const clampingRightPadding = LAYOUT_CONSTANTS.verticalTabStripHorizontalPadding;
		const ghostLeft = Math.min(
			Math.max(
				drag.dragAreaBounds.left,
				drag.dragAreaBounds.left +
					drag.dragAreaBounds.width -
					clampingRightPadding -
					layout.boundsRelativeToPointer.width,
			),
			Math.max(drag.dragAreaBounds.left, unclampedLeft),
		);
		const ghostTop = Math.min(
			Math.max(
				drag.dragAreaBounds.top,
				drag.dragAreaBounds.top +
					drag.dragAreaBounds.height -
					layout.boundsRelativeToPointer.height,
			),
			Math.max(drag.dragAreaBounds.top, unclampedTop),
		);
		return (
			<div
				className={styles['ct-drag-ghost']}
				data-tab-strip-drag-ghost=''
				data-layout={squash ? 'squash' : 'vertical'}
				style={
					{
						'--ob-comp-tab-strip-inline-start': `${ghostLeft}px`,
						'--ob-comp-tab-strip-block-start': `${ghostTop}px`,
						'--ob-comp-tab-strip-inline-size': `${layout.boundsRelativeToPointer.width}px`,
						'--ob-comp-tab-strip-block-size': `${layout.boundsRelativeToPointer.height}px`,
						'--ob-comp-tab-strip-group-color': group ? GROUP_COLORS[group.color] : undefined,
					} satisfies CSSProperties
				}
				aria-hidden='true'
			>
				{shownViews.map((view) => {
					const visual = visualData.get(view.id);
					if (!visual) return null;
					const dragStartOffset =
						sourceView && sourceVisual
							? getDragStartAnimationOffset({
									view,
									sourceView,
									targetOffset: visual.offset,
									sourceTargetOffset: sourceVisual.offset,
								})
							: { x: 0, y: 0 };
					const position = getAnimatedDraggedViewPosition({
						axes: 'both',
						draggingBounds: {
							x: ghostLeft,
							y: ghostTop,
							width: layout.boundsRelativeToPointer.width,
							height: layout.boundsRelativeToPointer.height,
						},
						targetOffset: visual.offset,
						animationStartOffset: dragStartOffset,
						animationValue: dragStartAnimationValue,
					});
					const clamped = clampDraggedViewPosition(
						position,
						{ width: view.width, height: view.height },
						{
							x: drag.dragAreaBounds.left,
							y: drag.dragAreaBounds.top,
							width: drag.dragAreaBounds.width,
							height: drag.dragAreaBounds.height,
						},
					);
					const compacting =
						drag.dragStartAnimationProgress < 1 &&
						(dragStartOffset.x !== 0 || dragStartOffset.y !== 0);
					const tab =
						view.kind === 'tab'
							? (tabById.get(view.id) ?? tabById.get(view.tabIds?.[0] ?? '') ?? null)
							: null;
					const draggedGroup =
						view.kind === 'group'
							? (snapshot.groups.find((candidate) => `group:${candidate.id}` === view.id) ?? null)
							: null;
					const draggedGroupTabs = draggedGroup
						? (view.tabIds ?? []).flatMap((id) => {
								const candidate = tabById.get(id);
								return candidate ? [candidate] : [];
							})
						: [];
					const groupViewClassName = draggedGroup
						? styles['ct-drag-ghost__group-view']
						: styles['ct-drag-ghost__group'];
					const viewClassName =
						view.kind === 'group' ? groupViewClassName : styles['ct-drag-ghost__tab-view'];
					let viewContent: ReactNode = <span>{drag.label}</span>;
					if (view.kind === 'group' && draggedGroup)
						viewContent = (
							<VerticalDragGroupVisual
								group={draggedGroup}
								collapsed={
									drag.groupHeaderId === draggedGroup.id
										? drag.groupWasCollapsedBeforeDrag
										: draggedGroup.collapsed
								}
								tabs={draggedGroupTabs}
								snapshot={snapshot}
								width={view.width}
								collapsedStrip={collapsed}
								touchUi={touchUi}
								closeButtonAllowed={closeButtonAllowed}
							/>
						);
					else if (view.kind !== 'group' && tab)
						viewContent = (
							<VerticalDragTabVisual
								tab={tab}
								snapshot={snapshot}
								width={view.width}
								collapsed={collapsed}
								touchUi={touchUi}
								closeButtonAllowed={closeButtonAllowed}
							/>
						);
					return (
						<div
							key={view.id}
							className={viewClassName}
							data-drag-view={view.kind === 'group' && draggedGroup ? 'group-visual' : view.kind}
							data-vertical-drag-view={
								view.kind === 'group' && draggedGroup ? 'group-visual' : view.kind
							}
							data-source={view.id === sourceVisualId || undefined}
							data-compacting={compacting || undefined}
							style={
								{
									'--ob-comp-tab-strip-inline-start': `${clamped.x - ghostLeft}px`,
									'--ob-comp-tab-strip-block-start': `${clamped.y - ghostTop}px`,
									'--ob-comp-tab-strip-inline-size': `${view.width}px`,
									'--ob-comp-tab-strip-block-size': `${view.height}px`,
								} satisfies CSSProperties
							}
						>
							{viewContent}
						</div>
					);
				})}
				{squash && tabs.length > 1 ? (
					<span className={styles['ct-drag-ghost__count']}>{tabs.length}</span>
				) : null}
			</div>
		);
	}

	return null;
}
