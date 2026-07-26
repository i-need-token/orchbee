export const HORIZONTAL_TAB_MINIMUM_INACTIVE_WIDTH = 32;
export const HORIZONTAL_TAB_MINIMUM_CONTENTS_WIDTH_FOR_CLOSE = 68;
export const HORIZONTAL_TAB_TOUCH_MINIMUM_CONTENTS_WIDTH_FOR_CLOSE = 100;
export const HORIZONTAL_TAB_DECLUTTER_CLOSE_HIDE_WIDTH = 100;
export const HORIZONTAL_TAB_PINNED_NORMAL_RENDER_EXTRA_WIDTH = 30;
export const TAB_FAVICON_DESIGN_WIDTH = 16;
export const TAB_ALERT_DESIGN_WIDTH = 16;
export const TAB_CLOSE_DESIGN_WIDTH = 16;
export const TAB_CLOSE_TOUCH_DESIGN_WIDTH = 24;
export const TAB_AFTER_TITLE_PADDING = 4;
export const TAB_PRE_TITLE_PADDING = 8;
export const TAB_ALERT_CLOSE_PADDING_ADJUSTMENT = 4;
export const TAB_ALERT_CLOSE_PADDING_ADJUSTMENT_TOUCH = 8;

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

export interface TabChildVisibility {
	showFavicon: boolean;
	showAlert: boolean;
	showClose: boolean;
	showTitle: boolean;
	centerSingleControl: boolean;
}

export interface HorizontalTabChildVisibilityInput {
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

export const VERTICAL_TAB_HORIZONTAL_INSET = 8;
export const VERTICAL_TAB_CHILD_UNCOLLAPSED_MIN_WIDTH = 45;
export const VERTICAL_TAB_CLOSE_APPEARANCE_THRESHOLD = VERTICAL_TAB_CHILD_UNCOLLAPSED_MIN_WIDTH - 3;

export interface VerticalTabChildVisibilityInput {
	width: number;
	active: boolean;
	pinned: boolean;
	collapsed: boolean;
	expandingOnHover?: boolean;
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
	const centered = (input.pinned || input.collapsed) && !input.expandingOnHover;
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
			visible: !input.pinned || Boolean(input.expandingOnHover),
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
