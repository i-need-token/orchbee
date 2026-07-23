import { LAYOUT_CONSTANTS } from './layout-constants';

export const TAB_TOP_CORNER_RADIUS = 8;
export const TAB_BOTTOM_CORNER_RADIUS = 10;
export const TAB_SEPARATOR_WIDTH = 2;
export const TAB_SEPARATOR_HEIGHT = 16;
export const TAB_SEPARATOR_HORIZONTAL_MARGIN = 2;
export const TAB_HORIZONTAL_CONTENT_PADDING = 8;
export const TAB_VERTICAL_CONTENT_PADDING = 6;
export const TAB_CONTENT_HORIZONTAL_INSET =
	TAB_BOTTOM_CORNER_RADIUS + TAB_HORIZONTAL_CONTENT_PADDING;
export const TAB_CONTENT_TOP_INSET =
	LAYOUT_CONSTANTS.tabStripPadding + TAB_VERTICAL_CONTENT_PADDING;
export const TAB_CONTENT_BOTTOM_INSET =
	LAYOUT_CONSTANTS.tabStripPadding +
	TAB_VERTICAL_CONTENT_PADDING +
	LAYOUT_CONSTANTS.tabstripToolbarOverlap;
export const TAB_SEPARATOR_OVERLAP = TAB_SEPARATOR_HORIZONTAL_MARGIN * 2 + TAB_SEPARATOR_WIDTH;
export const TAB_STRIP_DECLUTTER_MIN_TABS_FOR_SEPARATOR_HIDE = 20;

export type HorizontalTabPathType = 'active' | 'border' | 'highlight' | 'hit-test';

function clamp(value: number, minimum: number, maximum: number): number {
	return Math.min(maximum, Math.max(minimum, value));
}

/** Port of TabStyleViewsImpl::GetTopCornerRadiusForWidth. */
export function getHorizontalTabTopCornerRadius(width: number): number {
	const topWidth = width - TAB_TOP_CORNER_RADIUS * 2;
	return clamp(topWidth / 3, 0, TAB_TOP_CORNER_RADIUS);
}

function roundedRectPath(input: {
	left: number;
	top: number;
	right: number;
	bottom: number;
	topLeft: number;
	topRight: number;
	bottomRight: number;
	bottomLeft: number;
}): string {
	const { left, top, right, bottom } = input;
	const tl = Math.max(0, input.topLeft);
	const tr = Math.max(0, input.topRight);
	const br = Math.max(0, input.bottomRight);
	const bl = Math.max(0, input.bottomLeft);
	return [
		`M ${left + tl} ${top}`,
		`L ${right - tr} ${top}`,
		tr ? `A ${tr} ${tr} 0 0 1 ${right} ${top + tr}` : `L ${right} ${top}`,
		`L ${right} ${bottom - br}`,
		br ? `A ${br} ${br} 0 0 1 ${right - br} ${bottom}` : `L ${right} ${bottom}`,
		`L ${left + bl} ${bottom}`,
		bl ? `A ${bl} ${bl} 0 0 1 ${left} ${bottom - bl}` : `L ${left} ${bottom}`,
		`L ${left} ${top + tl}`,
		tl ? `A ${tl} ${tl} 0 0 1 ${left + tl} ${top}` : `L ${left} ${top}`,
		'Z',
	].join(' ');
}

/**
 * SVG translation of TabStyleViewsImpl::GetPath at one device-independent
 * pixel per SVG unit. The browser still performs final device-pixel snapping.
 */
