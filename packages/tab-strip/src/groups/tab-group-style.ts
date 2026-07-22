import { LAYOUT_CONSTANTS, DEFAULT_TAB_STYLE_METRICS } from '../style';
export const TAB_GROUP_HEADER_CHIP_VERTICAL_INSET = 2;
export const TAB_GROUP_HEADER_TITLE_ADJUSTMENT = -2;
export const TAB_GROUP_HEADER_SYNC_ICON_WIDTH = 16;
export const TAB_GROUP_HEADER_ATTENTION_WIDTH = 8;
export const TAB_GROUP_HEADER_EMPTY_CHIP_SIZE = 20;
export const TAB_GROUP_HEADER_CORNER_RADIUS = 6;
export const TAB_GROUP_OVERLAP_ADJUSTMENT = 2;
export const TAB_GROUP_SYNC_ICON_PADDING = 2;
export const TAB_GROUP_UNDERLINE_STROKE_THICKNESS = 2;
const EMOJI_PRESENTATION_OR_REGIONAL_INDICATOR = /[\p{Emoji_Presentation}\p{Regional_Indicator}]/u;
function isSingleGrapheme(text: string): boolean {
	if (!text) return false;
	const Segmenter = Intl.Segmenter;
	const segments = new Segmenter(undefined, {
		granularity: 'grapheme',
	}).segment(text);
	const iterator = segments[Symbol.iterator]();
	if (iterator.next().done) return false;
	return Boolean(iterator.next().done);
}

/**
 * Port of TabGroupHeader::GetTitleVisualCenteringOffset. Apple Color Emoji's
 * asymmetric side bearings need a one-pixel compensation for one grapheme.
 */
export function getTabGroupTitleVisualCenteringOffset(
	title: string,
	options: Readonly<{
		isMac: boolean;
		rtl?: boolean;
	}>,
): number {
	if (
		!options.isMac ||
		!isSingleGrapheme(title) ||
		!EMOJI_PRESENTATION_OR_REGIONAL_INDICATOR.test(title)
	)
		return 0;
	return options.rtl ? -1 : 1;
}
export interface HorizontalTabGroupHeaderLayoutInput {
	title: string;
	measuredTitleWidth: number;
	shared?: boolean;
	attention?: boolean;
	collapsed?: boolean;
	tabCount?: number;
	standardTabWidth?: number;
	tabOverlap?: number;
}
export interface HorizontalTabGroupHeaderLayout {
	desiredWidth: number;
	chipWidth: number;
	chipHeight: number;
	chipX: number;
	chipY: number;
	textWidth: number;
	textMaxWidth: number;
	showHeaderIcon: boolean;
	showAttentionIndicator: boolean;
	horizontalInset: number;
}

/** Direct arithmetic port of TabGroupStyle and TabGroupHeader's chip layout. */
export function calculateHorizontalTabGroupHeaderLayout(
	input: HorizontalTabGroupHeaderLayoutInput,
): HorizontalTabGroupHeaderLayout {
	const overlap = input.tabOverlap ?? DEFAULT_TAB_STYLE_METRICS.tabOverlap;
	const standardWidth = input.standardTabWidth ?? 256;
	const showHeaderIcon = input.shared ?? false;
	const showAttentionIndicator =
		showHeaderIcon &&
		(input.collapsed ?? false) &&
		(input.attention ?? false) &&
		(input.tabCount ?? 0) > 0;
	const horizontalInset = TAB_GROUP_HEADER_CORNER_RADIUS;
	const chipX = overlap - TAB_GROUP_OVERLAP_ADJUSTMENT;
	const totalSpace =
		LAYOUT_CONSTANTS.tabStripHeight -
		TAB_GROUP_HEADER_EMPTY_CHIP_SIZE -
		LAYOUT_CONSTANTS.tabstripToolbarOverlap;
	const chipY = Math.trunc(totalSpace / 2);
	const iconWidth = showHeaderIcon ? TAB_GROUP_HEADER_SYNC_ICON_WIDTH : 0;
	const iconPadding = showHeaderIcon ? TAB_GROUP_SYNC_ICON_PADDING : 0;
	const textMaxWidth = Math.trunc((standardWidth - overlap) / 2) - iconWidth - iconPadding;
	const textWidth = Math.max(0, Math.min(input.measuredTitleWidth, textMaxWidth));
	let chipWidth: number;
	if (!input.title) {
		chipWidth = showAttentionIndicator
			? TAB_GROUP_HEADER_SYNC_ICON_WIDTH +
				TAB_GROUP_HEADER_ATTENTION_WIDTH +
				TAB_GROUP_SYNC_ICON_PADDING +
				horizontalInset * 2
			: TAB_GROUP_HEADER_EMPTY_CHIP_SIZE;
	} else {
		const attentionWidth = showAttentionIndicator
			? TAB_GROUP_HEADER_ATTENTION_WIDTH + TAB_GROUP_SYNC_ICON_PADDING * 2
			: 0;
		const contentWidth = iconWidth + iconPadding + textWidth + attentionWidth;
		chipWidth = Math.max(TAB_GROUP_HEADER_EMPTY_CHIP_SIZE, contentWidth + horizontalInset * 2);
	}
	const groupViewOverlap = overlap - TAB_GROUP_OVERLAP_ADJUSTMENT;
	return {
		desiredWidth: groupViewOverlap * 2 + chipWidth,
		chipWidth,
		chipHeight: TAB_GROUP_HEADER_EMPTY_CHIP_SIZE,
		chipX,
		chipY,
		textWidth,
		textMaxWidth,
		showHeaderIcon,
		showAttentionIndicator,
		horizontalInset,
	};
}
export function shouldHideTabGroupUnderline(
	leadingKind: 'header' | 'tab' | null,
	trailingKind: 'header' | 'tab' | null,
): boolean {
	return leadingKind === 'header' && trailingKind === 'header';
}
export interface HorizontalTabGroupUnderlineInput {
	leadingHeaderX: number;
	trailingTabX: number;
	trailingTabWidth: number;
	trailingTabActive: boolean;
	tabOverlap?: number;
}

