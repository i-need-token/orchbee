import styles from './VerticalContents.module.css';
import {
	type CSSProperties,
	type MouseEvent as ReactMouseEvent,
	type PointerEvent as ReactPointerEvent,
	type ReactNode,
} from 'react';
import {
	type TabData,
	type TabGroupData,
	type TabModelSnapshot,
} from '../../../../model/tab-model';
import {
	calculatePinnedTabContainerLayout,
	calculatePinnedTabContainerMinimumSize,
} from '../../../../layout/pinned-tab-container-layout';
import { calculateTabStripViewLayout } from '../../../../layout/tab-strip-view-layout';
import {
	calculateUnpinnedTabContainerLayout,
	calculateUnpinnedTabContainerMinimumSize,
} from '../../../../layout/unpinned-tab-container-view-layout';
import { type TabCloseSource } from '../../../../layout/tab-closing-mode';
import { requireDefined } from '../../../../utils/requireDefined';
import {
	TabStripOrientation,
	VerticalTabStripCollapseState,
} from '../../../../types/tab-strip-types';
import { LAYOUT_CONSTANTS } from '../../../../layout/layout-constants';
import { RoundedScrollbar } from './components/rounded-scrollbar/RoundedScrollbar';
import { VerticalTabStripTopContainer } from './components/vertical-tab-strip-top-container/VerticalTabStripTopContainer';
import { VerticalTabStripBottomContainer } from './components/vertical-tab-strip-bottom-container/VerticalTabStripBottomContainer';
import {
	GROUP_COLORS,
	type DragState,
	type MenuState,
} from '../../../../utils/tab-strip-internals';
import { TabButton } from '../tab-button/TabButton';
import { GroupHeader } from '../group-header/GroupHeader';
import { useVerticalContents } from './hooks/useVerticalContents';

