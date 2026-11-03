import styles from './TabStripView.module.css';
import { ContextMenu, ContextMenuTrigger } from '@orchbee/ui/components';
import { type ReactNode } from 'react';
import { TAB_STRIP_ACTION_IDS } from '../../actions/tab-strip-action-contracts';
import { HoverCardUpdateType } from './components/hover-card/model/tab-hover-card-controller';
import { LAYOUT_CONSTANTS } from '../../layout/layout-constants';
import { TabStripContextMenu } from './components/tab-strip-context-menu/TabStripContextMenu';
import { DragGhost } from './components/drag-ghost/DragGhost';
import { HoverCard } from './components/hover-card/HoverCard';
import { HorizontalContents } from './components/horizontal-contents/HorizontalContents';
import { VerticalContents } from './components/vertical-contents/VerticalContents';
import type { TabStripViewModel } from '../../types/tab-strip-view-model';

// TabContainerImpl::AnimateViewTo uses the configured rich-animation duration.

/** Pointer-lifecycle surface implemented by CrossWindowTabDragRuntime. */

export function TabStripView({
	horizontalLeadingActions,
	view,
}: {
	readonly horizontalLeadingActions: ReactNode;
	readonly view: TabStripViewModel;
}): ReactNode {
	const {
		actions,
		identityIcons,
		statusAdornments,
		orientation,
		onOrientationChange,
		verticalCollapsed,
		verticalExpandOnHover,
		showHoverCardPreviews,
		showHoverCardMemoryUsage,
		showGroupHoverCards,
		showVerticalTopControls,
		verticalToolbarHeight,
		verticalCaptionButtonWidth,
		verticalIsExitingExpandOnHover,
		onVerticalCollapsedChange,
		onVerticalTabSearchClick,
		onVerticalExpandOnHoverChange,
		showNewTabButton,
		touchUi,
		onTabKeyboardFocusChanged,
		onRovingFocusChanged,
		onRovingFocusExited,
		rovingFocusKey,
		onMoveTabsToNewWindow,
		onMoveGroupToNewWindow,
		onSetTabTitle,
		onNameGroup,
		shortcutLabels,
		ariaLabel,
		snapshot,
		verticalTabCountTransitionSuppressed,
		horizontal,
		stripRef,
		size,
		menu,
		setMenu,
		hover,
		drag,
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
		repositionDragAfterScroll,
		toggleGroupFromHeader,
		onGroupKeyDown,
		onTabKeyDown,
		onTabKeyUp,
		addTab,
	} = view;
	const closeMenu = () => {
		if (menu?.kind === 'combo-end') comboButtonController.onMenuClosed();
		setMenu(null);
	};
	const openGroupEditorFromControl = (id: string, x: number, y: number) => {
		openGroupEditor(id, x, y);
		const groupRegion = Array.from(
			stripRef.current?.querySelectorAll<HTMLElement>('[data-tab-strip-group]') ?? [],
		).find((candidate) => candidate.dataset['groupId'] === id);
		groupRegion?.dispatchEvent(
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
				data-tab-closing-mode={tabClosingMode?.source}
				onContextMenu={(event) => {
					if (onOrientationChange === undefined) return;
					const target = event.target;
					const ownedContextTarget =
						target instanceof Element
							? target.closest('[data-tab-id], [data-group-id], button, input, a, [role="button"]')
							: null;
					if (ownedContextTarget !== null && event.currentTarget.contains(ownedContextTarget)) {
						return;
					}
					setMenu({ kind: 'strip', x: event.clientX, y: event.clientY });
				}}
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
					if (anchor?.dataset['tabId']) {
						onRovingFocusChanged(`tab:${anchor.dataset['tabId']}`);
					} else if (anchor?.dataset['groupId']) {
						onRovingFocusChanged(`group:${anchor.dataset['groupId']}`);
					} else {
						onRovingFocusChanged(null);
					}
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
						onRovingFocusExited();
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
						identityIcons={identityIcons}
						snapshot={snapshot}
						statusAdornments={statusAdornments}
						tabCountTransitionSuppressed={verticalTabCountTransitionSuppressed}
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
						requestCloseTabs={requestCloseTabs}
						onTabPointerDown={onTabPointerDown}
						onGroupPointerDown={onGroupPointerDown}
						onDragScrolled={repositionDragAfterScroll}
						toggleGroupFromHeader={toggleGroupFromHeader}
						onGroupKeyDown={onGroupKeyDown}
						onTabKeyDown={onTabKeyDown}
						onTabKeyUp={onTabKeyUp}
						rovingFocusKey={rovingFocusKey}
						onTabMouseEnter={onTabMouseEnter}
						onGroupMouseEnter={onGroupMouseEnter}
						showGroupHoverCards={showGroupHoverCards}
						clearHover={clearHover}
						setMenu={setMenu}
						onOpenGroupEditor={openGroupEditorFromControl}
						showNewTabButton={showNewTabButton}
						addTab={addTab}
						newTabCreationEnabled={
							actions.getCreationAvailability({ groupId: null, kind: 'new-tab' }).kind === 'enabled'
						}
						touchUi={touchUi}
						closeButtonAllowed
						showTopControls={showVerticalTopControls}
						showTabSearchButton={comboButtonSnapshot.tabSearchVisible}
						tabSearchTargetVisible={comboButtonSnapshot.tabSearchTargetVisible}
						tabSearchAnimating={comboButtonSnapshot.tabSearchAnimating}
						toolbarHeight={verticalToolbarHeight}
						captionButtonWidth={verticalCaptionButtonWidth}
						isExitingExpandOnHover={verticalIsExitingExpandOnHover}
						onTabSearchContextMenu={(event) => {
							comboButtonController.onMenuOpened('tab-search');
							setMenu({ kind: 'combo-end', x: event.clientX, y: event.clientY });
						}}
						onCollapseContextMenu={(event) => {
							setMenu({ kind: 'collapse', x: event.clientX, y: event.clientY });
						}}
						onCollapsedChange={
							onVerticalCollapsedChange === undefined
								? undefined
								: (collapsed) => {
										if (collapsed) {
											expandOnHoverController.setHovered(false);
											expandOnHoverController.setFocused(false);
										}
										actions.execute(
											TAB_STRIP_ACTION_IDS.setVerticalCollapsed,
											{ value: collapsed },
											'button',
										);
									}
						}
						onTabSearchClick={
							onVerticalTabSearchClick === undefined
								? undefined
								: () => {
										actions.execute(TAB_STRIP_ACTION_IDS.openTabSearch, undefined, 'button');
									}
						}
					/>
				)}
				{drag?.started && !horizontal ? (
					<DragGhost
						drag={drag}
						snapshot={snapshot}
						orientation={orientation}
						collapsed={verticalCollapsed}
						touchUi={touchUi}
						closeButtonAllowed
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
					actions={actions}
					state={menu}
					snapshot={snapshot}
					orientation={orientation}
					onOrientationChange={onOrientationChange}
					closeAllowed
					verticalExpandOnHover={verticalExpandOnHover}
					tabSearchPinned={comboButtonSnapshot.tabSearchPinned}
					onVerticalExpandOnHoverChange={onVerticalExpandOnHoverChange}
					moveTabsToNewWindowAvailable={onMoveTabsToNewWindow !== undefined}
					moveGroupToNewWindowAvailable={onMoveGroupToNewWindow !== undefined}
					onSetTabTitle={onSetTabTitle}
					onNameGroup={onNameGroup}
					shortcutLabels={shortcutLabels}
					onClose={closeMenu}
				/>
			) : null}
		</ContextMenu>
	);
}
