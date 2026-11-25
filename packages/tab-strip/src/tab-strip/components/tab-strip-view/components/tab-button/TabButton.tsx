import styles from './TabButton.module.css';
import { Tooltip, TooltipContent, TooltipTrigger } from '@orchbee/ui/components';
import {
	memo,
	useState,
	type CSSProperties,
	type MouseEvent as ReactMouseEvent,
	type PointerEvent as ReactPointerEvent,
	type ReactNode,
} from 'react';
import { type TabData } from '../../../../model/tab-model';
import { type TabGroupId } from '../../../../model/tab-types';
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
	getTabStripTooltipPlacement,
	joinClassNames,
} from '../../../../utils/tab-strip-internals';
import { useTabButton } from './hooks/useTabButton';
import { useTabStripMessages } from '../../../../hooks/useTabStripMessages';
import type { TabStatusAdornment } from '../../../../types/tab-strip-options';
import type { TabButtonContext } from './utils/tabButtonContext';

/** A Tab whose title is hidden or clipped names itself in its tooltip. */
function tabTitleCutShort(tabView: HTMLElement | null): boolean {
	if (tabView === null) return false;
	const title = tabView.querySelector<HTMLElement>('[data-tab-title]');
	return title === null || title.scrollWidth > title.clientWidth;
}

interface TabButtonProps extends TabButtonContext {
	tab: Readonly<TabData>;
	identityIcon?: ReactNode | undefined;
	statusAdornment?: Readonly<TabStatusAdornment> | undefined;
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
	frameCondensed?: boolean;
	/** The Tab's slot state, read by the parent at render time. Slot objects change in place. */
	slotGroup?: TabGroupId | null | undefined;
	slotDragging?: boolean | undefined;
	slotAnimating?: boolean | undefined;
	slotPinTransition?: TabSlotViewState['pinTransition'] | undefined;
	dragging?: boolean;
	/** Names the Tab in the shared Tooltip while its title is hidden or cut short. */
	titleTooltip?: boolean;
}

/**
 * Renders one Tab. Memoized: the parent passes each value of the Tab's `tabButtonContext` as a
 * prop, so only a Tab whose own state or neighbour's state changed renders again.
 */
