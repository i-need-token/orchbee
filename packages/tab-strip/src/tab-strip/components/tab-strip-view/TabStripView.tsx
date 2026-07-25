import styles from './TabStripView.module.css';
import { ContextMenu, ContextMenuTrigger } from '@orchbee/ui/components';
import { type ReactNode } from 'react';
import { HoverCardUpdateType } from './components/hover-card/model/tab-hover-card-controller';
import { LAYOUT_CONSTANTS } from '../../layout/layout-constants';
import { TabStripContextMenu } from './components/tab-strip-context-menu/TabStripContextMenu';
import { DragGhost } from './components/drag-ghost/DragGhost';
import { HoverCard } from './components/hover-card/HoverCard';
import { HorizontalContents } from './components/horizontal-contents/HorizontalContents';
import { VerticalContents } from './components/vertical-contents/VerticalContents';
import type { TabStripViewModel } from '../../types/tab-strip-view-model';

// TabContainerImpl::AnimateViewTo uses RichAnimationDuration(200ms).

/** Pointer-lifecycle surface implemented by CrossWindowTabDragRuntime. */

export function TabStripView({
	horizontalLeadingActions,
	view,
}: {
	readonly horizontalLeadingActions: ReactNode;
	readonly view: TabStripViewModel;
}): ReactNode {
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
		stripRef,
		size,
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
		settleTargetLeft,
		focusedGroupColor,
	} = view;
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
					if (anchor && event.currentTarget.contains(anchor)) clearHover(HoverCardUpdateType.Focus);
				}}
			>
				{horizontal ? (
					<HorizontalContents horizontalLeadingActions={horizontalLeadingActions} view={view} />
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
	);
}
