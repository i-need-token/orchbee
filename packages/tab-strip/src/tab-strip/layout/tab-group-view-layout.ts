import { TabStripOrientation, VerticalTabStripCollapseState } from '../types/tab-strip-types';
import { LAYOUT_CONSTANTS } from './layout-constants';
import type { CollectionChildLayoutData } from './collection-layout-types';
import type { ChildLayout, ProposedLayout, Size, SizeBounds } from './proposed-layout';

const TAB_VERTICAL_PADDING = 2;
const GROUP_LINE_WIDTH = 2;
const GROUP_LINE_COLLAPSED_LEADING_PADDING = 6;
const GROUP_HEADER_HEIGHT = 26;
const GROUP_HEADER_VERTICAL_MARGIN = 4;
export const VERTICAL_TAB_GROUP_COLLAPSED_HEIGHT =
	GROUP_HEADER_HEIGHT + 2 * GROUP_HEADER_VERTICAL_MARGIN;
const GROUP_TAB_LEADING_PADDING = 10;

export interface TabGroupViewLayoutInput {
	orientation: TabStripOrientation;
	sizeBounds: SizeBounds;
	headerId: string;
	headerPreferredSize: Size;
	headerVisible?: boolean;
	lineId?: string;
	lineVisible?: boolean;
	children: readonly CollectionChildLayoutData[];
	collapseState?: VerticalTabStripCollapseState;
	groupCollapsed?: boolean;
}

/** Port of common/tab_group_view_layout.cc. */
export function calculateTabGroupViewLayout(input: TabGroupViewLayoutInput): ProposedLayout {
	return input.orientation === TabStripOrientation.Horizontal
		? calculateHorizontal(input)
		: calculateVertical(input);
}

function calculateHorizontal(input: TabGroupViewLayoutInput): ProposedLayout {
	const containerHeight = input.sizeBounds.height ?? LAYOUT_CONSTANTS.tabHeight;
	const headerWidth = input.headerPreferredSize.width;
	const children: ChildLayout[] = [
		{
			id: input.headerId,
			visible: input.headerVisible ?? true,
			bounds: { x: 0, y: 0, width: headerWidth, height: containerHeight },
		},
		{
			id: input.lineId ?? `${input.headerId}-line`,
			visible: false,
			bounds: { x: 0, y: 0, width: 0, height: 0 },
		},
	];

	let x = headerWidth;
	for (const child of input.children) {
		const visible = child.visible ?? true;
		const bounds = {
			x: child.drag?.x ?? x,
			y: 0,
			width: child.preferredSize.width,
			height: containerHeight,
		};
		children.push({ id: child.id, visible, bounds });
		if (visible) {
			x += bounds.width;
		}
	}
	return {
		hostSize: { width: input.groupCollapsed ? headerWidth : x, height: containerHeight },
		children,
	};
}

function calculateVertical(input: TabGroupViewLayoutInput): ProposedLayout {
	const collapseState = input.collapseState ?? VerticalTabStripCollapseState.Expanded;
	let width = 0;
	let height = GROUP_HEADER_VERTICAL_MARGIN;
	const collapsedStrip = collapseState !== VerticalTabStripCollapseState.Expanded;

	const headerBounds = {
		x: collapsedStrip ? LAYOUT_CONSTANTS.verticalTabStripHorizontalPadding : 0,
		y: height,
		width: 0,
		height: GROUP_HEADER_HEIGHT,
	};
	if (input.sizeBounds.width !== null) {
		headerBounds.width = input.sizeBounds.width - headerBounds.x;
	}
	const children: ChildLayout[] = [
		{
			id: input.headerId,
			visible: input.headerVisible ?? true,
			bounds: headerBounds,
		},
	];
	height += GROUP_HEADER_HEIGHT + GROUP_HEADER_VERTICAL_MARGIN + TAB_VERTICAL_PADDING;
	width = Math.max(width, headerBounds.width);

	const groupLineBounds = {
		x: collapsedStrip
			? GROUP_LINE_COLLAPSED_LEADING_PADDING
			: (GROUP_TAB_LEADING_PADDING - GROUP_LINE_WIDTH) / 2,
		y: collapsedStrip ? GROUP_HEADER_VERTICAL_MARGIN : height,
		width: GROUP_LINE_WIDTH,
		height: 0,
	};

	for (const child of input.children) {
		const x = collapsedStrip
			? LAYOUT_CONSTANTS.verticalTabStripHorizontalPadding
			: GROUP_TAB_LEADING_PADDING;
		const bounds = {
			x,
			y: child.drag?.y ?? height,
			width:
				input.sizeBounds.width === null ? child.preferredSize.width : input.sizeBounds.width - x,
			height: child.preferredSize.height,
		};
		children.push({ id: child.id, visible: child.visible ?? true, bounds });
		height += bounds.height + TAB_VERTICAL_PADDING;
		width = Math.max(width, bounds.width);
	}
	height -= TAB_VERTICAL_PADDING;
	if (input.children.length > 0) {
		groupLineBounds.height = height - groupLineBounds.y;
	}
	children.push({
		id: input.lineId ?? `${input.headerId}-line`,
		visible: input.lineVisible ?? true,
		bounds: groupLineBounds,
	});

	if (!input.groupCollapsed) {
		height += TAB_VERTICAL_PADDING;
	}
	return {
		hostSize: {
			width,
			height: input.groupCollapsed ? VERTICAL_TAB_GROUP_COLLAPSED_HEIGHT : height,
		},
		children,
	};
}

export function calculateTabGroupViewMinimumSize(input: TabGroupViewLayoutInput): Size {
	if (input.orientation === TabStripOrientation.Vertical) {
		return calculateVertical({ ...input, sizeBounds: { width: 0, height: 0 } }).hostSize;
	}
	let width = input.headerPreferredSize.width;
	if (!input.groupCollapsed) {
		width += input.children.reduce((sum, child) => sum + (child.minimumSize?.width ?? 0), 0);
	}
	return { width, height: LAYOUT_CONSTANTS.tabHeight };
}
