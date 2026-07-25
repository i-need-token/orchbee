import styles from './HorizontalContents.module.css';
import { type CSSProperties, type ReactNode } from 'react';
import { calculateTabContainerZValue } from '../../../../layout/z-orderable-tab-container-element';
import { requireDefined } from '../../../../utils/requireDefined';
import { calculateHorizontalTabGroupUnderlineBounds } from '../../../../layout/tab-group-style';
import { TabStripIcon } from '../tab-strip-icon/TabStripIcon';
import { TabStripControlButton } from './components/tab-strip-control-button/TabStripControlButton';
import {
	GROUP_COLORS,
	joinClassNames,
	tabIsApparentlyActive,
} from '../../../../utils/tab-strip-internals';
import { TabButton } from '../tab-button/TabButton';
import { GroupHeader } from '../group-header/GroupHeader';
import { useTabStripMessages } from '../../../../hooks/useTabStripMessages';
import type { TabStripViewModel } from '../../../../types/tab-strip-view-model';
import { LAYOUT_CONSTANTS } from '../../../../layout/layout-constants';

export function HorizontalContents({
	horizontalLeadingActions,
	view,
}: {
	readonly horizontalLeadingActions: ReactNode;
	readonly view: TabStripViewModel;
}): ReactNode {
	const messages = useTabStripMessages();
	const {
		orientation,
		showGroupHoverCards,
		showNewTabButton,
		horizontalFrameCondensed,
		touchUi,
		tabStripDeclutter,
		snapshot,
		focusedGroup,
		groupFocusActive,
		tabVisibleWhileFocused,
		size,
		horizontalTrackRef,
		items,
		horizontalAnimationRuntimeRef,
		animatedHorizontalFrame,
		setMenu,
		hover,
		drag,
		closingTabIds,
		fullyCollapsedGroupIds,
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
		addTab,
		paintedHorizontalBounds,
	} = view;

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
				{snapshot.groups.map((group) => {
					if (groupFocusActive && group.id !== requireDefined(focusedGroup).id) return null;
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
							fullyCollapsedGroupIds.has(group.id) &&
							focusedGroup?.id !== group.id)
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
					const animatedOpacity = animatedHorizontalFrame?.opacityById.get(item.id);
					const hiddenByGroupFocus =
						groupFocusActive &&
						(item.type === 'group'
							? item.group.id !== requireDefined(focusedGroup).id
							: !tabVisibleWhileFocused(item.tab));
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
									'--ob-comp-tab-strip-inline-start': `${bounds.x}px`,
									'--ob-comp-tab-strip-inline-size': `${bounds.width}px`,
									'--ob-comp-tab-strip-runtime-opacity':
										animatedOpacity !== undefined && animatedOpacity < 1
											? animatedOpacity
											: undefined,
									'--ob-comp-tab-strip-runtime-layer': calculateTabContainerZValue({
										kind: 'group-header',
										active: activeGroupHeader,
									}),
								}}
								data-focus-hidden={hiddenByGroupFocus || undefined}
								aria-hidden={hiddenByGroupFocus || undefined}
								data-drag-source={
									(drag?.started &&
										drag.visualViews.some(
											(dragView) => dragView.id === `group:${item.group.id}`,
										)) ||
									undefined
								}
								data-slot-animating={
									horizontalAnimationRuntimeRef.current?.getSlot(item.id)?.animating || undefined
								}
							>
								<GroupHeader
									group={item.group}
									tabs={snapshot.tabs.filter((tab) => tab.group === item.group.id)}
									activeTabId={snapshot.activeTabId}
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
									showHoverCard={showGroupHoverCards}
									frameCondensed={horizontalFrameCondensed}
								/>
							</div>
						);
					}
					const dragging = Boolean(drag?.started && drag.tabIds.includes(item.tab.id));
					const hiddenByCollapsedGroup =
						item.tab.group !== null &&
						fullyCollapsedGroupIds.has(item.tab.group) &&
						Boolean(snapshot.groups.find((group) => group.id === item.tab.group)?.collapsed) &&
						item.tab.group !== expandedHeaderDragGroupId &&
						focusedGroup?.id !== item.tab.group;
					const active = tabIsApparentlyActive(item.tab, snapshot);
					const selected = snapshot.selectedTabIds.has(item.tab.id);
					const hovering = hover?.kind === 'tab' && hover.id === item.tab.id;
					const slotState = horizontalAnimationRuntimeRef.current?.getSlot(item.id);
					const zIndex = dragging
						? 32
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
								'--ob-comp-tab-strip-inline-start': `${bounds.x}px`,
								'--ob-comp-tab-strip-inline-size': `${bounds.width}px`,
								'--ob-comp-tab-strip-runtime-opacity':
									animatedOpacity !== undefined && animatedOpacity < 1
										? animatedOpacity
										: undefined,
								'--ob-comp-tab-strip-runtime-layer': zIndex,
							}}
							data-focus-hidden={hiddenByGroupFocus || undefined}
							data-group-collapsed-hidden={hiddenByCollapsedGroup || undefined}
							aria-hidden={hiddenByGroupFocus || hiddenByCollapsedGroup || undefined}
							data-drag-source={dragging || undefined}
							data-closing={closingTabIds.has(item.tab.id) || undefined}
						>
							<TabButton
								tab={item.tab}
								snapshot={snapshot}
								width={bounds.width}
								orientation={orientation}
								collapsed={false}
								closing={closingTabIds.has(item.tab.id)}
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
						icon={<TabStripIcon name='plus' />}
						frameCondensed={horizontalFrameCondensed}
						onClick={addTab}
						onContextMenu={(event) => {
							setMenu({ kind: 'new-tab', x: event.clientX, y: event.clientY });
						}}
						inlineStart={Math.min(
							(paintedHorizontalBounds(items.length - 1)?.x ?? 0) +
								(paintedHorizontalBounds(items.length - 1)?.width ?? 0) +
								4,
							Math.max(0, size.width - 38),
						)}
					/>
				) : null}
			</div>
		</div>
	);
}
