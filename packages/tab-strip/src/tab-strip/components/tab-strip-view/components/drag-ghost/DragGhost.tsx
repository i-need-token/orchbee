import styles from './DragGhost.module.css';
import { type CSSProperties, type ReactNode } from 'react';
import { type TabModelSnapshot } from '../../../../model/tab-model';
import { buildDraggedTabsLayout } from '../../../../dragging/dragged-tabs-container-layout';
import { TabStripOrientation } from '../../../../types/tab-strip-types';
import { LAYOUT_CONSTANTS } from '../../../../layout/layout-constants';
import { TAB_SURFACE_HORIZONTAL_INSET } from '../../../../layout/tab-style-views';
import {
	GROUP_COLORS,
	packHorizontalDragViews,
	getSourceVisualId,
	getVerticalDragVisualViews,
	groupHeaderWidth,
	tabIsApparentlyActive,
	type DragState,
} from '../../../../utils/tab-strip-internals';
import { calculateTabContainerZValue } from '../../../../layout/z-orderable-tab-container-element';
import { TabButton } from '../tab-button/TabButton';
import { GroupHeader } from '../group-header/GroupHeader';
import { VerticalDragTabVisual } from './components/vertical-drag-tab-visual/VerticalDragTabVisual';
import { VerticalDragGroupVisual } from './components/vertical-drag-group-visual/VerticalDragGroupVisual';

