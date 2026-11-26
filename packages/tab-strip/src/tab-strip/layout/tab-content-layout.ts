const HORIZONTAL_TAB_MINIMUM_INACTIVE_WIDTH = 32;
const HORIZONTAL_TAB_MINIMUM_CONTENTS_WIDTH_FOR_CLOSE = 68;
const HORIZONTAL_TAB_TOUCH_MINIMUM_CONTENTS_WIDTH_FOR_CLOSE = 100;
const HORIZONTAL_TAB_DECLUTTER_CLOSE_HIDE_WIDTH = 100;
const HORIZONTAL_TAB_PINNED_NORMAL_RENDER_EXTRA_WIDTH = 30;
const TAB_FAVICON_DESIGN_WIDTH = 16;
const TAB_ALERT_DESIGN_WIDTH = 16;
const TAB_CLOSE_DESIGN_WIDTH = 16;
const TAB_CLOSE_TOUCH_DESIGN_WIDTH = 24;
export const TAB_AFTER_TITLE_PADDING = 4;
const TAB_PRE_TITLE_PADDING = 8;
const TAB_ALERT_CLOSE_PADDING_ADJUSTMENT = 4;
const TAB_ALERT_CLOSE_PADDING_ADJUSTMENT_TOUCH = 8;

/** Port of Tab::Layout's conditional alert-to-close spacing. */
export function calculateHorizontalAlertCloseGap(contentsWidth: number, touchUi = false): number {
	const minimumCloseWidth = touchUi
		? HORIZONTAL_TAB_TOUCH_MINIMUM_CONTENTS_WIDTH_FOR_CLOSE
		: HORIZONTAL_TAB_MINIMUM_CONTENTS_WIDTH_FOR_CLOSE;
	let adjustment = 0;
	if (contentsWidth >= minimumCloseWidth) {
		adjustment = touchUi
			? TAB_ALERT_CLOSE_PADDING_ADJUSTMENT_TOUCH
			: TAB_ALERT_CLOSE_PADDING_ADJUSTMENT;
	}
	return TAB_AFTER_TITLE_PADDING + adjustment;
}

interface TabChildVisibility {
	showFavicon: boolean;
	showAlert: boolean;
	showClose: boolean;
	showTitle: boolean;
	centerSingleControl: boolean;
}

interface HorizontalTabChildVisibilityInput {
	width: number;
	contentsWidth: number;
	height?: number;
	minimumInactiveWidth?: number;
	pinnedWidth?: number;
	active: boolean;
	pinned: boolean;
	pinTransition?: 'pinning' | 'unpinning' | null | undefined;
	hovered?: boolean;
	focused?: boolean;
	closeFocused?: boolean;
	touchUi?: boolean;
	declutterEnabled?: boolean;
	hasFavicon: boolean;
	hasAlert: boolean;
	closeAllowed?: boolean;
}

export function calculateHorizontalPinnedControlOffset({
	width,
	pinnedWidth,
	controlWidth,
	normalControlX,
}: Readonly<{
	width: number;
	pinnedWidth: number;
	controlWidth: number;
	normalControlX: number;
}>): number {
	const compactProgress = Math.min(
		1,
		Math.max(0, 1 - (width - pinnedWidth) / HORIZONTAL_TAB_PINNED_NORMAL_RENDER_EXTRA_WIDTH),
	);
	const pinnedControlX = (pinnedWidth - controlWidth) / 2;
	return Math.round(compactProgress * (pinnedControlX - normalControlX));
}

