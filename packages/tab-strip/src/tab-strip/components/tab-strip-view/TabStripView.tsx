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

// TabContainerImpl::AnimateViewTo uses the configured rich-animation duration.

export function TabStripView({
	horizontalLeadingActions,
	horizontalTrailingActions,
	view,
}: {
	readonly horizontalLeadingActions: ReactNode;
	readonly horizontalTrailingActions: ReactNode;
	readonly view: TabStripViewModel;
}): ReactNode {
	const {
		actions,
		identityIcons,
		statusAdornments,
		orientation,
		verticalCollapsed,
		showHoverCardPreviews,
		showHoverCardMemoryUsage,
		showGroupHoverCards,
		showNewTabButton,
		touchUi,
		verticalFill,
		onTabKeyboardFocusChanged,
		onRovingFocusChanged,
		onRovingFocusExited,
		rovingFocusKey,
		closeResume,
		onSetTabTitle,
		onNameGroup,
		newTabSections,
		newTabMenuAvailable,
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

	const fill = !horizontal && verticalFill;

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
				data-fill={fill || undefined}
				data-collapsed={verticalCollapsed || undefined}
				data-hover-card-waiting={hoverCardSnapshot.waitingToShow || undefined}
				data-dragging={(drag?.started && !drag.settling) || undefined}
				data-tab-closing-mode={tabClosingMode?.source}
				onScrollCapture={() => {
					clearHover(HoverCardUpdateType.Event);
					if (menu?.kind === 'group' && groupEditorTracker.onContentsScrolled()) {
						setMenu(null);
					}
				}}
				onFocusCapture={(event) => {
					const target = event.target;
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
					<HorizontalContents
						horizontalLeadingActions={horizontalLeadingActions}
						horizontalTrailingActions={horizontalTrailingActions}
						view={view}
					/>
				) : (
					<VerticalContents
						identityIcons={identityIcons}
						snapshot={snapshot}
						statusAdornments={statusAdornments}
						tabCountTransitionSuppressed={verticalTabCountTransitionSuppressed}
						collapsed={verticalCollapsed}
						availableWidth={
							size.width ||
							(verticalCollapsed ? LAYOUT_CONSTANTS.verticalTabStripCollapsedWidth : 240)
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
						newTabMenuAvailable={newTabMenuAvailable}
						touchUi={touchUi}
						closeButtonAllowed
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
				{/* A full-width list leaves no room beside its rows for a card, and shows each title whole. */}
				{hover &&
				!fill &&
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
					closeAllowed
					closeResume={closeResume}
					onSetTabTitle={onSetTabTitle}
					onNameGroup={onNameGroup}
					newTabSections={newTabSections}
					onClose={closeMenu}
				/>
			) : null}
		</ContextMenu>
	);
}