export function getHorizontalTabPath(input: {
	width: number;
	pathType: HorizontalTabPathType;
	closing?: boolean;
	frameCondensed?: boolean;
	/** TabStyleViewsImpl::GetStrokeThickness, in DIPs. */
	strokeThickness?: number;
}): string {
	const width = Math.max(0, input.width);
	let contentRadius = getHorizontalTabTopCornerRadius(width);
	let extensionRadius = TAB_BOTTOM_CORNER_RADIUS;
	let top = LAYOUT_CONSTANTS.tabStripPadding;

	if (input.pathType === 'highlight' || input.pathType === 'hit-test') {
		let left = extensionRadius;
		let right = width - extensionRadius;
		const limitedTabSpace = right - left < 16;
		const expandLeft = limitedTabSpace || input.pathType === 'hit-test';
		const expandRight = limitedTabSpace || input.pathType === 'hit-test';
		if (!input.closing && expandLeft) left -= TAB_SEPARATOR_OVERLAP / 2;
		if (!input.closing && expandRight) right += TAB_SEPARATOR_OVERLAP / 2;

		const bottom =
			input.pathType === 'hit-test'
				? top + LAYOUT_CONSTANTS.tabHeight
				: top +
					LAYOUT_CONSTANTS.tabHeight -
					LAYOUT_CONSTANTS.tabStripPadding -
					LAYOUT_CONSTANTS.tabstripToolbarOverlap;
		const extendHitTestToTop = input.pathType === 'hit-test' && input.frameCondensed;
		if (extendHitTestToTop) top -= LAYOUT_CONSTANTS.tabStripPadding;
		return roundedRectPath({
			left,
			top,
			right,
			bottom,
			topLeft: extendHitTestToTop ? 0 : contentRadius,
			topRight: extendHitTestToTop ? 0 : contentRadius,
			bottomRight: input.pathType === 'hit-test' ? 0 : contentRadius,
			bottomLeft: input.pathType === 'hit-test' ? 0 : contentRadius,
		});
	}

	const left = 0;
	const right = width;
	const extendedBottom = LAYOUT_CONSTANTS.tabStripHeight;
	let tabBottom = extendedBottom - LAYOUT_CONSTANTS.tabstripToolbarOverlap;
	let tabLeft = left + extensionRadius;
	let tabRight = right - extensionRadius;
	let topLeftRadius = contentRadius;
	let topRightRadius = contentRadius;

	const strokeAdjustment = Math.max(
		0,
		input.strokeThickness ?? (input.pathType === 'border' ? 2 : 0),
	);
	if (input.pathType === 'active' || input.pathType === 'border') {
		tabLeft += strokeAdjustment / 2;
		tabRight -= strokeAdjustment / 2;
		top += strokeAdjustment / 2;
		contentRadius = Math.max(0, contentRadius - strokeAdjustment / 2);
		topLeftRadius = contentRadius;
		topRightRadius = contentRadius;
		tabBottom -= strokeAdjustment / 2;
		extensionRadius -= strokeAdjustment / 2;
	}

	const path: string[] = [];
	path.push(`M ${left} ${extendedBottom}`);
	if (tabBottom !== extendedBottom) {
		path.push(input.pathType === 'border' ? `M ${left} ${tabBottom}` : `L ${left} ${tabBottom}`);
	}
	path.push(`L ${tabLeft - extensionRadius} ${tabBottom}`);
	path.push(
		extensionRadius
			? `A ${extensionRadius} ${extensionRadius} 0 0 0 ${tabLeft} ${tabBottom - extensionRadius}`
			: `L ${tabLeft} ${tabBottom}`,
	);
	path.push(`L ${tabLeft} ${top + topLeftRadius}`);
	path.push(
		topLeftRadius
			? `A ${topLeftRadius} ${topLeftRadius} 0 0 1 ${tabLeft + topLeftRadius} ${top}`
			: `L ${tabLeft} ${top}`,
	);

	path.push(`L ${tabRight - topRightRadius} ${top}`);
	path.push(
		topRightRadius
			? `A ${topRightRadius} ${topRightRadius} 0 0 1 ${tabRight} ${top + topRightRadius}`
			: `L ${tabRight} ${top}`,
	);
	path.push(`L ${tabRight} ${tabBottom - extensionRadius}`);
	path.push(
		extensionRadius
			? `A ${extensionRadius} ${extensionRadius} 0 0 0 ${tabRight + extensionRadius} ${tabBottom}`
			: `L ${tabRight} ${tabBottom}`,
	);
	if (tabBottom !== extendedBottom) path.push(`L ${right} ${tabBottom}`);
	if (input.pathType !== 'border') path.push(`L ${right} ${extendedBottom}`);

	if (input.pathType !== 'border') path.push('Z');
	return path.join(' ');
}

/**
 * DIP-space equivalent of Tab::GetHitTestMask for callers that cannot use an
 * SVG hit target (for example unit tests and host integrations).
 */