export function DragGhost({
	drag,
	snapshot,
	orientation,
	collapsed,
	settleTargetLeft,
	touchUi,
	closeButtonAllowed,
	declutterEnabled,
}: {
	drag: DragState;
	snapshot: TabModelSnapshot;
	orientation: TabStripOrientation;
	collapsed: boolean;
	settleTargetLeft?: number | null;
	touchUi: boolean;
	closeButtonAllowed: boolean;
	declutterEnabled: boolean;
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
	const originalGroupByTabId = new Map(drag.restoreState.tabs.map((tab) => [tab.id, tab.group]));

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
		const shownViews = verticalVisualViews.filter((view) => !visualData.get(view.id)?.shouldHide);
		const sourceView =
			verticalVisualViews.find((view) => view.id === sourceVisualId) ?? verticalVisualViews[0];
		const sourceVisual = sourceView ? visualData.get(sourceView.id) : undefined;
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
					const dragStartOffsetX =
						sourceView && sourceVisual
							? view.x - sourceView.x - (visual.offset.x - sourceVisual.offset.x)
							: 0;
					const dragStartOffsetY =
						sourceView && sourceVisual
							? view.y - sourceView.y - (visual.offset.y - sourceVisual.offset.y)
							: 0;
					const compacting = dragStartOffsetX !== 0 || dragStartOffsetY !== 0;
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
									'--ob-comp-tab-strip-inline-start': `${visual.offset.x}px`,
									'--ob-comp-tab-strip-block-start': `${visual.offset.y}px`,
									'--ob-comp-tab-strip-inline-size': `${view.width}px`,
									'--ob-comp-tab-strip-block-size': `${view.height}px`,
									'--ob-comp-tab-strip-drag-initial-offset-x': `${dragStartOffsetX}px`,
									'--ob-comp-tab-strip-drag-initial-offset-y': `${dragStartOffsetY}px`,
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

	// TabStrip::MaybeStartDrag always places the header and every group tab in
	// the drag session. kCollapseTabGroupDuringDrag only collapses the backing
	// model while an originally-expanded group is in flight; those captured tab
	// views remain visible and move as one unit. A group that was already
	// collapsed is the only header drag whose painted surface stays compact.
	const compactGroupHeaderDrag = drag.groupHeaderId !== null && drag.groupWasCollapsedBeforeDrag;
	const horizontalSourceViews = compactGroupHeaderDrag
		? drag.visualViews.filter((view) => view.kind === 'group')
		: drag.visualViews;
	const packedHorizontalDrag = packHorizontalDragViews({
		views: horizontalSourceViews,
		sourceVisualId,
		pointerRatioX: drag.pointerOffsetRatios.x,
		...(compactGroupHeaderDrag && group
			? { sourceWidthOverride: groupHeaderWidth(group, tabs.length) }
			: {}),
	});
	const horizontalViews = packedHorizontalDrag.views;
	const width = packedHorizontalDrag.width;
	const height = LAYOUT_CONSTANTS.tabStripHeight;
	const horizontalMouseOffset =
		packedHorizontalDrag.mouseOffset || drag.pointerOffsetFromVisualOrigin.x;
	const unclampedGhostLeft =
		drag.settling && settleTargetLeft !== null && settleTargetLeft !== undefined
			? settleTargetLeft
			: drag.x - horizontalMouseOffset;
	const ghostLeft = Math.min(
		Math.max(
			drag.dragAreaBounds.left,
			drag.dragAreaBounds.left + drag.dragAreaBounds.width - width,
		),
		Math.max(drag.dragAreaBounds.left, unclampedGhostLeft),
	);
	const draggedGroupUnderlines = snapshot.groups.flatMap((draggedGroup) => {
		const groupViews = horizontalViews.filter((view) =>
			view.kind === 'group'
				? view.id === `group:${draggedGroup.id}`
				: tabById.get(view.id)?.group === draggedGroup.id,
		);
		if (groupViews.length === 0) return [];
		const hasDraggedTabFromGroup = groupViews.some(
			(view) => view.kind === 'tab' && originalGroupByTabId.get(view.id) === draggedGroup.id,
		);
		const leading = groupViews.reduce((result, view) => (view.x < result.x ? view : result));
		const trailing = groupViews.reduce((result, view) =>
			view.x + view.width > result.x + result.width ? view : result,
		);
		const leadingTab = leading.kind === 'tab' ? tabById.get(leading.id) : null;
		const trailingTab = trailing.kind === 'tab' ? tabById.get(trailing.id) : null;
		const naturalUnderlineLeft =
			leading.x +
			(leadingTab && tabIsApparentlyActive(leadingTab, snapshot)
				? TAB_SURFACE_HORIZONTAL_INSET
				: 18);
		const naturalUnderlineRight =
			trailing.x +
			trailing.width +
			(trailingTab && tabIsApparentlyActive(trailingTab, snapshot)
				? -TAB_SURFACE_HORIZONTAL_INSET
				: -18);
		const draggedUnderline = {
			x: naturalUnderlineLeft,
			width: Math.max(0, naturalUnderlineRight - naturalUnderlineLeft),
		};
		return [
			{
				group: draggedGroup,
				underline: hasDraggedTabFromGroup ? draggedUnderline : { x: 0, width: 0 },
			},
		];
	});
	return (
		<div
			className={styles['ct-drag-ghost']}
			data-tab-strip-drag-ghost=''
			data-layout='horizontal'
			data-settling={drag.settling || undefined}
			style={
				{
					'--ob-comp-tab-strip-inline-start': `${ghostLeft}px`,
					'--ob-comp-tab-strip-block-start': `${drag.dragAreaBounds.top}px`,
					'--ob-comp-tab-strip-inline-size': `${width}px`,
					'--ob-comp-tab-strip-block-size': `${height}px`,
					'--ob-comp-tab-strip-runtime-layer': calculateTabContainerZValue({
						kind: 'group-header',
						active: true,
					}),
				} satisfies CSSProperties
			}
			aria-hidden='true'
		>
			{horizontalViews.map((view) => {
				const tab = view.kind === 'tab' ? tabById.get(view.id) : null;
				const draggedGroupId = view.kind === 'group' ? view.id.slice('group:'.length) : null;
				const draggedGroup =
					draggedGroupId === null
						? null
						: (snapshot.groups.find((candidate) => candidate.id === draggedGroupId) ?? null);
				return (
					<div
						key={view.id}
						className={
							view.kind === 'group' ? styles['ct-drag-ghost__group'] : styles['ct-drag-ghost__item']
						}
						data-drag-view={view.kind}
						data-horizontal-drag-view={view.kind}
						data-source={view.id === sourceVisualId || undefined}
						data-compacting={view.initialOffsetX !== 0 || undefined}
						style={
							{
								'--ob-comp-tab-strip-inline-start': `${view.x}px`,
								'--ob-comp-tab-strip-block-start': `${view.y}px`,
								'--ob-comp-tab-strip-inline-size': `${view.width}px`,
								'--ob-comp-tab-strip-block-size': `${view.height}px`,
								'--ob-comp-tab-strip-drag-initial-offset-x': `${view.initialOffsetX}px`,
							} satisfies CSSProperties
						}
					>
						{view.kind === 'group' && draggedGroup ? (
							<GroupHeader
								group={draggedGroup}
								tabs={snapshot.tabs.filter((candidate) => candidate.group === draggedGroup.id)}
								activeTabId={compactGroupHeaderDrag ? snapshot.activeTabId : null}
								orientation={TabStripOrientation.Horizontal}
								collapsedStrip={false}
								onToggle={() => undefined}
								onPointerDown={() => undefined}
								onKeyDown={() => undefined}
								onContextMenu={() => undefined}
								onMouseEnter={() => undefined}
								onMouseLeave={() => undefined}
								showHoverCard={false}
							/>
						) : null}
						{view.kind === 'tab' && tab ? (
							<TabButton
								tab={tab}
								snapshot={snapshot}
								width={view.width}
								orientation={TabStripOrientation.Horizontal}
								collapsed={false}
								onPointerDown={() => undefined}
								onClose={() => undefined}
								onContextMenu={() => undefined}
								onMouseEnter={() => undefined}
								onMouseLeave={() => undefined}
								onKeyDown={() => undefined}
								onKeyUp={() => undefined}
								touchUi={touchUi}
								closeButtonAllowed={closeButtonAllowed}
								declutterEnabled={declutterEnabled}
								hoveredTabId={null}
							/>
						) : null}
					</div>
				);
			})}
			{draggedGroupUnderlines.map(({ group: draggedGroup, underline }) =>
				underline.width > 0 ? (
					<span
						key={`underline-${draggedGroup.id}`}
						className={styles['ct-drag-ghost__underline']}
						data-drag-group-underline=''
						data-group-underline-id={draggedGroup.id}
						style={
							{
								'--ob-comp-tab-strip-inline-start': `${underline.x}px`,
								'--ob-comp-tab-strip-inline-size': `${underline.width}px`,
								'--ob-comp-tab-strip-group-color': GROUP_COLORS[draggedGroup.color],
							} satisfies CSSProperties
						}
					/>
				) : null,
			)}
		</div>
	);
}
