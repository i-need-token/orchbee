import styles from './TabButton.module.css';
import {
	type CSSProperties,
	type MouseEvent as ReactMouseEvent,
	type PointerEvent as ReactPointerEvent,
	type ReactNode,
} from 'react';
import { type TabData, type TabModelSnapshot } from '../../../../model/tab-model';
import { type TabSlotViewState } from '../../../../model/TabSlotViewState';
import {
	TAB_AFTER_TITLE_PADDING,
	calculateHorizontalAlertCloseGap,
	calculateHorizontalPinnedControlOffset,
	calculateHorizontalTabChildVisibility,
	calculateVerticalTabChildVisibility,
} from '../../../../layout/tab-content-layout';
import { type TabCloseSource } from '../../../../layout/tab-closing-mode';
import { TabStripOrientation } from '../../../../types/tab-strip-types';
import {
	TAB_CONTENT_HORIZONTAL_INSET,
	getHorizontalTabSeparatorBounds,
	getHorizontalTabSeparatorTargets,
} from '../../../../layout/tab-style-views';
import { AlertIndicatorButton } from './components/alert-indicator-button/AlertIndicatorButton';
import { TabCloseButton } from './components/tab-close-button/TabCloseButton';
import { TabIcon } from './components/tab-icon/TabIcon';
import { getVisibleTabTitle } from './utils/getVisibleTabTitle';
import {
	TAB_SIZE,
	joinClassNames,
	tabIsApparentlyActive,
} from '../../../../utils/tab-strip-internals';
import { useTabButton } from './hooks/useTabButton';
import { useTabStripMessages } from '../../../../hooks/useTabStripMessages';
import type { TabStatusAdornment } from '../../../../types/tab-strip-options';