/** Direct browser-independent port of horizontal Tab::UpdateIconVisibility. */
export function calculateHorizontalTabChildVisibility(
	input: Readonly<HorizontalTabChildVisibilityInput>,
): TabChildVisibility {
	const minimumInactiveWidth = input.minimumInactiveWidth ?? HORIZONTAL_TAB_MINIMUM_INACTIVE_WIDTH;
	if (input.width < minimumInactiveWidth || (input.height ?? 35) < 35) {
		return {
			showFavicon: false,
			showAlert: false,
			showClose: false,
			showTitle: false,
			centerSingleControl: false,
		};
	}

	const hasAlert = input.hasAlert;
	const interacted = Boolean(input.hovered || input.focused || input.closeFocused);
	const declutterEnabled = input.declutterEnabled ?? false;
	const hasFavicon = input.hasFavicon;
	const closeAllowed = input.closeAllowed ?? true;
	const pinnedPresentation = input.pinned || input.pinTransition === 'unpinning';

	if (pinnedPresentation) {
		const renderAsNormal =
			!input.pinned ||
			input.width >= (input.pinnedWidth ?? 64) + HORIZONTAL_TAB_PINNED_NORMAL_RENDER_EXTRA_WIDTH;
		return {
			showFavicon: hasFavicon && !hasAlert,
			showAlert: hasAlert,
			showClose: false,
			showTitle: renderAsNormal,
			centerSingleControl: input.pinned && !renderAsNormal && input.pinTransition !== 'pinning',
		};
	}

	let availableWidth = Math.max(0, input.contentsWidth);
	const closeDesignWidth = input.touchUi ? TAB_CLOSE_TOUCH_DESIGN_WIDTH : TAB_CLOSE_DESIGN_WIDTH;
	const closeWidth = closeDesignWidth + TAB_AFTER_TITLE_PADDING;
	const minimumCloseWidth = input.touchUi
		? HORIZONTAL_TAB_TOUCH_MINIMUM_CONTENTS_WIDTH_FOR_CLOSE
		: HORIZONTAL_TAB_MINIMUM_CONTENTS_WIDTH_FOR_CLOSE;
	const largeEnoughForClose = availableWidth >= minimumCloseWidth;
	let showClose = false;
	let showAlert = false;
	let showFavicon = false;
	let centerSingleControl = false;

	if (input.active) {
		// Compact desktop Chrome uses alert > close > favicon priority. A media
		// alert may defer close until interaction, but a favicon may not: when the
		// favicon and close button cannot coexist, the favicon disappears.
		showClose = closeAllowed;
		if (declutterEnabled && showClose) {
			const alertFitsWithoutClose = hasAlert && TAB_ALERT_DESIGN_WIDTH <= availableWidth;
			const alertFitsWithClose = hasAlert && TAB_ALERT_DESIGN_WIDTH <= availableWidth - closeWidth;
			if (alertFitsWithoutClose && !alertFitsWithClose) showClose = interacted;
		}
		if (showClose) availableWidth -= closeWidth;
		showAlert = hasAlert && TAB_ALERT_DESIGN_WIDTH <= availableWidth;
		if (showAlert) availableWidth -= TAB_ALERT_DESIGN_WIDTH;
		showFavicon = hasFavicon && TAB_FAVICON_DESIGN_WIDTH <= availableWidth;
		if (showFavicon) availableWidth -= TAB_FAVICON_DESIGN_WIDTH;
	} else {
		showAlert = hasAlert && TAB_ALERT_DESIGN_WIDTH <= availableWidth;
		if (showAlert) availableWidth -= TAB_ALERT_DESIGN_WIDTH;
		showFavicon = hasFavicon && TAB_FAVICON_DESIGN_WIDTH <= availableWidth;
		if (showFavicon) availableWidth -= TAB_FAVICON_DESIGN_WIDTH;
		const decluttered =
			declutterEnabled && availableWidth <= HORIZONTAL_TAB_DECLUTTER_CLOSE_HIDE_WIDTH;
		showClose = closeAllowed && largeEnoughForClose && (!decluttered || interacted);
		if (showClose) availableWidth -= closeWidth;

		if (!showClose && !showAlert && !showFavicon) {
			showAlert = hasAlert;
			showFavicon = !showAlert && hasFavicon;
			centerSingleControl = showAlert || showFavicon;
		}
	}

	const visibleControls = Number(showFavicon) + Number(showAlert) + Number(showClose);
	if (
		declutterEnabled &&
		visibleControls === 1 &&
		availableWidth < TAB_PRE_TITLE_PADDING + TAB_AFTER_TITLE_PADDING
	) {
		centerSingleControl = true;
	}

	return {
		showFavicon,
		showAlert,
		showClose,
		showTitle: !centerSingleControl,
		centerSingleControl,
	};
}

/** A title narrower than this is little more than its 12 px fade, so a narrow Tab leaves it out. */
const HORIZONTAL_TAB_MINIMUM_TITLE_WIDTH = 24;
const TAB_FAVICON_TITLE_GAP = 8;
/** The unread dot's cell: the 6 px dot and 4 px on each side. */
const TAB_UNREAD_DESIGN_WIDTH = 14;
/** What the stylesheet keeps clear at the content's end for a close button over it. */
const TAB_CLOSE_RESERVE = 24;
const TAB_CLOSE_TOUCH_RESERVE = 32;

interface NarrowHorizontalTabInput {
	contentsWidth: number;
	active: boolean;
	pinned: boolean;
	touchUi?: boolean;
	interacted: boolean;
	/** Whether the Tab has a favicon or state glyph to draw. */
	hasIcon: boolean;
	hasUnread: boolean;
}

export interface HorizontalTabChildVisibility extends TabChildVisibility {
	/** The close button takes the icon's place, which it hides, instead of the far end. */
	closeOverIcon: boolean;
}

/**
 * Orchbee's narrow horizontal Tab, after Chrome's rules. A Tab whose title has no room to read
 * drops it and centers its icon, with the unread dot beside it. Its close button stops holding a
 * place of its own: the current Tab's waits for the pointer or focus and then takes the icon's
 * place, so nothing moves while it shows; any other Tab's is left out, since a button where its
 * icon was would close a Tab that was only being chosen. A touch has no hover to call the button
 * up, so a touch Tab keeps Chrome's close button.
 */