export function VerticalContents({
	snapshot,
	fullyCollapsedGroupIds,
	collapsed,
	expandedOnHover,
	expandOnHoverEnabled,
	topContainerCollapsed,
	bottomContainerCollapsed,
	availableWidth,
	drag,
	closingTabIds,
	requestCloseTabs,
	onTabPointerDown,
	onGroupPointerDown,
	toggleGroupFromHeader,
	onGroupKeyDown,
	onTabKeyDown,
	onTabKeyUp,
	onTabMouseEnter,
	onGroupMouseEnter,
	showGroupHoverCards,
	clearHover,
	setMenu,
	onOpenGroupEditor,
	showNewTabButton,
	addTab,
	touchUi,
	closeButtonAllowed,
	showTopControls,
	showTabSearchButton,
	tabSearchTargetVisible,
	tabSearchAnimating,
	groupFocused,
	focusedGroupId,
	showPinnedTabsInFocusedGroups,
	toolbarHeight,
	captionButtonWidth,
	isExitingExpandOnHover,
	onCollapsedChange,
	onUnfocusGroup,
	onTabSearchClick,
	onTabSearchContextMenu,
	onCollapseContextMenu,
}: {
	snapshot: TabModelSnapshot;
	fullyCollapsedGroupIds: ReadonlySet<string>;
	collapsed: boolean;
	expandedOnHover: boolean;
	expandOnHoverEnabled: boolean;
	topContainerCollapsed: boolean;
	bottomContainerCollapsed: boolean;
	availableWidth: number;
	drag: DragState | null;
	closingTabIds: ReadonlySet<string>;
	requestCloseTabs: (tabIds: readonly string[], source?: TabCloseSource) => void;
	onTabPointerDown: (event: ReactPointerEvent<HTMLDivElement>, tab: Readonly<TabData>) => void;
	onGroupPointerDown: (
		event: ReactPointerEvent<HTMLDivElement>,
		group: Readonly<TabGroupData>,
	) => void;
	toggleGroupFromHeader: (groupId: string, source?: 'mouse' | 'touch' | 'keyboard') => void;
	onGroupKeyDown: (event: React.KeyboardEvent<HTMLDivElement>, groupId: string) => void;
	onTabKeyDown: (event: React.KeyboardEvent, tabId: string) => void;
	onTabKeyUp: (event: React.KeyboardEvent, tabId: string) => void;
	onTabMouseEnter: (event: ReactMouseEvent<HTMLDivElement>, tabId: string) => void;
	onGroupMouseEnter: (event: ReactMouseEvent<HTMLDivElement>, groupId: string) => void;
	showGroupHoverCards: boolean;
	clearHover: () => void;
	setMenu: (menu: MenuState) => void;
	onOpenGroupEditor: (groupId: string, x: number, y: number) => void;
	showNewTabButton: boolean;
	addTab: () => void;
	touchUi: boolean;
	closeButtonAllowed: boolean;
	showTopControls: boolean;
	showTabSearchButton: boolean;
	tabSearchTargetVisible: boolean;
	tabSearchAnimating: boolean;
	groupFocused: boolean;
	focusedGroupId: string | null;
	showPinnedTabsInFocusedGroups: boolean;
	toolbarHeight: number;
	captionButtonWidth: number;
	isExitingExpandOnHover: boolean;
	onCollapsedChange?: ((collapsed: boolean) => void) | undefined;
	onUnfocusGroup?: (() => void) | undefined;
	onTabSearchClick?: (() => void) | undefined;
	onTabSearchContextMenu: (event: ReactMouseEvent<HTMLButtonElement>) => void;
	onCollapseContextMenu: (event: ReactMouseEvent<HTMLButtonElement>) => void;
}): ReactNode {
	const { isAnimatingSize, pinnedScrollRef, regionsRef, regionsSize, scrollRef } =
		useVerticalContents({ availableWidth, drag, snapshot });
	const pinned = snapshot.tabs.filter(
		(tab) => tab.pinned && (focusedGroupId === null || showPinnedTabsInFocusedGroups),
	);
	const unpinned = snapshot.tabs.filter(
		(tab) => !tab.pinned && (focusedGroupId === null || tab.group === focusedGroupId),
	);
	const groups = new Map(snapshot.groups.map((group) => [group.id, group]));
	const draggedWholeGroupIds = new Set(
		drag?.started
			? drag.visualViews.flatMap((view) =>
					view.kind === 'group' ? [view.id.slice('group:'.length)] : [],
				)
			: [],
	);
	const renderedGroups = new Set<string>();
	const defaultTabWidth = Math.max(
		0,
		availableWidth - LAYOUT_CONSTANTS.verticalTabStripHorizontalPadding * 2,
	);
	const renderTab = (tab: Readonly<TabData>, width = defaultTabWidth) => (
		<div
			key={tab.id}
			className={styles['ct-vertical-tab-wrap']}
			data-tab-strip-tab-wrapper=''
			data-drag-source={
				(drag?.started &&
					drag.tabIds.includes(tab.id) &&
					(tab.group === null || !draggedWholeGroupIds.has(tab.group))) ||
				undefined
			}
			data-closing={closingTabIds.has(tab.id) || undefined}
		>
			<TabButton
				tab={tab}
				snapshot={snapshot}
				width={width}
				orientation={TabStripOrientation.Vertical}
				collapsed={collapsed}
				closing={closingTabIds.has(tab.id)}
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
				closeButtonAllowed={closeButtonAllowed}
				verticalExpandedOnHover={expandedOnHover}
				hoveredTabId={null}
			/>
		</div>
	);
	const renderTabSequence = (tabs: readonly Readonly<TabData>[]): ReactNode[] =>
		tabs.map((tab) => renderTab(tab));

	const pinnedLayoutCollapsed =
		collapsed &&
		(expandedOnHover || availableWidth <= LAYOUT_CONSTANTS.verticalTabStripCollapsedWidth);
	const pinnedLayoutWidth = Math.max(
		0,
		availableWidth - LAYOUT_CONSTANTS.verticalTabStripHorizontalPadding,
	);
	const pinnedLayout = calculatePinnedTabContainerLayout({
		sizeBounds: { width: pinnedLayoutWidth, height: null },
		collapseState: pinnedLayoutCollapsed
			? VerticalTabStripCollapseState.Collapsed
			: VerticalTabStripCollapseState.Expanded,
		expandOnHoverEnabled,
		children: pinned.map((tab) => ({
			id: tab.id,
			preferredSize: {
				width: LAYOUT_CONSTANTS.verticalTabMinWidth,
				height: LAYOUT_CONSTANTS.verticalTabPinnedHeight,
			},
		})),
	});

	const unpinnedDirectChildren: Array<{
		id: string;
		group?: boolean;
		preferredSize: { width: number; height: number };
		minimumSize: { width: number; height: number };
	}> = [];
	const countedGroups = new Set<string>();
	for (const tab of unpinned) {
		if (tab.group !== null) {
			if (countedGroups.has(tab.group)) continue;
			countedGroups.add(tab.group);
			const group = groups.get(tab.group);
			const groupTabs = unpinned.filter((candidate) => candidate.group === tab.group);
			const directChildCount = groupTabs.length;
			const fullyCollapsed = Boolean(
				group?.collapsed && focusedGroupId !== group.id && fullyCollapsedGroupIds.has(group.id),
			);
			unpinnedDirectChildren.push({
				id: `group-${tab.group}`,
				group: true,
				preferredSize: {
					width: availableWidth,
					height: fullyCollapsed ? 34 : 36 + directChildCount * 32,
				},
				minimumSize: { width: LAYOUT_CONSTANTS.verticalTabMinWidth, height: 34 },
			});
			continue;
		}
		unpinnedDirectChildren.push({
			id: tab.id,
			preferredSize: { width: defaultTabWidth, height: LAYOUT_CONSTANTS.verticalTabHeight },
			minimumSize: {
				width: LAYOUT_CONSTANTS.verticalTabMinWidth,
				height: LAYOUT_CONSTANTS.verticalTabHeight,
			},
		});
	}
	const collapseState = collapsed
		? VerticalTabStripCollapseState.Collapsed
		: VerticalTabStripCollapseState.Expanded;
	const unpinnedNaturalLayout = calculateUnpinnedTabContainerLayout({
		orientation: TabStripOrientation.Vertical,
		sizeBounds: { width: availableWidth, height: null },
		children: unpinnedDirectChildren,
		collapseState,
	});
	const tabStripViewLayout = calculateTabStripViewLayout({
		orientation: TabStripOrientation.Vertical,
		sizeBounds: {
			width: availableWidth,
			height: regionsSize.height > 0 ? regionsSize.height : null,
		},
		pinnedPreferred: pinnedLayout.hostSize,
		pinnedMinimum: calculatePinnedTabContainerMinimumSize(pinned.length, collapseState),
		pinnedVisible: pinned.length > 0,
		unpinnedPreferred: unpinnedNaturalLayout.hostSize,
		unpinnedMinimum: calculateUnpinnedTabContainerMinimumSize(
			TabStripOrientation.Vertical,
			unpinnedDirectChildren,
		),
		unpinnedVisible: unpinnedDirectChildren.length > 0,
		separatorPreferredHeight: 1,
		collapsed,
	});

	return (
		<>
			{showTopControls ? (
				<VerticalTabStripTopContainer
					availableWidth={availableWidth}
					collapsed={topContainerCollapsed}
					toolbarHeight={toolbarHeight}
					captionButtonWidth={captionButtonWidth}
					isExitingExpandOnHover={isExitingExpandOnHover}
					showUnfocusButton={groupFocused}
					showTabSearchButton={showTabSearchButton}
					tabSearchTargetVisible={tabSearchTargetVisible}
					tabSearchAnimating={tabSearchAnimating}
					onTabSearchContextMenu={onTabSearchContextMenu}
					onCollapseContextMenu={onCollapseContextMenu}
					onCollapsedChange={onCollapsedChange}
					onUnfocusGroup={onUnfocusGroup}
					onTabSearchClick={onTabSearchClick}
				/>
			) : null}
			<div ref={regionsRef} className={styles['ct-vertical-regions']}>
				<div
					className={styles['ct-vertical-pinned-scroll-shell']}
					data-vertical-region='pinned'
					aria-hidden={!tabStripViewLayout.pinnedVisible || undefined}
					style={{
						'--ob-comp-tab-strip-inline-start': `${tabStripViewLayout.pinnedBounds.x}px`,
						'--ob-comp-tab-strip-block-start': `${tabStripViewLayout.pinnedBounds.y}px`,
						'--ob-comp-tab-strip-inline-size': `${tabStripViewLayout.pinnedBounds.width}px`,
						'--ob-comp-tab-strip-block-size': `${tabStripViewLayout.pinnedBounds.height}px`,
					}}
				>
					<div
						ref={pinnedScrollRef}
						className={styles['ct-vertical-pinned-scroll']}
						data-tab-strip-scroll-viewport=''
					>
						<div
							className={styles['ct-vertical-pinned']}
							style={{ '--ob-comp-tab-strip-block-size': `${pinnedLayout.hostSize.height}px` }}
						>
							{pinnedLayout.children.map((child, index) => {
								const tab = requireDefined(pinned[index]);
								return (
									<div
										key={tab.id}
										className={styles['ct-vertical-pinned__unit']}
										style={{
											'--ob-comp-tab-strip-inline-start': `${child.bounds.x}px`,
											'--ob-comp-tab-strip-block-start': `${child.bounds.y}px`,
											'--ob-comp-tab-strip-inline-size': `${child.bounds.width}px`,
											'--ob-comp-tab-strip-block-size': `${child.bounds.height}px`,
										}}
									>
										{renderTab(tab, child.bounds.width)}
									</div>
								);
							})}
						</div>
					</div>
					<RoundedScrollbar
						viewportRef={pinnedScrollRef}
						isAnimatingSize={isAnimatingSize}
						shouldHaveRightMargin={!collapsed}
					/>
				</div>
				{tabStripViewLayout.separatorVisible ? (
					<div
						className={styles['ct-vertical-separator']}
						style={{
							'--ob-comp-tab-strip-inline-start': `${tabStripViewLayout.separatorBounds.x}px`,
							'--ob-comp-tab-strip-block-start': `${tabStripViewLayout.separatorBounds.y}px`,
							'--ob-comp-tab-strip-inline-size': `${tabStripViewLayout.separatorBounds.width}px`,
							'--ob-comp-tab-strip-block-size': `${tabStripViewLayout.separatorBounds.height}px`,
						}}
					/>
				) : null}
				<div
					className={styles['ct-vertical-scroll-shell']}
					data-vertical-region='unpinned'
					style={{
						'--ob-comp-tab-strip-inline-start': `${tabStripViewLayout.unpinnedBounds.x}px`,
						'--ob-comp-tab-strip-block-start': `${tabStripViewLayout.unpinnedBounds.y}px`,
						'--ob-comp-tab-strip-inline-size': `${tabStripViewLayout.unpinnedBounds.width}px`,
						'--ob-comp-tab-strip-block-size': `${tabStripViewLayout.unpinnedBounds.height}px`,
					}}
				>
					<div
						ref={scrollRef}
						className={styles['ct-vertical-scroll']}
						data-tab-strip-scroll-viewport=''
					>
						<div className={styles['ct-vertical-scroll__content']} data-tab-strip-scroll-content=''>
							{unpinned.map((tab) => {
								if (tab.group === null) return renderTab(tab);
								if (renderedGroups.has(tab.group)) return null;
								renderedGroups.add(tab.group);
								const group = groups.get(tab.group);
								if (!group) return renderTab(tab);
								const groupTabs = unpinned.filter((candidate) => candidate.group === group.id);
								const fullyCollapsed =
									group.collapsed &&
									focusedGroupId !== group.id &&
									fullyCollapsedGroupIds.has(group.id);
								const collapsing =
									group.collapsed && focusedGroupId !== group.id && !fullyCollapsed;
								return (
									<section
										key={group.id}
										className={styles['ct-vertical-group']}
										data-tab-strip-group=''
										data-group-id={group.id}
										data-group-collapsed={fullyCollapsed || undefined}
										data-group-collapsing={collapsing || undefined}
										data-drag-source={draggedWholeGroupIds.has(group.id) || undefined}
										style={
											{
												'--ob-comp-tab-strip-group-color': GROUP_COLORS[group.color],
											} satisfies CSSProperties
										}
									>
										<GroupHeader
											group={group}
											tabs={groupTabs}
											activeTabId={snapshot.activeTabId}
											orientation={TabStripOrientation.Vertical}
											collapsedStrip={collapsed}
											onToggle={(source) => toggleGroupFromHeader(group.id, source)}
											onPointerDown={(event) => onGroupPointerDown(event, group)}
											onKeyDown={(event) => onGroupKeyDown(event, group.id)}
											onContextMenu={(event) => {
												onOpenGroupEditor(group.id, event.clientX, event.clientY);
											}}
											onOpenEditor={(x, y) => onOpenGroupEditor(group.id, x, y)}
											onMouseEnter={(event) => onGroupMouseEnter(event, group.id)}
											onMouseLeave={clearHover}
											showHoverCard={showGroupHoverCards}
										/>
										<div
											className={styles['ct-vertical-group__tabs']}
											aria-hidden={fullyCollapsed || undefined}
										>
											<div className={styles['ct-vertical-group__tabs-inner']}>
												{renderTabSequence(groupTabs)}
											</div>
										</div>
									</section>
								);
							})}
						</div>
					</div>
					<RoundedScrollbar
						viewportRef={scrollRef}
						isAnimatingSize={isAnimatingSize}
						shouldHaveRightMargin={!collapsed}
					/>
				</div>
			</div>
			{showNewTabButton ? (
				<VerticalTabStripBottomContainer
					collapseState={
						bottomContainerCollapsed
							? VerticalTabStripCollapseState.Collapsed
							: VerticalTabStripCollapseState.Expanded
					}
					onNewTab={addTab}
					onContextMenu={(event) => {
						setMenu({ kind: 'new-tab', x: event.clientX, y: event.clientY });
					}}
				/>
			) : null}
		</>
	);
}