export function TabButton({
	tab,
	identityIcon,
	statusAdornment,
	snapshot,
	width,
	orientation,
	collapsed,
	onPointerDown,
	onClose,
	onContextMenu,
	onMouseEnter,
	onMouseLeave,
	onKeyDown,
	onKeyUp,
	tabIndex: rovingTabIndex,
	touchUi,
	closeButtonAllowed,
	declutterEnabled = false,
	hoveredTabId,
	verticalExpandedOnHover = false,
	frameCondensed = false,
	slotState,
	dragging = false,
}: {
	tab: Readonly<TabData>;
	identityIcon?: ReactNode | undefined;
	statusAdornment?: Readonly<TabStatusAdornment> | undefined;
	snapshot: TabModelSnapshot;
	width: number;
	orientation: TabStripOrientation;
	collapsed: boolean;
	onPointerDown: (event: ReactPointerEvent<HTMLDivElement>, tab: Readonly<TabData>) => void;
	onClose: (event: ReactMouseEvent, tabId: string, source: TabCloseSource) => void;
	onContextMenu: (event: ReactMouseEvent, tabId: string) => void;
	onMouseEnter: (event: ReactMouseEvent<HTMLDivElement>, tabId: string) => void;
	onMouseLeave: () => void;
	onKeyDown: (event: React.KeyboardEvent, tabId: string) => void;
	onKeyUp: (event: React.KeyboardEvent, tabId: string) => void;
	tabIndex: -1 | 0;
	touchUi: boolean;
	closeButtonAllowed: boolean;
	declutterEnabled?: boolean;
	hoveredTabId?: string | null;
	verticalExpandedOnHover?: boolean;
	frameCondensed?: boolean;
	slotState?: Readonly<TabSlotViewState> | undefined;
	dragging?: boolean;
}): ReactNode {
	const messages = useTabStripMessages();
	const { focused, hovered, measuredTabSize, setFocused, setHovered, tabViewRef } = useTabButton();
	// Chromium's BoundsAnimator lays out a Tab at each animation frame. Observe
	// the transitioning wrapper so child visibility uses that current width
	// instead of jumping directly to the ideal target width.
	const renderedWidth =
		orientation === TabStripOrientation.Horizontal && measuredTabSize.width > 0
			? measuredTabSize.width
			: width;
	const active = tabIsApparentlyActive(tab, snapshot);
	const selected = snapshot.selectedTabIds.has(tab.id);
	const visualHovered = hovered || hoveredTabId === tab.id;
	const alertState = tab.alertState ?? 'none';
	const hasAlert = alertState !== 'none';
	const hasFavicon =
		identityIcon !== undefined ||
		statusAdornment !== undefined ||
		tab.shouldDisplayFavicon !== false;
	const horizontalContentsWidth = Math.max(0, renderedWidth - TAB_CONTENT_HORIZONTAL_INSET * 2);
	const childVisibility =
		orientation === TabStripOrientation.Horizontal
			? calculateHorizontalTabChildVisibility({
					width: renderedWidth,
					contentsWidth: horizontalContentsWidth,
					active,
					pinned: tab.pinned,
					pinTransition: slotState?.pinTransition,
					hovered: visualHovered,
					focused,
					touchUi,
					declutterEnabled,
					hasFavicon,
					hasAlert,
					closeAllowed: closeButtonAllowed,
					pinnedWidth: TAB_SIZE.pinnedTabWidth,
				})
			: calculateVerticalTabChildVisibility({
					width,
					active,
					pinned: tab.pinned,
					collapsed,
					expandingOnHover: verticalExpandedOnHover,
					hovered: visualHovered,
					focused,
					hasFavicon,
					hasAlert,
					closeAllowed: closeButtonAllowed,
				});
	const pinningControlOffset =
		orientation === TabStripOrientation.Horizontal && slotState?.pinTransition === 'pinning'
			? calculateHorizontalPinnedControlOffset({
					width: renderedWidth,
					pinnedWidth: TAB_SIZE.pinnedTabWidth,
					controlWidth: 16,
					normalControlX: TAB_CONTENT_HORIZONTAL_INSET,
				})
			: 0;
	const group =
		tab.group === null
			? null
			: (snapshot.groups.find((candidate) => candidate.id === tab.group) ?? null);
	const separatorBounds = getHorizontalTabSeparatorBounds(renderedWidth);
	const tabPositionIndex = snapshot.tabs.findIndex((candidate) => candidate.id === tab.id);
	const separatorStateFor = (candidate: Readonly<TabData> | undefined) =>
		candidate
			? {
					active: tabIsApparentlyActive(candidate, snapshot),
					selected: snapshot.selectedTabIds.has(candidate.id),
					hovered: false,
					group: candidate.group,
				}
			: null;
	const separatorTargets = getHorizontalTabSeparatorTargets({
		tab: {
			active,
			selected,
			hovered: false,
			group: tab.group,
		},
		previous: separatorStateFor(snapshot.tabs[tabPositionIndex - 1]),
		next: separatorStateFor(snapshot.tabs[tabPositionIndex + 1]),
		tabCount: snapshot.tabs.length,
		declutterEnabled,
	});
	const previousTabHovered = hoveredTabId === snapshot.tabs[tabPositionIndex - 1]?.id;
	const nextTabHovered = hoveredTabId === snapshot.tabs[tabPositionIndex + 1]?.id;
	const content = (
		<span className={styles['ct-tab__content']} data-tab-content=''>
			{childVisibility.showFavicon || statusAdornment?.glyph !== undefined ? (
				<TabIcon
					tab={tab}
					availableWidth={Math.max(0, renderedWidth - 18)}
					identityIcon={identityIcon}
					statusAdornment={statusAdornment}
				/>
			) : null}
			{childVisibility.showTitle ? (
				<span className={styles['ct-tab__title']}>{getVisibleTabTitle(tab, messages)}</span>
			) : null}
			{statusAdornment?.unreadLabel === undefined ? null : (
				<span
					aria-hidden='true'
					className={styles['unread']}
					data-tab-unread=''
					title={statusAdornment.unreadLabel}
				>
					●
				</span>
			)}
			{childVisibility.showAlert ? <AlertIndicatorButton alertState={alertState} /> : null}
			{childVisibility.showClose ? (
				<TabCloseButton
					tabTitle={tab.title}
					touchUi={touchUi}
					onClose={(event, pointerType) =>
						onClose(event, tab.id, pointerType === 'touch' ? 'touch' : 'mouse')
					}
				/>
			) : null}
		</span>
	);
	return (
		<div
			ref={tabViewRef}
			role='tab'
			tabIndex={rovingTabIndex}
			aria-selected={selected}
			aria-posinset={tabPositionIndex + 1}
			aria-setsize={snapshot.tabs.length}
			aria-label={
				statusAdornment === undefined
					? messages.formatTabAccessibleLabel(tab, group)
					: `${messages.formatTabAccessibleLabel(tab, group)}, ${statusAdornment.accessibleLabel}`
			}
			title=''
			className={styles['ct-tab']}
			data-tab-id={tab.id}
			data-tab-strip-roving-key={`tab:${tab.id}`}
			data-active={active || undefined}
			data-selected={selected || undefined}
			data-hovered={visualHovered || undefined}
			data-pinned={tab.pinned || undefined}
			data-grouped={tab.group !== null || undefined}
			data-render-normal={
				(orientation === TabStripOrientation.Horizontal &&
					tab.pinned &&
					childVisibility.showTitle) ||
				undefined
			}
			data-center-control={childVisibility.centerSingleControl || undefined}
			data-horizontal-close={
				(orientation === TabStripOrientation.Horizontal && childVisibility.showClose) || undefined
			}
			data-visible-alert={childVisibility.showAlert || undefined}
			data-compact-alert-close-gap={
				(orientation === TabStripOrientation.Horizontal &&
					childVisibility.showAlert &&
					childVisibility.showClose &&
					calculateHorizontalAlertCloseGap(horizontalContentsWidth, touchUi) ===
						TAB_AFTER_TITLE_PADDING) ||
				undefined
			}
			data-layout-closed={
				(!childVisibility.showFavicon &&
					!childVisibility.showAlert &&
					!childVisibility.showClose &&
					!childVisibility.showTitle) ||
				undefined
			}
			data-orientation={orientation}
			data-frame-condensed={
				(orientation === TabStripOrientation.Horizontal && frameCondensed) || undefined
			}
			data-collapsed={collapsed || undefined}
			data-slot-group={slotState?.group ?? undefined}
			data-slot-dragging={slotState?.dragging || dragging || undefined}
			data-slot-animating={slotState?.animating || undefined}
			data-pin-compacting={
				(orientation === TabStripOrientation.Horizontal &&
					slotState?.pinTransition === 'pinning') ||
				undefined
			}
			style={
				{
					'--ob-comp-tab-strip-pin-control-offset': `${pinningControlOffset}px`,
				} satisfies CSSProperties & Record<'--ob-comp-tab-strip-pin-control-offset', string>
			}
			onPointerDown={(event) => onPointerDown(event, tab)}
			onAuxClick={(event) => {
				if (event.button === 1) onClose(event, tab.id, 'mouse');
			}}
			onContextMenu={(event) => onContextMenu(event, tab.id)}
			onMouseEnter={(event) => {
				setHovered(true);
				onMouseEnter(event, tab.id);
			}}
			onMouseLeave={() => {
				setHovered(false);
				onMouseLeave();
			}}
			onFocusCapture={() => setFocused(true)}
			onBlurCapture={(event) => {
				if (
					!(event.relatedTarget instanceof Node) ||
					!event.currentTarget.contains(event.relatedTarget)
				)
					setFocused(false);
			}}
			onKeyDown={(event) => onKeyDown(event, tab.id)}
			onKeyUp={(event) => onKeyUp(event, tab.id)}
		>
			{orientation === TabStripOrientation.Horizontal ? (
				<>
					<span
						className={styles['ct-tab__hit-target']}
						data-tab-hit-target=''
						aria-hidden='true'
					/>
					<span
						className={styles['ct-tab__surface']}
						data-tab-surface=''
						data-tab-background-fill=''
					>
						{content}
					</span>
					<span
						className={joinClassNames(
							styles['ct-tab__separator'],
							styles['ct-tab__separator--leading'],
						)}
						data-tab-separator='leading'
						data-hover-suppressed={visualHovered || previousTabHovered || undefined}
						style={{
							'--ob-comp-tab-strip-inline-start': `${separatorBounds.leading.x}px`,
							'--ob-comp-tab-strip-block-start': `${separatorBounds.leading.y}px`,
							'--ob-comp-tab-strip-inline-size': `${separatorBounds.leading.width}px`,
							'--ob-comp-tab-strip-block-size': `${separatorBounds.leading.height}px`,
							'--ob-comp-tab-strip-runtime-opacity': separatorTargets.leading,
						}}
					/>
					<span
						className={joinClassNames(
							styles['ct-tab__separator'],
							styles['ct-tab__separator--trailing'],
						)}
						data-tab-separator='trailing'
						data-hover-suppressed={visualHovered || nextTabHovered || undefined}
						style={{
							'--ob-comp-tab-strip-inline-start': `${separatorBounds.trailing.x}px`,
							'--ob-comp-tab-strip-block-start': `${separatorBounds.trailing.y}px`,
							'--ob-comp-tab-strip-inline-size': `${separatorBounds.trailing.width}px`,
							'--ob-comp-tab-strip-block-size': `${separatorBounds.trailing.height}px`,
							'--ob-comp-tab-strip-runtime-opacity': separatorTargets.trailing,
						}}
					/>
				</>
			) : (
				<>
					<span className={styles['ct-tab__background']} data-tab-state-layer=''>
						<span className={styles['ct-tab__hover-layer']} data-tab-hover-layer='' />
					</span>
					{content}
				</>
			)}
		</div>
	);
}
