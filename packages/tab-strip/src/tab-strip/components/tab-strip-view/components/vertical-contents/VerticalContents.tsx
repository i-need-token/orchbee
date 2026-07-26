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
import { calculatePinnedTabContainerMinimumSize } from '../../../../layout/pinned-tab-container-layout';
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
import { useVerticalGroupCollapseCompletion } from './hooks/useVerticalGroupCollapseCompletion';
import { useVerticalPinAnimation } from './hooks/useVerticalPinAnimation';
import { type VerticalPinAnimationEntry } from './utils/buildVerticalPinAnimationEntries';
import { calculateVerticalPinnedPinAnimationLayout } from './utils/calculateVerticalPinnedPinAnimationLayout';

export function VerticalContents({
	snapshot,
	tabCountTransitionSuppressed,
	collapsed,
	expandedOnHover,
	expandOnHoverEnabled,
	topContainerCollapsed,
	bottomContainerCollapsed,
	availableWidth,
	drag,
	requestCloseTabs,
	onTabPointerDown,
	onGroupPointerDown,
	onDragScrolled,
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
	toolbarHeight,
	captionButtonWidth,
	isExitingExpandOnHover,
	onCollapsedChange,
	onTabSearchClick,
	onTabSearchContextMenu,
	onCollapseContextMenu,
}: {
	snapshot: TabModelSnapshot;
	tabCountTransitionSuppressed: boolean;
	collapsed: boolean;
	expandedOnHover: boolean;
	expandOnHoverEnabled: boolean;
	topContainerCollapsed: boolean;
	bottomContainerCollapsed: boolean;
	availableWidth: number;
	drag: DragState | null;
	requestCloseTabs: (tabIds: readonly string[], source?: TabCloseSource) => void;
	onTabPointerDown: (event: ReactPointerEvent<HTMLDivElement>, tab: Readonly<TabData>) => void;
	onGroupPointerDown: (
		event: ReactPointerEvent<HTMLDivElement>,
		group: Readonly<TabGroupData>,
	) => void;
	onDragScrolled: () => void;
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
	toolbarHeight: number;
	captionButtonWidth: number;
	isExitingExpandOnHover: boolean;
	onCollapsedChange?: ((collapsed: boolean) => void) | undefined;
	onTabSearchClick?: (() => void) | undefined;
	onTabSearchContextMenu: (event: ReactMouseEvent<HTMLButtonElement>) => void;
	onCollapseContextMenu: (event: ReactMouseEvent<HTMLButtonElement>) => void;
}): ReactNode {
	const { isAnimatingSize, pinnedScrollRef, regionsRef, regionsSize, scrollRef } =
		useVerticalContents({ availableWidth, collapsed, drag, onDragScrolled, snapshot });
	const { pinned, unpinned } = useVerticalPinAnimation({ snapshot });
	const { fullyCollapsedGroupIds, onGroupTabsTransitionEnd } = useVerticalGroupCollapseCompletion(
		snapshot.groups,
	);
	const groups = new Map(snapshot.groups.map((group) => [group.id, group]));
	for (const entry of unpinned) {
		if (entry.retainedGroup) groups.set(entry.retainedGroup.id, entry.retainedGroup);
	}
	const draggedWholeGroupIds = new Set(
		drag?.started
			? drag.visualViews.flatMap((view) =>
					view.kind === 'group' ? [view.id.slice('group:'.length)] : [],
				)
			: [],
	);
	const snappingExpandedGroupPickup =
		drag?.started === true &&
		drag.groupHeaderId !== null &&
		!drag.groupWasCollapsedBeforeDrag &&
		drag.targetIndex === null;
	const defaultTabWidth = Math.max(
		0,
		availableWidth - LAYOUT_CONSTANTS.verticalTabStripHorizontalPadding * 2,
	);
	const isIndividuallyDraggedTab = (tab: Readonly<TabData>): boolean =>
		Boolean(
			drag?.started &&
			drag.tabIds.includes(tab.id) &&
			(tab.group === null || !draggedWholeGroupIds.has(tab.group)),
		);
	const dragSourceRenderKey = (
		stableKey: string,
		dragSource: boolean,
		slotIdentity: string | number,
	): string => (dragSource ? `${stableKey}:drag-slot:${slotIdentity}` : stableKey);
	const renderTab = (entry: Readonly<VerticalPinAnimationEntry>, width = defaultTabWidth) => {
		const { tab } = entry;
		const dragSource = isIndividuallyDraggedTab(tab);
		return (
			<div
				key={dragSourceRenderKey(entry.key, dragSource, entry.order)}
				className={styles['ct-vertical-tab-wrap']}
				data-tab-strip-tab-wrapper=''
				data-pin-transition={entry.phase === 'stable' ? undefined : entry.phase}
				data-pin-transition-source={entry.phase === 'source' || undefined}
				aria-hidden={entry.phase === 'source' || undefined}
				style={
					{
						'--ob-comp-tab-strip-block-size': `${
							LAYOUT_CONSTANTS.verticalTabHeight * entry.mainAxisScale
						}px`,
						'--ob-comp-tab-strip-runtime-opacity': entry.opacity,
					} satisfies CSSProperties &
						Record<
							'--ob-comp-tab-strip-block-size' | '--ob-comp-tab-strip-runtime-opacity',
							number | string
						>
				}
				data-drag-source={dragSource || undefined}
			>
				<TabButton
					tab={tab}
					snapshot={snapshot}
					width={width}
					orientation={TabStripOrientation.Vertical}
					collapsed={collapsed}
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
	};
	const renderTabSequence = (
		entries: readonly Readonly<VerticalPinAnimationEntry>[],
	): ReactNode[] => entries.map((entry) => renderTab(entry));

	const pinnedLayoutCollapsed =
		collapsed &&
		(expandedOnHover || availableWidth <= LAYOUT_CONSTANTS.verticalTabStripCollapsedWidth);
	const pinnedLayoutWidth = Math.max(
		0,
		availableWidth - LAYOUT_CONSTANTS.verticalTabStripHorizontalPadding,
	);
	const pinnedLayout = calculateVerticalPinnedPinAnimationLayout({
		sizeBounds: { width: pinnedLayoutWidth, height: null },
		collapseState: pinnedLayoutCollapsed
			? VerticalTabStripCollapseState.Collapsed
			: VerticalTabStripCollapseState.Expanded,
		expandOnHoverEnabled,
		children: pinned,
	});

	const unpinnedDirectChildren: Array<{
		id: string;
		group?: boolean;
		preferredSize: { width: number; height: number };
		minimumSize: { width: number; height: number };
	}> = [];
	const countedGroups = new Set<string>();
	for (const entry of unpinned) {
		const { tab } = entry;
		if (tab.group !== null) {
			if (countedGroups.has(tab.group)) continue;
			countedGroups.add(tab.group);
			const group = groups.get(tab.group);
			const groupTabs = unpinned.filter((candidate) => candidate.tab.group === tab.group);
			const currentGroupExists = snapshot.groups.some((candidate) => candidate.id === tab.group);
			const sourceGroupScale = groupTabs.reduce(
				(scale, candidate) => Math.max(scale, candidate.mainAxisScale),
				0,
			);
			const groupCollapsed = group?.collapsed === true;
			const animatedGroupHeight = currentGroupExists
				? 36 +
					groupTabs.reduce(
						(height, candidate) =>
							height + LAYOUT_CONSTANTS.verticalTabHeight * candidate.mainAxisScale,
						0,
					)
				: (36 + groupTabs.length * LAYOUT_CONSTANTS.verticalTabHeight) * sourceGroupScale;
			unpinnedDirectChildren.push({
				id: `group-${tab.group}`,
				group: true,
				preferredSize: {
					width: availableWidth,
					height: groupCollapsed ? 34 : animatedGroupHeight,
				},
				minimumSize: {
					width: LAYOUT_CONSTANTS.verticalTabMinWidth,
					height: groupCollapsed ? 34 : Math.min(34, animatedGroupHeight),
				},
			});
			continue;
		}
		const animatedTabHeight = LAYOUT_CONSTANTS.verticalTabHeight * entry.mainAxisScale;
		unpinnedDirectChildren.push({
			id: tab.id,
			preferredSize: { width: defaultTabWidth, height: animatedTabHeight },
			minimumSize: {
				width: LAYOUT_CONSTANTS.verticalTabMinWidth,
				height: animatedTabHeight,
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
	const pinnedMinimum = calculatePinnedTabContainerMinimumSize(pinned.length, collapseState);
	const unpinnedMinimum = calculateUnpinnedTabContainerMinimumSize(
		TabStripOrientation.Vertical,
		unpinnedDirectChildren,
	);
	const tabStripViewLayout = calculateTabStripViewLayout({
		orientation: TabStripOrientation.Vertical,
		sizeBounds: {
			width: availableWidth,
			height: regionsSize.height > 0 ? regionsSize.height : null,
		},
		pinnedPreferred: pinnedLayout.hostSize,
		pinnedMinimum: {
			...pinnedMinimum,
			height: Math.min(pinnedMinimum.height, pinnedLayout.hostSize.height),
		},
		pinnedVisible: pinned.length > 0,
		unpinnedPreferred: unpinnedNaturalLayout.hostSize,
		unpinnedMinimum: {
			...unpinnedMinimum,
			height: Math.min(unpinnedMinimum.height, unpinnedNaturalLayout.hostSize.height),
		},
		unpinnedVisible: unpinnedDirectChildren.length > 0,
		separatorPreferredHeight: 1,
		collapsed,
	});
	const unpinnedBoundsById = new Map(
		unpinnedNaturalLayout.children.map((child) => [child.id, child.bounds]),
	);
	const renderedGroups = new Set<string>();

	return (
		<>
			{showTopControls ? (
				<VerticalTabStripTopContainer
					availableWidth={availableWidth}
					collapsed={topContainerCollapsed}
					toolbarHeight={toolbarHeight}
					captionButtonWidth={captionButtonWidth}
					isExitingExpandOnHover={isExitingExpandOnHover}
					showTabSearchButton={showTabSearchButton}
					tabSearchTargetVisible={tabSearchTargetVisible}
					tabSearchAnimating={tabSearchAnimating}
					onTabSearchContextMenu={onTabSearchContextMenu}
					onCollapseContextMenu={onCollapseContextMenu}
					onCollapsedChange={onCollapsedChange}
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
							data-pin-animation={pinned.some((entry) => entry.phase !== 'stable') || undefined}
							style={{ '--ob-comp-tab-strip-block-size': `${pinnedLayout.hostSize.height}px` }}
						>
							{pinnedLayout.children.map((child) => {
								const entry = requireDefined(
									pinned.find((candidate) => candidate.key === child.id),
								);
								const dragSource = isIndividuallyDraggedTab(entry.tab);
								return (
									<div
										key={dragSourceRenderKey(
											entry.key,
											dragSource,
											`${child.bounds.x}:${child.bounds.y}`,
										)}
										className={styles['ct-vertical-pinned__unit']}
										style={{
											'--ob-comp-tab-strip-inline-start': `${child.bounds.x}px`,
											'--ob-comp-tab-strip-block-start': `${child.bounds.y}px`,
											'--ob-comp-tab-strip-inline-size': `${child.bounds.width}px`,
											'--ob-comp-tab-strip-block-size': `${child.bounds.height}px`,
										}}
									>
										{renderTab(entry, child.bounds.width)}
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
						<div
							className={styles['ct-vertical-scroll__content']}
							data-tab-strip-scroll-content=''
							data-tab-count-changed={tabCountTransitionSuppressed || undefined}
							data-pin-animation={unpinned.some((entry) => entry.phase !== 'stable') || undefined}
							data-group-drag-pickup={snappingExpandedGroupPickup || undefined}
							style={{
								'--ob-comp-tab-strip-block-size': `${unpinnedNaturalLayout.hostSize.height}px`,
							}}
						>
							{unpinned.map((entry) => {
								const { tab } = entry;
								const directChildId = tab.group === null ? tab.id : `group-${tab.group}`;
								if (tab.group !== null) {
									if (renderedGroups.has(tab.group)) return null;
									renderedGroups.add(tab.group);
								}
								const bounds = requireDefined(unpinnedBoundsById.get(directChildId));
								let content: ReactNode = renderTab(entry);
								if (tab.group === null) {
									const dragSource = isIndividuallyDraggedTab(tab);
									return (
										<div
											key={dragSourceRenderKey(entry.key, dragSource, bounds.y)}
											className={styles['ct-vertical-unpinned__unit']}
											data-vertical-unpinned-unit={directChildId}
											data-drag-source-unit={dragSource || undefined}
											style={{
												'--ob-comp-tab-strip-block-start': `${bounds.y}px`,
												'--ob-comp-tab-strip-block-size': `${bounds.height}px`,
											}}
										>
											{content}
										</div>
									);
								}
								const group = groups.get(tab.group);
								if (!group) {
									const dragSource = isIndividuallyDraggedTab(tab);
									return (
										<div
											key={dragSourceRenderKey(entry.key, dragSource, bounds.y)}
											className={styles['ct-vertical-unpinned__unit']}
											data-vertical-unpinned-unit={directChildId}
											data-drag-source-unit={dragSource || undefined}
											style={{
												'--ob-comp-tab-strip-block-start': `${bounds.y}px`,
												'--ob-comp-tab-strip-block-size': `${bounds.height}px`,
											}}
										>
											{content}
										</div>
									);
								}
								const groupTabs = unpinned.filter((candidate) => candidate.tab.group === group.id);
								const currentGroupExists = snapshot.groups.some(
									(candidate) => candidate.id === group.id,
								);
								const sourceGroupScale = groupTabs.reduce(
									(scale, candidate) => Math.max(scale, candidate.mainAxisScale),
									0,
								);
								const fullyCollapsed = group.collapsed && fullyCollapsedGroupIds.has(group.id);
								const collapsing = group.collapsed && !fullyCollapsed;
								let groupBlockSize: number;
								if (!currentGroupExists) {
									groupBlockSize =
										(36 + groupTabs.length * LAYOUT_CONSTANTS.verticalTabHeight) * sourceGroupScale;
								} else if (group.collapsed) {
									groupBlockSize = 34;
								} else {
									groupBlockSize =
										36 +
										groupTabs.reduce(
											(height, candidate) =>
												height + LAYOUT_CONSTANTS.verticalTabHeight * candidate.mainAxisScale,
											0,
										);
								}
								content = (
									<section
										className={styles['ct-vertical-group']}
										data-tab-strip-group=''
										data-group-id={group.id}
										data-group-collapsed={fullyCollapsed || undefined}
										data-group-collapsing={collapsing || undefined}
										data-drag-source={draggedWholeGroupIds.has(group.id) || undefined}
										data-pin-transition-source={!currentGroupExists || undefined}
										aria-hidden={!currentGroupExists || undefined}
										style={
											{
												'--ob-comp-tab-strip-group-color': GROUP_COLORS[group.color],
												'--ob-comp-tab-strip-block-size': `${groupBlockSize}px`,
												'--ob-comp-tab-strip-runtime-opacity': currentGroupExists
													? 1
													: sourceGroupScale,
											} satisfies CSSProperties &
												Record<
													| '--ob-comp-tab-strip-block-size'
													| '--ob-comp-tab-strip-group-color'
													| '--ob-comp-tab-strip-runtime-opacity',
													number | string
												>
										}
									>
										<GroupHeader
											group={group}
											tabs={groupTabs.map((candidate) => candidate.tab)}
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
											data-tab-strip-group-tabs=''
											aria-hidden={fullyCollapsed || undefined}
											onTransitionEnd={(event) => onGroupTabsTransitionEnd(event, group.id)}
										>
											<div className={styles['ct-vertical-group__tabs-inner']}>
												{renderTabSequence(groupTabs)}
											</div>
										</div>
									</section>
								);
								return (
									<div
										key={dragSourceRenderKey(
											group.id,
											draggedWholeGroupIds.has(group.id),
											bounds.y,
										)}
										className={styles['ct-vertical-unpinned__unit']}
										data-vertical-unpinned-unit={directChildId}
										data-drag-source-unit={draggedWholeGroupIds.has(group.id) || undefined}
										style={{
											'--ob-comp-tab-strip-block-start': `${bounds.y}px`,
											'--ob-comp-tab-strip-block-size': `${bounds.height}px`,
										}}
									>
										{content}
									</div>
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
