import styles from './TabStrip.module.css';
import { ContextMenu, ContextMenuTrigger } from '@orchbee/ui/components';
import { type CSSProperties, type ReactNode } from 'react';
import { HoverCardUpdateType } from '../hovercard';
import { calculateTabContainerZValue } from '../layout';
import { requireDefined } from '../common';
import { LAYOUT_CONSTANTS } from '../style';
import { calculateHorizontalTabGroupUnderlineBounds } from '../groups';
import { TabStripIcon } from './components/tab-strip-icon/TabStripIcon';
import { TabStripControlButton } from './components/tab-strip-control-button/TabStripControlButton';
import {
	GROUP_COLORS,
	joinClassNames,
	tabIsApparentlyActive,
	type TabStripProps,
} from './utils/tab-strip-internals';
import { TabButton } from './components/tab-button/TabButton';
import { GroupHeader } from './components/group-header/GroupHeader';
import { TabStripContextMenu } from './components/tab-strip-context-menu/TabStripContextMenu';
import { DragGhost } from './components/drag-ghost/DragGhost';
import { HoverCard } from './components/hover-card/HoverCard';
import { VerticalContents } from './components/vertical-contents/VerticalContents';
import { useTabStrip } from './hooks/useTabStrip';
import { DEFAULT_TAB_STRIP_MESSAGES, TabStripMessagesContext } from './messages';

export type { TabStripCrossWindowDragRuntime, TabStripProps } from './utils/tab-strip-internals';
export type { TabStripMessages } from './messages';

// TabContainerImpl::AnimateViewTo uses RichAnimationDuration(200ms).

/** Pointer-lifecycle surface implemented by CrossWindowTabDragRuntime. */