export function isPointInHorizontalTabHitTest(input: {
	width: number;
	x: number;
	y: number;
	closing?: boolean;
	frameCondensed?: boolean;
}): boolean {
	const width = Math.max(0, input.width);
	const radius = getHorizontalTabTopCornerRadius(width);
	let left = TAB_BOTTOM_CORNER_RADIUS;
	let right = width - TAB_BOTTOM_CORNER_RADIUS;
	if (!input.closing) {
		left -= TAB_SEPARATOR_OVERLAP / 2;
		right += TAB_SEPARATOR_OVERLAP / 2;
	}
	const top = input.frameCondensed ? 0 : LAYOUT_CONSTANTS.tabStripPadding;
	const bottom = LAYOUT_CONSTANTS.tabStripPadding + LAYOUT_CONSTANTS.tabHeight;
	if (input.x < left || input.x > right || input.y < top || input.y > bottom) return false;
	if (input.frameCondensed || input.y >= top + radius) return true;

	const leftRadius = radius;
	const rightRadius = radius;
	if (leftRadius > 0 && input.x < left + leftRadius) {
		const dx = input.x - (left + leftRadius);
		const dy = input.y - (top + leftRadius);
		return dx * dx + dy * dy <= leftRadius * leftRadius;
	}
	if (rightRadius > 0 && input.x > right - rightRadius) {
		const dx = input.x - (right - rightRadius);
		const dy = input.y - (top + rightRadius);
		return dx * dx + dy * dy <= rightRadius * rightRadius;
	}
	return true;
}

/** Port of TabStyleViewsImpl::GetSeparatorBounds at scale 1. */
export function getHorizontalTabSeparatorBounds(width: number): Readonly<{
	leading: Readonly<{ x: number; y: number; width: number; height: number }>;
	trailing: Readonly<{ x: number; y: number; width: number; height: number }>;
}> {
	const visibleHeight = LAYOUT_CONSTANTS.tabStripHeight - LAYOUT_CONSTANTS.tabstripToolbarOverlap;
	const separatorVerticalMargins = LAYOUT_CONSTANTS.tabStripPadding * 2;
	const extraVerticalSpace = visibleHeight - TAB_SEPARATOR_HEIGHT - separatorVerticalMargins;
	const y = extraVerticalSpace / 2 + LAYOUT_CONSTANTS.tabStripPadding;
	const leadingX = TAB_BOTTOM_CORNER_RADIUS - TAB_SEPARATOR_HORIZONTAL_MARGIN - TAB_SEPARATOR_WIDTH;
	return {
		leading: { x: leadingX, y, width: TAB_SEPARATOR_WIDTH, height: TAB_SEPARATOR_HEIGHT },
		trailing: {
			x: width - TAB_BOTTOM_CORNER_RADIUS + TAB_SEPARATOR_HORIZONTAL_MARGIN,
			y,
			width: TAB_SEPARATOR_WIDTH,
			height: TAB_SEPARATOR_HEIGHT,
		},
	};
}

export interface TabSeparatorState {
	active: boolean;
	selected: boolean;
	hovered: boolean;
	group: string | null;
}

function hasVisibleTabBackground(tab: Readonly<TabSeparatorState> | null): boolean {
	return Boolean(tab && (tab.active || tab.selected || tab.hovered));
}

/**
 * Steady-state form of TabStyleViewsImpl::GetSeparatorOpacity. CSS animates
 * between these targets using the same 200ms hover duration as Chromium.
 */
export function getHorizontalTabSeparatorTargets(input: {
	tab: Readonly<TabSeparatorState>;
	previous: Readonly<TabSeparatorState> | null;
	next: Readonly<TabSeparatorState> | null;
	tabCount: number;
	declutterEnabled?: boolean;
	trailingControlHasOpaqueBackground?: boolean;
}): Readonly<{ leading: number; trailing: number }> {
	if (
		(input.declutterEnabled ?? false) &&
		input.tabCount >= TAB_STRIP_DECLUTTER_MIN_TABS_FOR_SEPARATOR_HIDE
	) {
		return { leading: 0, trailing: 0 };
	}
	if (hasVisibleTabBackground(input.tab)) return { leading: 0, trailing: 0 };

	// Adjacent tab bounds overlap, so rendering both sides of one boundary
	// produces two visibly separated rules. The preceding tab owns the shared
	// boundary through its trailing separator.
	const next = input.next;
	if (!next) {
		return {
			leading: 0,
			trailing: input.trailingControlHasOpaqueBackground ? 0 : 1,
		};
	}
	if (hasVisibleTabBackground(next)) return { leading: 0, trailing: 0 };
	return { leading: 0, trailing: 1 };
}