export const TabButton = memo(function TabButton({
	tab,
	identityIcon,
	statusAdornment,
	active,
	selected,
	group,
	tabPositionIndex,
	tabCount,
	hoveredByStrip,
	hasPreviousTab,
	previousGroup,
	previousActive,
	previousSelected,
	previousTabHovered,
	hasNextTab,
	nextGroup,
	nextActive,
	nextSelected,
	nextTabHovered,
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
	frameCondensed = false,
	slotGroup,
	slotDragging,
	slotAnimating,
	slotPinTransition,
	dragging = false,
	titleTooltip = false,
}: TabButtonProps): ReactNode {
	const messages = useTabStripMessages();
	const [tooltipOpen, setTooltipOpen] = useState(false);
	const { focused, hovered, measuredTabSize, setFocused, setHovered, tabViewRef } = useTabButton();
	// Chromium's BoundsAnimator lays out a Tab at each animation frame. Observe
	// the transitioning wrapper so child visibility uses that current width
	// instead of jumping directly to the ideal target width.
	const renderedWidth =
		orientation === TabStripOrientation.Horizontal && measuredTabSize.width > 0
			? measuredTabSize.width
			: width;
	const visualHovered = hovered || hoveredByStrip;
	const alertState = tab.alertState ?? 'none';
	const hasAlert = alertState !== 'none';
	const hasFavicon =
		identityIcon !== undefined ||
		statusAdornment !== undefined ||
		tab.shouldDisplayFavicon !== false;
	const hasStatusSymbol =
		statusAdornment?.glyph !== undefined || statusAdornment?.mark !== undefined;
	const horizontalContentsWidth = Math.max(0, renderedWidth - TAB_CONTENT_HORIZONTAL_INSET * 2);
	const childVisibility =
		orientation === TabStripOrientation.Horizontal
			? calculateHorizontalTabChildVisibility({
					width: renderedWidth,
					contentsWidth: horizontalContentsWidth,
					active,
					pinned: tab.pinned,
					pinTransition: slotPinTransition,
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
					hovered: visualHovered,
					focused,
					hasFavicon,
					hasAlert,
					closeAllowed: closeButtonAllowed,
				});
	const pinningControlOffset =
		orientation === TabStripOrientation.Horizontal && slotPinTransition === 'pinning'
			? calculateHorizontalPinnedControlOffset({
					width: renderedWidth,
					pinnedWidth: TAB_SIZE.pinnedTabWidth,
					controlWidth: 16,
					normalControlX: TAB_CONTENT_HORIZONTAL_INSET,
				})
			: 0;
	const separatorBounds = getHorizontalTabSeparatorBounds(renderedWidth);
	const separatorTargets = getHorizontalTabSeparatorTargets({
		tab: {
			active,
			selected,
			hovered: false,
			group: tab.group,
		},
		previous: hasPreviousTab
			? {
					active: previousActive,
					selected: previousSelected,
					hovered: false,
					group: previousGroup,
				}
			: null,
		next: hasNextTab
			? {
					active: nextActive,
					selected: nextSelected,
					hovered: false,
					group: nextGroup,
				}
			: null,
		tabCount,
		declutterEnabled,
	});
	const visibleTitle = getVisibleTabTitle(tab, messages);
	const { side: tooltipSide, align: tooltipAlign } = getTabStripTooltipPlacement(
		orientation,
		collapsed,
		!tab.pinned,
	);
	const tooltipDisabled = dragging || slotDragging === true;
	const content = (
		<span className={styles['ct-tab__content']} data-tab-content=''>
			{childVisibility.showFavicon || hasStatusSymbol ? (
				<TabIcon
					tab={tab}
					active={active}
					availableWidth={Math.max(0, renderedWidth - 18)}
					identityIcon={identityIcon}
					statusAdornment={statusAdornment}
				/>
			) : null}
			{childVisibility.showTitle ? (
				<span className={styles['ct-tab__title']} data-tab-title=''>
					{visibleTitle}
				</span>
			) : null}
			{statusAdornment?.unreadLabel === undefined ? null : (
				<Tooltip disabled={tooltipDisabled}>
					<TooltipTrigger
						render={
							<span aria-hidden='true' className={styles['ct-tab__unread']} data-tab-unread=''>
								●
							</span>
						}
					/>
					<TooltipContent side={tooltipSide} align='center'>
						{statusAdornment.unreadLabel}
					</TooltipContent>
				</Tooltip>
			)}
			{childVisibility.showAlert ? (
				<AlertIndicatorButton
					alertState={alertState}
					dragging={tooltipDisabled}
					tooltipSide={tooltipSide}
				/>
			) : null}
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
	const tabView = (
		<div
			ref={tabViewRef}
			role='tab'
			tabIndex={rovingTabIndex}
			aria-selected={selected}
			aria-posinset={tabPositionIndex + 1}
			aria-setsize={tabCount}
			aria-label={
				statusAdornment === undefined
					? messages.formatTabAccessibleLabel(tab, group)
					: `${messages.formatTabAccessibleLabel(tab, group)}, ${statusAdornment.accessibleLabel}`
			}
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
			data-slot-group={slotGroup ?? undefined}
			data-slot-dragging={slotDragging || dragging || undefined}
			data-slot-animating={slotAnimating || undefined}
			data-pin-compacting={
				(orientation === TabStripOrientation.Horizontal && slotPinTransition === 'pinning') ||
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
	if (!titleTooltip) return tabView;
	return (
		<Tooltip
			disabled={tooltipDisabled}
			open={tooltipOpen}
			onOpenChange={(next) => setTooltipOpen(next && tabTitleCutShort(tabViewRef.current))}
		>
			<TooltipTrigger render={tabView} />
			<TooltipContent side={tooltipSide} align={tooltipAlign}>
				{visibleTitle}
			</TooltipContent>
		</Tooltip>
	);
});