export function TabStrip(props: TabStripProps): ReactNode {
	const messages = props.messages ?? DEFAULT_TAB_STRIP_MESSAGES;
	const {
		model,
		orientation,
		verticalCollapsed,
		verticalExpandOnHover,
		showHoverCardPreviews,
		showHoverCardMemoryUsage,
		showGroupHoverCards,
		showVerticalTopControls,
		verticalGroupFocused,
		showPinnedTabsInFocusedGroups,
		verticalToolbarHeight,
		verticalCaptionButtonWidth,
		verticalIsExitingExpandOnHover,
		onVerticalCollapsedChange,
		onVerticalUnfocusGroup,
		onVerticalTabSearchClick,
		onVerticalExpandOnHoverChange,
		showNewTabButton,
		horizontalFrameCondensed,
		touchUi,
		tabStripDeclutter,
		onTabKeyboardFocusChanged,
		onMoveTabsToNewWindow,
		onMoveGroupToNewWindow,
		enableTabGroupFocusing,
		ariaLabel,
		snapshot,
		horizontal,
		focusedGroup,
		groupFocusActive,
		tabVisibleWhileFocused,
		stripRef,
		size,
		items,
		horizontalAnimationRuntimeRef,
		animatedHorizontalFrame,
		menu,
		setMenu,
		hover,
		drag,
		closingTabIds,
		fullyCollapsedGroupIds,
		tabClosingMode,
		announcement,
		groupEditorTracker,
		openGroupEditor,
		hoverCardSnapshot,
		thumbnailSnapshot,
		comboButtonController,
		comboButtonSnapshot,
		expandOnHoverController,
		expandOnHoverSnapshot,
		expandedOnHover,
		effectiveVerticalCollapsed,
		requestCloseTabs,
		expandedHeaderDragGroupId,
		horizontalLayout,
		clearHover,
		updateHoverForElement,
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
		settleTargetLeft,
		focusedGroupColor,
	} = useTabStrip(props, messages);
	const closeMenu = () => {
		if (menu?.kind === 'combo-end') comboButtonController.onMenuClosed();
		setMenu(null);
	};
	const openGroupEditorFromControl = (id: string, x: number, y: number) => {
		openGroupEditor(id, x, y);
		stripRef.current?.dispatchEvent(
			new MouseEvent('contextmenu', {
				bubbles: true,
				cancelable: true,
				clientX: x,
				clientY: y,
			}),
		);
	};

	return (
		<TabStripMessagesContext.Provider value={messages}>
			<ContextMenu
				open={menu !== null}
				onOpenChange={(open) => {
					if (!open) closeMenu();
				}}
			>
				<ContextMenuTrigger
					ref={stripRef}
					className={styles['ct-tab-strip']}
					style={{ '--ob-comp-tab-strip-focused-group-color': focusedGroupColor }}
					role='tablist'
					aria-label={ariaLabel}
					aria-orientation={horizontal ? 'horizontal' : 'vertical'}
					data-ct-root=''
					data-orientation={orientation}
					data-touch-ui={touchUi || undefined}
					data-collapsed={effectiveVerticalCollapsed || undefined}
					data-expanded-on-hover={expandedOnHover || undefined}
					data-waiting-to-expand={expandOnHoverSnapshot.waitingToExpand || undefined}
					data-hover-card-waiting={hoverCardSnapshot.waitingToShow || undefined}
					data-dragging={(drag?.started && !drag.settling) || undefined}
					data-focused-group={focusedGroup?.id}
					data-tab-closing-mode={tabClosingMode?.source}
					onScrollCapture={() => {
						clearHover(HoverCardUpdateType.Event);
						if (menu?.kind === 'group' && groupEditorTracker.onContentsScrolled()) {
							setMenu(null);
						}
					}}
					onMouseEnter={(event) => {
						const target = event.target;
						if (!horizontal && target instanceof Node && event.currentTarget.contains(target)) {
							expandOnHoverController.recordPointerPosition?.(
								event.clientX,
								event.currentTarget.getBoundingClientRect().right,
							);
							expandOnHoverController.setHovered(true);
						}
					}}
					onMouseMove={(event) => {
						const target = event.target;
						if (!horizontal && target instanceof Node && event.currentTarget.contains(target)) {
							expandOnHoverController.recordPointerPosition?.(
								event.clientX,
								event.currentTarget.getBoundingClientRect().right,
							);
						}
					}}
					onMouseLeave={(event) => {
						const target = event.target;
						if (!horizontal && target instanceof Node && event.currentTarget.contains(target)) {
							expandOnHoverController.setHovered(false);
						}
					}}
					onFocusCapture={(event) => {
						const target = event.target;
						if (!horizontal && target instanceof Node && event.currentTarget.contains(target)) {
							expandOnHoverController.setFocused(true);
						}
						const anchor =
							target instanceof Element
								? target.closest<HTMLElement>('[data-tab-id], [data-group-id]')
								: null;
						if (anchor?.dataset['tabId']) onTabKeyboardFocusChanged?.(anchor.dataset['tabId']);
						if (anchor && event.currentTarget.contains(anchor)) {
							if (anchor.dataset['tabId'])
								updateHoverForElement(anchor, anchor.dataset['tabId'], HoverCardUpdateType.Focus);
							else if (anchor.dataset['groupId'] && showGroupHoverCards) {
								updateHoverForElement(
									anchor,
									anchor.dataset['groupId'],
									HoverCardUpdateType.Focus,
									'group',
								);
							}
						}
					}}
					onBlurCapture={(event) => {
						const target = event.target;
						const next = event.relatedTarget;
						if (
							!horizontal &&
							target instanceof Node &&
							event.currentTarget.contains(target) &&
							(!(next instanceof Node) || !event.currentTarget.contains(next))
						) {
							expandOnHoverController.setFocused(false);
						}
						if (!(next instanceof Node) || !event.currentTarget.contains(next)) {
							onTabKeyboardFocusChanged?.(null);
						}
						const anchor =
							target instanceof Element
								? target.closest<HTMLElement>('[data-tab-id], [data-group-id]')
								: null;
						if (anchor && event.currentTarget.contains(anchor))
							clearHover(HoverCardUpdateType.Focus);
					}}
				>
					{horizontal ? (
						<div
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
									.map((item, index) =>
										item.type === 'tab' && item.tab.group === group.id ? index : -1,
									)
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
								const trailingTab = snapshot.tabs.filter((tab) => tab.group === group.id).at(-1);
								const underline = calculateHorizontalTabGroupUnderlineBounds({
									leadingHeaderX: start,
									trailingTabX: last.x,
									trailingTabWidth: last.width,
									trailingTabActive: trailingTab
										? tabIsApparentlyActive(trailingTab, snapshot)
										: false,
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
									height: 41,
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
													drag.visualViews.some((view) => view.id === `group:${item.group.id}`)) ||
												undefined
											}
											data-slot-animating={
												horizontalAnimationRuntimeRef.current?.getSlot(item.id)?.animating ||
												undefined
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
									Boolean(
										snapshot.groups.find((group) => group.id === item.tab.group)?.collapsed,
									) &&
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
					) : (
						<VerticalContents
							snapshot={snapshot}
							fullyCollapsedGroupIds={fullyCollapsedGroupIds}
							collapsed={verticalCollapsed}
							expandedOnHover={expandedOnHover}
							expandOnHoverEnabled={verticalExpandOnHover}
							topContainerCollapsed={verticalCollapsed}
							bottomContainerCollapsed={verticalCollapsed}
							availableWidth={
								size.width ||
								(effectiveVerticalCollapsed ? LAYOUT_CONSTANTS.verticalTabStripCollapsedWidth : 240)
							}
							drag={drag}
							closingTabIds={closingTabIds}
							requestCloseTabs={requestCloseTabs}
							onTabPointerDown={onTabPointerDown}
							onGroupPointerDown={onGroupPointerDown}
							toggleGroupFromHeader={toggleGroupFromHeader}
							onGroupKeyDown={onGroupKeyDown}
							onTabKeyDown={onTabKeyDown}
							onTabKeyUp={onTabKeyUp}
							onTabMouseEnter={onTabMouseEnter}
							onGroupMouseEnter={onGroupMouseEnter}
							showGroupHoverCards={showGroupHoverCards}
							clearHover={clearHover}
							setMenu={setMenu}
							onOpenGroupEditor={openGroupEditorFromControl}
							showNewTabButton={showNewTabButton}
							addTab={addTab}
							touchUi={touchUi}
							closeButtonAllowed
							showTopControls={showVerticalTopControls}
							showTabSearchButton={comboButtonSnapshot.tabSearchVisible}
							tabSearchTargetVisible={comboButtonSnapshot.tabSearchTargetVisible}
							tabSearchAnimating={comboButtonSnapshot.tabSearchAnimating}
							groupFocused={verticalGroupFocused || groupFocusActive}
							focusedGroupId={focusedGroup?.id ?? null}
							showPinnedTabsInFocusedGroups={showPinnedTabsInFocusedGroups}
							toolbarHeight={verticalToolbarHeight}
							captionButtonWidth={verticalCaptionButtonWidth}
							isExitingExpandOnHover={verticalIsExitingExpandOnHover}
							onUnfocusGroup={() => {
								if (snapshot.focusedGroupId !== null) model.setFocusedGroup(null);
								onVerticalUnfocusGroup?.();
							}}
							onTabSearchContextMenu={(event) => {
								comboButtonController.onMenuOpened('tab-search');
								setMenu({ kind: 'combo-end', x: event.clientX, y: event.clientY });
							}}
							onCollapseContextMenu={(event) => {
								setMenu({ kind: 'collapse', x: event.clientX, y: event.clientY });
							}}
							onCollapsedChange={onVerticalCollapsedChange}
							onTabSearchClick={onVerticalTabSearchClick}
						/>
					)}
					{drag?.started ? (
						<DragGhost
							drag={drag}
							snapshot={snapshot}
							orientation={orientation}
							collapsed={verticalCollapsed}
							settleTargetLeft={settleTargetLeft}
							touchUi={touchUi}
							closeButtonAllowed
							declutterEnabled={tabStripDeclutter}
						/>
					) : null}
					{hover &&
					hoverCardSnapshot.visible &&
					hoverCardSnapshot.targetId === `${hover.kind}:${hover.id}` &&
					!drag?.started &&
					!menu ? (
						<HoverCard
							hover={hover}
							snapshot={snapshot}
							showPreviews={showHoverCardPreviews}
							showMemoryUsage={showHoverCardMemoryUsage}
							transitioning={hoverCardSnapshot.transitioning}
							thumbnailSnapshot={thumbnailSnapshot}
						/>
					) : null}
					<span
						key={announcement.sequence}
						className={styles['ct-sr-only']}
						role='status'
						aria-live='polite'
						aria-atomic='true'
					>
						{announcement.text}
					</span>
				</ContextMenuTrigger>
				{menu ? (
					<TabStripContextMenu
						state={menu}
						snapshot={snapshot}
						model={model}
						orientation={orientation}
						requestCloseTabs={requestCloseTabs}
						closeAllowed
						verticalExpandOnHover={verticalExpandOnHover}
						comboButtonController={comboButtonController}
						enableTabGroupFocusing={enableTabGroupFocusing}
						onVerticalExpandOnHoverChange={onVerticalExpandOnHoverChange}
						onMoveTabsToNewWindow={onMoveTabsToNewWindow}
						onMoveGroupToNewWindow={onMoveGroupToNewWindow}
						onClose={closeMenu}
					/>
				) : null}
			</ContextMenu>
		</TabStripMessagesContext.Provider>
	);
}
