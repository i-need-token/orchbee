import { ProductIcon } from '@orchbee/ui/components';
import styles from './HorizontalContents.module.css';
import { type CSSProperties, type ReactNode } from 'react';
import { calculateTabContainerZValue } from '../../../../layout/z-orderable-tab-container-element';
import { requireDefined } from '../../../../utils/requireDefined';
import { calculateHorizontalTabGroupUnderlineBounds } from '../../../../layout/tab-group-style';
import { TAB_SURFACE_HORIZONTAL_INSET } from '../../../../layout/tab-style-views';
import { TabStripControlButton } from './components/tab-strip-control-button/TabStripControlButton';
import {
	calculateHorizontalDragPresentation,
	calculateHorizontalTabStripTrailingEdge,
	GROUP_COLORS,
	joinClassNames,
	tabIsApparentlyActive,
} from '../../../../utils/tab-strip-internals';
import { TabButton } from '../tab-button/TabButton';
import { GroupHeader } from '../group-header/GroupHeader';
import { useTabStripMessages } from '../../../../hooks/useTabStripMessages';
import type { TabStripViewModel } from '../../../../types/tab-strip-view-model';
import { LAYOUT_CONSTANTS } from '../../../../layout/layout-constants';
import { DEFAULT_TAB_STYLE_METRICS } from '../../../../layout/tab-style-metrics';
import { tabCollectionEaseOut } from '../../../../layout/tab-collection-animating-layout-manager';
import {
	clampDraggedViewPosition,
	getAnimatedDraggedViewPosition,
} from '../../../../dragging/dragged-tabs-container-layout';