export function narrowHorizontalTabChildVisibility(
	visibility: Readonly<TabChildVisibility>,
	input: Readonly<NarrowHorizontalTabInput>,
): HorizontalTabChildVisibility {
	const kept = { ...visibility, closeOverIcon: false };
	// Chrome lays out nothing below its minimum width, and leaves pinned Tabs and media alerts alone.
	const laidOut = visibility.showTitle || visibility.centerSingleControl;
	if (input.pinned || !laidOut || visibility.showAlert || !input.hasIcon) return kept;
	const fixed =
		TAB_FAVICON_DESIGN_WIDTH +
		TAB_FAVICON_TITLE_GAP +
		TAB_AFTER_TITLE_PADDING +
		(input.hasUnread ? TAB_UNREAD_DESIGN_WIDTH : 0);
	const closeReserve = input.touchUi ? TAB_CLOSE_TOUCH_RESERVE : TAB_CLOSE_RESERVE;
	const chromeRoom = input.contentsWidth - fixed - (visibility.showClose ? closeReserve : 0);
	if (visibility.showTitle && chromeRoom >= HORIZONTAL_TAB_MINIMUM_TITLE_WIDTH) return kept;
	if (input.touchUi) {
		return {
			...visibility,
			showFavicon: true,
			showTitle: false,
			centerSingleControl: true,
			closeOverIcon: false,
		};
	}
	const moved = input.active && visibility.showClose;
	const showTitle =
		visibility.showTitle && input.contentsWidth - fixed >= HORIZONTAL_TAB_MINIMUM_TITLE_WIDTH;
	return {
		showFavicon: true,
		showAlert: false,
		showClose: moved && input.interacted,
		showTitle,
		centerSingleControl: !showTitle,
		closeOverIcon: moved && input.interacted,
	};
}

const VERTICAL_TAB_HORIZONTAL_INSET = 8;
const VERTICAL_TAB_CHILD_UNCOLLAPSED_MIN_WIDTH = 45;
const VERTICAL_TAB_CLOSE_APPEARANCE_THRESHOLD = VERTICAL_TAB_CHILD_UNCOLLAPSED_MIN_WIDTH - 3;

interface VerticalTabChildVisibilityInput {
	width: number;
	active: boolean;
	pinned: boolean;
	collapsed: boolean;
	hovered?: boolean;
	focused?: boolean;
	closeFocused?: boolean;
	hasFavicon: boolean;
	hasAlert: boolean;
	closeAllowed?: boolean;
}

interface VerticalChildCandidate {
	kind: 'close' | 'alert' | 'favicon' | 'title';
	visible: boolean;
	minWidth: number;
	padding: number;
}

/** Port of vertical TabView::IsChildVisible/CalculateProposedLayout. */
export function calculateVerticalTabChildVisibility(
	input: Readonly<VerticalTabChildVisibilityInput>,
): TabChildVisibility {
	const interacted = Boolean(input.hovered || input.focused || input.closeFocused);
	const centered = input.pinned || input.collapsed;
	const hasAlert = input.hasAlert;
	const closeVisible =
		(input.closeAllowed ?? true) &&
		!input.pinned &&
		(input.width < VERTICAL_TAB_CLOSE_APPEARANCE_THRESHOLD
			? input.active && interacted
			: input.active || interacted);
	const candidates: VerticalChildCandidate[] = [
		{ kind: 'close', visible: closeVisible, minWidth: 16, padding: 4 },
		{ kind: 'alert', visible: hasAlert, minWidth: 16, padding: 4 },
		{
			kind: 'favicon',
			visible: input.hasFavicon && (!input.pinned || !hasAlert),
			minWidth: 16,
			padding: 8,
		},
		{
			kind: 'title',
			visible: !input.pinned,
			minWidth: 10,
			padding: 4,
		},
	];

	const shown = new Set<VerticalChildCandidate['kind']>();
	let remainingWidth = Math.max(0, input.width - VERTICAL_TAB_HORIZONTAL_INSET * 2);
	let placedChildren = 0;
	for (const candidate of candidates) {
		const canRender = centered
			? placedChildren === 0
			: candidate.minWidth + candidate.padding < remainingWidth || placedChildren < 2;
		if (!candidate.visible || !canRender) continue;
		shown.add(candidate.kind);
		placedChildren += 1;
		if (!centered) remainingWidth -= candidate.minWidth + candidate.padding;
	}

	return {
		showFavicon: shown.has('favicon'),
		showAlert: shown.has('alert'),
		showClose: shown.has('close'),
		showTitle: shown.has('title'),
		centerSingleControl: centered,
	};
}
