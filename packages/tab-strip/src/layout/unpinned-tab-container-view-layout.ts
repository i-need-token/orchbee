import { TabStripOrientation, VerticalTabStripCollapseState } from '../common';
import { LAYOUT_CONSTANTS } from '../style';
import type { CollectionChildLayoutData } from './collection-layout-types';
import type { ProposedLayout, Size, SizeBounds } from './proposed-layout';

const TAB_VERTICAL_PADDING = 2;

export interface UnpinnedTabContainerLayoutInput {
	orientation: TabStripOrientation;
	sizeBounds: SizeBounds;
	children: readonly CollectionChildLayoutData[];
	collapseState?: VerticalTabStripCollapseState;
	handlingDrag?: boolean;
	draggingViewsBottom?: number;
}

/** Port of common/unpinned_tab_container_view_layout.cc. */
export function calculateUnpinnedTabContainerLayout(
	input: UnpinnedTabContainerLayoutInput,
): ProposedLayout {
	return input.orientation === TabStripOrientation.Horizontal
		? calculateHorizontal(input)
		: calculateVertical(input);
}

function calculateHorizontal(input: UnpinnedTabContainerLayoutInput): ProposedLayout {
	const containerHeight = input.sizeBounds.height ?? LAYOUT_CONSTANTS.tabHeight;
	let x = 0;
	const children = input.children.map((child) => {
		const bounds = {
			x: child.drag?.x ?? x,
			y: 0,
			width: child.preferredSize.width,
			height: containerHeight,
		};
		x += bounds.width;
		return { id: child.id, visible: child.visible ?? true, bounds };
	});
	return { hostSize: { width: x, height: containerHeight }, children };
}

function calculateVertical(input: UnpinnedTabContainerLayoutInput): ProposedLayout {
	const collapseState = input.collapseState ?? VerticalTabStripCollapseState.Expanded;
	const horizontalPadding = LAYOUT_CONSTANTS.verticalTabStripHorizontalPadding;
	let width = 0;
	let height = 0;
	const children = input.children.map((child) => {
		const x =
			child.group && collapseState !== VerticalTabStripCollapseState.Expanded
				? 0
				: horizontalPadding;
		const childWidth =
			input.sizeBounds.width === null
				? child.preferredSize.width
				: input.sizeBounds.width - x - horizontalPadding;
		const bounds = {
			x,
			y: child.drag?.y ?? height,
			width: childWidth,
			height: child.preferredSize.height,
		};
		height += bounds.height + TAB_VERTICAL_PADDING;
		width = Math.max(width, bounds.width + bounds.x);
		return { id: child.id, visible: child.visible ?? true, bounds };
	});
	if (input.children.length > 0) {
		height -= TAB_VERTICAL_PADDING;
	}

	let draggedViewBottom = 0;
	if (input.handlingDrag) {
		draggedViewBottom = input.draggingViewsBottom ?? 0;
		if (input.sizeBounds.height !== null) {
			draggedViewBottom = Math.min(draggedViewBottom, input.sizeBounds.height);
		}
	}
	return { hostSize: { width, height: Math.max(height, draggedViewBottom) }, children };
}

export function calculateUnpinnedTabContainerMinimumSize(
	orientation: TabStripOrientation,
	children: readonly CollectionChildLayoutData[],
): Size {
	if (orientation === TabStripOrientation.Horizontal) {
		return {
			width: children.reduce((sum, child) => sum + (child.minimumSize?.width ?? 0), 0),
			height: LAYOUT_CONSTANTS.tabHeight,
		};
	}
	const childCount = children.length;
	return {
		width: childCount === 0 ? 0 : LAYOUT_CONSTANTS.verticalTabMinWidth,
		height:
			Math.ceil(LAYOUT_CONSTANTS.verticalTabHeight * Math.min(1.5, childCount)) +
			(childCount > 1 ? TAB_VERTICAL_PADDING : 0),
	};
}