/** Horizontal projection of TabGroupUnderline::CalculateTabGroupUnderlineBounds. */
export function calculateHorizontalTabGroupUnderlineBounds(
	input: HorizontalTabGroupUnderlineInput,
): Readonly<{
	x: number;
	width: number;
}> {
	const overlap = input.tabOverlap ?? DEFAULT_TAB_STYLE_METRICS.tabOverlap;
	const inactiveInset =
		overlap - TAB_GROUP_OVERLAP_ADJUSTMENT + TAB_GROUP_UNDERLINE_STROKE_THICKNESS;
	const x = input.leadingHeaderX + inactiveInset;
	const trailingRight =
		input.trailingTabX +
		input.trailingTabWidth +
		(input.trailingTabActive ? TAB_GROUP_UNDERLINE_STROKE_THICKNESS : -inactiveInset);
	return {
		x,
		width: Math.max(0, trailingRight - x),
	};
}
export interface MergeDraggedGroupUnderlineInput {
	normal: Readonly<{
		x: number;
		width: number;
	}>;
	dragged: Readonly<{
		x: number;
		width: number;
	}>;
	leadingDraggedView: Readonly<{
		x: number;
		width: number;
	}>;
	trailingDraggedView: Readonly<{
		x: number;
		width: number;
	}>;
}

/** Port of TabGroupViews::UpdateBounds' normal/drag underline merge. */
export function mergeDraggedGroupUnderlineBounds(input: MergeDraggedGroupUnderlineInput): Readonly<{
	x: number;
	width: number;
}> {
	const draggedRight = input.dragged.x + input.dragged.width;
	const leadingX = Math.min(
		input.dragged.x,
		Math.max(Math.min(input.dragged.x, input.leadingDraggedView.x), input.normal.x),
	);
	const trailingX = Math.min(
		Math.max(draggedRight, input.trailingDraggedView.x + input.trailingDraggedView.width),
		Math.max(draggedRight, input.normal.x + input.normal.width),
	);
	return {
		x: leadingX,
		width: Math.max(0, trailingX - leadingX),
	};
}

/** SVG equivalent of TabGroupHighlight::GetPath's tab-shaped SkPath. */
export function getTabGroupHighlightPath(
	width: number,
	height: number,
	top = LAYOUT_CONSTANTS.tabStripPadding,
	cornerRadius = 12,
): string {
	const w = Math.max(0, width);
	const h = Math.max(0, height);
	const r = Math.max(0, Math.min(cornerRadius, w / 4, (h - top) / 2));
	return [
		`M 0 ${h}`,
		`A ${r} ${r} 0 0 0 ${r} ${h - r}`,
		`L ${r} ${top + r}`,
		`A ${r} ${r} 0 0 1 ${2 * r} ${top}`,
		`L ${w - 2 * r} ${top}`,
		`A ${r} ${r} 0 0 1 ${w - r} ${top + r}`,
		`L ${w - r} ${h - r}`,
		`A ${r} ${r} 0 0 0 ${w} ${h}`,
		'Z',
	].join(' ');
}