export function HorizontalContents({
	horizontalLeadingActions,
	view,
}: {
	readonly horizontalLeadingActions: ReactNode;
	readonly view: TabStripViewModel;
}): ReactNode {
	const messages = useTabStripMessages();
	const {
		actions,
		identityIcons,
		orientation,
		statusAdornments,
		showGroupHoverCards,
		showNewTabButton,
		horizontalFrameCondensed,
		touchUi,
		tabStripDeclutter,
		snapshot,
		size,
		horizontalTrackRef,
		items,
		horizontalAnimationRuntimeRef,
		setMenu,
		hover,
		drag,
		openGroupEditor,
		requestCloseTabs,
		expandedHeaderDragGroupId,
		horizontalLayout,
		clearHover,
		onTabMouseEnter,
		onGroupMouseEnter,
		onTabPointerDown,
		onGroupPointerDown,
		toggleGroupFromHeader,
		onGroupKeyDown,
		onTabKeyDown,
		onTabKeyUp,
		rovingFocusKey,
		addTab,
		finishHorizontalDragSettle,
		paintedHorizontalBounds,
		settleTargetLeft,
	} = view;

	const idealTrailingBounds = horizontalLayout?.bounds.at(-1);
	const paintedTrailingBounds = paintedHorizontalBounds(items.length - 1);
	const dragPresentation =
		drag?.started === true
			? calculateHorizontalDragPresentation({ drag, snapshot, settleTargetLeft })
			: null;
	const dragStartAnimationValue = tabCollectionEaseOut(drag?.dragStartAnimationProgress ?? 1);
	const renderedDraggedViews =
		dragPresentation && drag
			? dragPresentation.views.map((draggedView) => {
					const position = getAnimatedDraggedViewPosition({
						axes: 'both',
						draggingBounds: {
							x: dragPresentation.left,
							y: drag.dragAreaBounds.top,
							width: dragPresentation.width,
							height: LAYOUT_CONSTANTS.tabStripHeight,
						},
						targetOffset: { x: draggedView.x, y: draggedView.y },
						animationStartOffset: { x: draggedView.initialOffsetX, y: 0 },
						animationValue: dragStartAnimationValue,
					});
					const clamped = clampDraggedViewPosition(
						position,
						{ width: draggedView.width, height: draggedView.height },
						{
							x: drag.dragAreaBounds.left,
							y: drag.dragAreaBounds.top,
							width: drag.dragAreaBounds.width,
							height: drag.dragAreaBounds.height,
						},
					);
					return {
						...draggedView,
						x: clamped.x - dragPresentation.left,
						y: clamped.y - drag.dragAreaBounds.top,
					};
				})
			: [];
	const draggedViewById = new Map(
		renderedDraggedViews.map((draggedView) => [draggedView.id, draggedView]),
	);
	const tabById = new Map(snapshot.tabs.map((tab) => [tab.id, tab]));
	const originalGroupByTabId = new Map(
		drag?.restoreState.tabs.map((tab) => [tab.id, tab.group]) ?? [],
	);
	const draggedGroupUnderlines =
		dragPresentation === null
			? []
			: snapshot.groups.flatMap((draggedGroup) => {
					const groupViews = renderedDraggedViews.filter((draggedView) =>
						draggedView.kind === 'group'
							? draggedView.id === `group:${draggedGroup.id}`
							: originalGroupByTabId.get(draggedView.id) === draggedGroup.id &&
								tabById.get(draggedView.id)?.group === draggedGroup.id,
					);
					if (groupViews.length === 0) return [];
					const leading = groupViews.reduce((result, candidate) =>
						candidate.x < result.x ? candidate : result,
					);
					const trailing = groupViews.reduce((result, candidate) =>
						candidate.x + candidate.width > result.x + result.width ? candidate : result,
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
					const tabDragged = groupViews.some((candidate) => candidate.kind === 'tab');
					if (!tabDragged) return [];
					return [
						{
							group: draggedGroup,
							x: naturalUnderlineLeft,
							width: Math.max(0, naturalUnderlineRight - naturalUnderlineLeft),
						},
					];
				});
	// Chromium keeps the ideal strip width as the preferred-width floor during
	// a drag. The drag context can expand that width to the right, but moving the
	// logical last tab left cannot pull the new-tab button inward.
	const newTabButtonTrailingEdge = calculateHorizontalTabStripTrailingEdge({
		idealTrailingEdge: (idealTrailingBounds?.x ?? 0) + (idealTrailingBounds?.width ?? 0),
		paintedTrailingEdge: (paintedTrailingBounds?.x ?? 0) + (paintedTrailingBounds?.width ?? 0),
		dragTrailingEdge:
			dragPresentation && drag
				? dragPresentation.left - drag.dragAreaBounds.left + dragPresentation.width
				: null,
	});

	return (
		<div className={styles['ct-horizontal-contents']}>
			{horizontalLeadingActions === null ? null : (
				<div className={styles['ct-horizontal-leading-actions']} data-horizontal-leading-actions=''>
					{horizontalLeadingActions}
				</div>
			)}
			<div
				ref={horizontalTrackRef}
				className={styles['ct-horizontal-track']}
				data-horizontal-track=''
				data-source-layout=''
			>
				{dragPresentation && drag ? (
					<div
						className={styles['ct-horizontal-drag-context']}
						data-horizontal-drag-context=''
						data-layout='horizontal'
						data-settling={drag.settling || undefined}
						onTransitionEnd={(event) => {
							if (
								!drag.settling ||
								event.currentTarget !== event.target ||
								(event.propertyName !== 'left' && event.propertyName !== 'width')
							)
								return;
							finishHorizontalDragSettle();
						}}
						style={
							{
								'--ob-comp-tab-strip-inline-start': `${dragPresentation.left}px`,
								'--ob-comp-tab-strip-block-start': `${drag.dragAreaBounds.top}px`,
								'--ob-comp-tab-strip-inline-size': `${dragPresentation.width}px`,
								'--ob-comp-tab-strip-block-size': `${LAYOUT_CONSTANTS.tabStripHeight}px`,
								'--ob-comp-tab-strip-runtime-layer':
									draggedGroupUnderlines.length > 0
										? calculateTabContainerZValue({ kind: 'group-underline' })
										: calculateTabContainerZValue({
												kind: 'group-header',
												active: true,
											}),
							} satisfies CSSProperties
						}
						aria-hidden='true'
					>
						{draggedGroupUnderlines.map(({ group, x, width }) =>
							width > 0 ? (
								<span
									key={`underline-${group.id}`}
									className={styles['ct-horizontal-drag-underline']}
									data-drag-group-underline=''
									data-group-underline-id={group.id}
									style={
										{
											'--ob-comp-tab-strip-inline-start': `${x}px`,
											'--ob-comp-tab-strip-inline-size': `${width}px`,
											'--ob-comp-tab-strip-group-color': GROUP_COLORS[group.color],
										} satisfies CSSProperties
									}
								/>
							) : null,
						)}
					</div>
				) : null}
				{snapshot.groups.map((group) => {
					const groupHeaderDragged =
						dragPresentation?.views.some(
							(candidate) => candidate.kind === 'group' && candidate.id === `group:${group.id}`,
						) === true;
					if (groupHeaderDragged) return null;
					const groupItemIndex = items.findIndex(
						(item) => item.type === 'group' && item.group.id === group.id,
					);
					const groupTabIndices = items
						.map((item, index) => (item.type === 'tab' && item.tab.group === group.id ? index : -1))
						.filter((index) => index !== -1);
					if (
						!horizontalLayout ||
						groupItemIndex < 0 ||
						groupTabIndices.length === 0 ||
						(group.collapsed &&
							groupTabIndices.every(
								(index) =>
									!horizontalAnimationRuntimeRef.current?.getSlot(requireDefined(items[index]).id)
										?.animating &&
									(paintedHorizontalBounds(index)?.width ?? 0) <=
										DEFAULT_TAB_STYLE_METRICS.tabOverlap,
							))
					)
						return null;
					const start = paintedHorizontalBounds(groupItemIndex)?.x ?? 0;
					const last = paintedHorizontalBounds(requireDefined(groupTabIndices.at(-1)));
					if (!last) return null;
					const underline = calculateHorizontalTabGroupUnderlineBounds({
						leadingHeaderX: start,
						trailingTabX: last.x,
						trailingTabWidth: last.width,
					});
					return (
						<span
							key={group.id}
							className={styles['ct-group-underline']}
							data-group-underline-id={group.id}
							style={
								{
									'--ob-comp-tab-strip-inline-start': `${underline.x}px`,
									'--ob-comp-tab-strip-inline-size': `${underline.width}px`,
									'--ob-comp-tab-strip-runtime-layer': calculateTabContainerZValue({
										kind: 'group-underline',
									}),
									'--ob-comp-tab-strip-group-color': GROUP_COLORS[group.color],
								} satisfies CSSProperties
							}
						/>
					);
				})}
				{items.map((item, index) => {
					const bounds = paintedHorizontalBounds(index) ?? {
						x: 0,
						width: 0,
						y: 0,
						height: LAYOUT_CONSTANTS.tabStripHeight,
					};
					const draggedViewId = item.type === 'group' ? `group:${item.group.id}` : item.tab.id;
					const draggedView = draggedViewById.get(draggedViewId);
					const renderedBounds =
						draggedView && dragPresentation && drag
							? {
									x: dragPresentation.left + draggedView.x,
									y: drag.dragAreaBounds.top + draggedView.y,
									width: draggedView.width,
									height: draggedView.height,
								}
							: bounds;
					if (item.type === 'group') {
						const activeGroupHeader =
							item.group.collapsed &&
							snapshot.tabs.some(
								(tab) => tab.id === snapshot.activeTabId && tab.group === item.group.id,
							);
						return (
							<div
								key={item.id}
								className={joinClassNames(
									styles['ct-horizontal-item'],
									styles['ct-horizontal-group'],
								)}
								data-horizontal-item=''
								style={{
									'--ob-comp-tab-strip-inline-start': `${renderedBounds.x}px`,
									'--ob-comp-tab-strip-block-start': `${renderedBounds.y}px`,
									'--ob-comp-tab-strip-inline-size': `${renderedBounds.width}px`,
									'--ob-comp-tab-strip-runtime-layer': draggedView
										? calculateTabContainerZValue({ kind: 'group-header', active: true })
										: calculateTabContainerZValue({
												kind: 'group-header',
												active: activeGroupHeader,
											}),
								}}
								data-drag-source={Boolean(draggedView) || undefined}
								data-horizontal-drag-view={draggedView?.kind}
								data-source={draggedView?.id === dragPresentation?.sourceVisualId || undefined}
								data-compacting={
									(draggedView !== undefined &&
										(drag?.dragStartAnimationProgress ?? 1) < 1 &&
										draggedView.initialOffsetX !== 0) ||
									undefined
								}
								data-drag-settling={(Boolean(draggedView) && drag?.settling) || undefined}
								data-slot-animating={
									horizontalAnimationRuntimeRef.current?.getSlot(item.id)?.animating || undefined
								}
							>
								<GroupHeader
									group={item.group}
									tabs={snapshot.tabs.filter((tab) => tab.group === item.group.id)}
									activeTabId={
										draggedView && !dragPresentation?.compactGroupHeaderDrag
											? null
											: snapshot.activeTabId
									}
									orientation={orientation}
									collapsedStrip={false}
									onToggle={(source) => toggleGroupFromHeader(item.group.id, source)}
									onPointerDown={(event) => onGroupPointerDown(event, item.group)}
									onKeyDown={(event) => onGroupKeyDown(event, item.group.id)}
									onContextMenu={(event) => {
										openGroupEditor(item.group.id, event.clientX, event.clientY);
									}}
									onMouseEnter={(event) => onGroupMouseEnter(event, item.group.id)}
									onMouseLeave={clearHover}
									tabIndex={rovingFocusKey === `group:${item.group.id}` ? 0 : -1}
									showHoverCard={showGroupHoverCards}
									frameCondensed={horizontalFrameCondensed}
								/>
							</div>
						);
					}
					const dragging = Boolean(drag?.started && drag.tabIds.includes(item.tab.id));
					const group =
						item.tab.group === null
							? null
							: (snapshot.groups.find((candidate) => candidate.id === item.tab.group) ?? null);
					const slotState = horizontalAnimationRuntimeRef.current?.getSlot(item.id);
					const hiddenByCollapsedGroup =
						Boolean(group?.collapsed) &&
						item.tab.group !== expandedHeaderDragGroupId &&
						!slotState?.animating &&
						bounds.width <= DEFAULT_TAB_STYLE_METRICS.tabOverlap;
					const active = tabIsApparentlyActive(item.tab, snapshot);
					const selected = snapshot.selectedTabIds.has(item.tab.id);
					const hovering = hover?.kind === 'tab' && hover.id === item.tab.id;
					const zIndex = dragging
						? calculateTabContainerZValue({ kind: 'group-header', active: true })
						: calculateTabContainerZValue({
								kind: 'tab',
								active,
								selected,
								hovering,
								hoverAnimationValue: hovering ? 1 : 0,
							});
					return (
						<div
							key={item.id}
							className={styles['ct-horizontal-item']}
							data-horizontal-item=''
							style={{
								'--ob-comp-tab-strip-inline-start': `${renderedBounds.x}px`,
								'--ob-comp-tab-strip-block-start': `${renderedBounds.y}px`,
								'--ob-comp-tab-strip-inline-size': `${renderedBounds.width}px`,
								'--ob-comp-tab-strip-runtime-layer': zIndex,
							}}
							data-group-collapsed-hidden={(hiddenByCollapsedGroup && !draggedView) || undefined}
							aria-hidden={(hiddenByCollapsedGroup && !draggedView) || undefined}
							data-drag-source={dragging || undefined}
							data-horizontal-drag-view={draggedView?.kind}
							data-source={draggedView?.id === dragPresentation?.sourceVisualId || undefined}
							data-compacting={
								(draggedView !== undefined &&
									(drag?.dragStartAnimationProgress ?? 1) < 1 &&
									draggedView.initialOffsetX !== 0) ||
								undefined
							}
							data-drag-settling={(Boolean(draggedView) && drag?.settling) || undefined}
						>
							<TabButton
								identityIcon={identityIcons.get(item.tab.id)}
								tab={item.tab}
								statusAdornment={statusAdornments.get(item.tab.id)}
								snapshot={snapshot}
								width={renderedBounds.width}
								orientation={orientation}
								collapsed={false}
								onPointerDown={onTabPointerDown}
								onClose={(event, id, source) => {
									event.stopPropagation();
									requestCloseTabs([id], source);
								}}
								onContextMenu={(event, id) => {
									setMenu({ kind: 'tab', id, x: event.clientX, y: event.clientY });
								}}
								onMouseEnter={onTabMouseEnter}
								onMouseLeave={clearHover}
								onKeyDown={onTabKeyDown}
								onKeyUp={onTabKeyUp}
								tabIndex={rovingFocusKey === `tab:${item.tab.id}` ? 0 : -1}
								touchUi={touchUi}
								closeButtonAllowed
								declutterEnabled={tabStripDeclutter}
								hoveredTabId={hover?.kind === 'tab' ? hover.id : null}
								frameCondensed={horizontalFrameCondensed}
								dragging={dragging}
								slotState={slotState}
							/>
						</div>
					);
				})}
				{showNewTabButton ? (
					<TabStripControlButton
						variantClass='ct-new-tab'
						type='button'
						aria-label={messages.newTab}
						disabled={
							actions.getCreationAvailability({ groupId: null, kind: 'new-tab' }).kind !== 'enabled'
						}
						icon={<ProductIcon name='add' />}
						frameCondensed={horizontalFrameCondensed}
						tabIndex={0}
						onClick={addTab}
						onContextMenu={(event) => {
							setMenu({ kind: 'new-tab', x: event.clientX, y: event.clientY });
						}}
						inlineStart={Math.min(newTabButtonTrailingEdge + 4, Math.max(0, size.width - 38))}
					/>
				) : null}
			</div>
		</div>
	);
}
