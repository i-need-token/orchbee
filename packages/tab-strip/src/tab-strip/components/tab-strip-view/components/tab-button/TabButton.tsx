import styles from './TabButton.module.css';
import {
	type MouseEvent as ReactMouseEvent,
	type PointerEvent as ReactPointerEvent,
	type ReactNode,
} from 'react';
import { type TabData, type TabModelSnapshot } from '../../../../model/tab-model';
import { type TabSlotViewState } from '../../../../model/TabSlotViewState';
import {
	TAB_AFTER_TITLE_PADDING,
	calculateHorizontalAlertCloseGap,
	calculateHorizontalTabChildVisibility,
	calculateVerticalTabChildVisibility,
} from '../../../../layout/tab-content-layout';
import { type TabCloseSource } from '../../../../layout/tab-closing-mode';
import { TabStripOrientation } from '../../../../types/tab-strip-types';
import { LAYOUT_CONSTANTS } from '../../../../layout/layout-constants';
import {
	TAB_CONTENT_HORIZONTAL_INSET,
	getHorizontalTabPath,
	getHorizontalTabSeparatorBounds,
	getHorizontalTabSeparatorTargets,
} from '../../../../layout/tab-style-views';
import { AlertIndicatorButton } from './components/alert-indicator-button/AlertIndicatorButton';
import { TabCloseButton } from './components/tab-close-button/TabCloseButton';
import { TabIcon } from './components/tab-icon/TabIcon';
import { getVisibleTabTitle } from './utils/tab-title';
import {
	TAB_SIZE,
	joinClassNames,
	tabIsApparentlyActive,
} from '../../../../utils/tab-strip-internals';
import { useTabButton } from './hooks/useTabButton';
import { useTabStripMessages } from '../../../../hooks/useTabStripMessages';

export function TabButton({
	tab,
	snapshot,
	width,
	orientation,
	collapsed,
	closing = false,
	onPointerDown,
	onClose,
	onContextMenu,
	onMouseEnter,
	onMouseLeave,
	onKeyDown,
	onKeyUp,
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
	snapshot: TabModelSnapshot;
	width: number;
	orientation: TabStripOrientation;
	collapsed: boolean;
	closing?: boolean;
	onPointerDown: (event: ReactPointerEvent<HTMLDivElement>, tab: Readonly<TabData>) => void;
	onClose: (event: ReactMouseEvent, tabId: string, source: TabCloseSource) => void;
	onContextMenu: (event: ReactMouseEvent, tabId: string) => void;
	onMouseEnter: (event: ReactMouseEvent<HTMLDivElement>, tabId: string) => void;
	onMouseLeave: () => void;
	onKeyDown: (event: React.KeyboardEvent, tabId: string) => void;
	onKeyUp: (event: React.KeyboardEvent, tabId: string) => void;
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
	// Closing views remain mounted for the shrink animation, but Chromium no
	// longer paints them as active during that interval.
	const { focused, hovered, measuredTabSize, setFocused, setHovered, tabViewRef } = useTabButton();
	// Chromium's BoundsAnimator lays out a Tab at each animation frame. Observe
	// the transitioning wrapper so paths and child visibility use that current
	// width instead of jumping directly to the ideal target width.
	const renderedWidth =
		orientation === TabStripOrientation.Horizontal && measuredTabSize.width > 0
			? measuredTabSize.width
			: width;
	const active = !closing && tabIsApparentlyActive(tab, snapshot);
	const selected = snapshot.selectedTabIds.has(tab.id);
	const visualHovered = hovered || hoveredTabId === tab.id;
	const alertState = tab.alertState ?? 'none';
	const hasAlert = alertState !== 'none';
	const hasFavicon = tab.shouldDisplayFavicon !== false;
	const horizontalContentsWidth = Math.max(0, renderedWidth - TAB_CONTENT_HORIZONTAL_INSET * 2);
	const childVisibility =
		orientation === TabStripOrientation.Horizontal
			? calculateHorizontalTabChildVisibility({
					width: renderedWidth,
					contentsWidth: horizontalContentsWidth,
					active,
					pinned: tab.pinned,
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
	const group =
		tab.group === null
			? null
			: (snapshot.groups.find((candidate) => candidate.id === tab.group) ?? null);
	const separatorBounds = getHorizontalTabSeparatorBounds(renderedWidth);
	const tabIndex = snapshot.tabs.findIndex((candidate) => candidate.id === tab.id);
	const separatorStateFor = (candidate: Readonly<TabData> | undefined) =>
		candidate
			? {
					active: tabIsApparentlyActive(candidate, snapshot),
					selected: snapshot.selectedTabIds.has(candidate.id),
					hovered: hoveredTabId === candidate.id,
					group: candidate.group,
				}
			: null;
	const separatorTargets = getHorizontalTabSeparatorTargets({
		tab: {
			active,
			selected,
			hovered: visualHovered,
			group: tab.group,
		},
		previous: separatorStateFor(snapshot.tabs[tabIndex - 1]),
		next: separatorStateFor(snapshot.tabs[tabIndex + 1]),
		tabCount: snapshot.tabs.length,
		declutterEnabled,
	});
	return (
		<div
			ref={tabViewRef}
			role='tab'
			tabIndex={-1}
			aria-selected={selected}
			aria-label={messages.formatTabAccessibleLabel(tab, group)}
			title=''
			className={styles['ct-tab']}
			data-tab-id={tab.id}
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
			data-collapsed={collapsed || undefined}
			data-slot-group={slotState?.group ?? undefined}
			data-slot-dragging={slotState?.dragging || dragging || undefined}
			data-slot-animating={slotState?.animating || undefined}
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
					<svg
						className={styles['ct-tab__background']}
						viewBox={`0 0 ${Math.max(1, renderedWidth)} ${LAYOUT_CONSTANTS.tabStripHeight}`}
						preserveAspectRatio='none'
						aria-hidden='true'
					>
						<path
							className={styles['ct-tab__hit-target']}
							d={getHorizontalTabPath({
								width: renderedWidth,
								pathType: 'hit-test',
								closing,
								frameCondensed,
							})}
						/>
						<path
							className={styles['ct-tab__background-fill']}
							d={getHorizontalTabPath({
								width: renderedWidth,
								pathType: active ? 'active' : 'highlight',
								closing,
							})}
						/>
					</svg>
					<span
						className={joinClassNames(
							styles['ct-tab__separator'],
							styles['ct-tab__separator--leading'],
						)}
						data-tab-separator='leading'
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
				<span className={styles['ct-tab__background']} />
			)}
			<span className={styles['ct-tab__content']}>
				{childVisibility.showFavicon ? (
					<TabIcon tab={tab} availableWidth={Math.max(0, renderedWidth - 18)} />
				) : null}
				{childVisibility.showTitle ? (
					<span className={styles['ct-tab__title']}>{getVisibleTabTitle(tab)}</span>
				) : null}
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
		</div>
	);
}
