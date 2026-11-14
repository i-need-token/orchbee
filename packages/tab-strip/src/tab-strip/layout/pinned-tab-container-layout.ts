import { VerticalTabStripCollapseState } from '../types/tab-strip-types';
import type { DraggedTabsAxes } from '../dragging/dragged-tabs-container-layout';
import { LAYOUT_CONSTANTS } from './layout-constants';
import type { ProposedLayout, Size, SizeBounds } from './proposed-layout';

const TAB_PADDING = 4;

/** Port of PinnedTabContainerView::OnCollapseStateChanged. */
export function getPinnedTabContainerDragAxes(
	collapseState: VerticalTabStripCollapseState,
): DraggedTabsAxes {
	return collapseState === VerticalTabStripCollapseState.Collapsed ? 'vertical-only' : 'both';
}

interface PinnedChildLayoutData {
	id: string;
	preferredSize: Size;
	drag?: { x: number; y: number; shouldHide?: boolean; shouldFloat?: boolean };
}

export interface PinnedTabContainerLayoutInput {
	sizeBounds: SizeBounds;
	children: readonly PinnedChildLayoutData[];
	collapseState: VerticalTabStripCollapseState;
	rtl?: boolean;
	/** The narrowest tile; touch tiles keep a 44 px target. */
	childMinimumWidth?: number;
}

/** Port of PinnedTabContainerView::CalculateVerticalLayout. */
export function calculatePinnedTabContainerLayout(
	input: PinnedTabContainerLayoutInput,
): ProposedLayout {
	if (input.children.length === 0) {
		return { hostSize: { width: 0, height: 0 }, children: [] };
	}

	let x = 0;
	let y = 0;
	let totalWidth = 0;
	let totalHeight = 0;
	let childrenOnRow = input.children.length;
	let childWidth = input.childMinimumWidth ?? LAYOUT_CONSTANTS.verticalTabMinWidth;

	if (input.sizeBounds.width !== null && input.sizeBounds.width > 0) {
		let availableWidth =
			input.sizeBounds.width - LAYOUT_CONSTANTS.verticalTabStripHorizontalPadding;
		childrenOnRow = Math.min(
			childrenOnRow,
			Math.floor((availableWidth - childWidth) / (childWidth + TAB_PADDING)) + 1,
		);
		childrenOnRow = Math.max(childrenOnRow, 1);
		availableWidth -= childrenOnRow * childWidth + TAB_PADDING * (childrenOnRow - 1);
		childWidth += Math.floor(availableWidth / childrenOnRow);
	}

	let rowIndex = 0;
	const children = input.children.map((child) => {
		const bounds = { x: 0, y: 0, width: childWidth, height: child.preferredSize.height };
		if (child.drag?.shouldHide) {
			return { id: child.id, visible: false, bounds };
		}

		bounds.y = child.drag?.y ?? y;
		let childX = child.drag?.x ?? x;
		if (child.drag && input.rtl && input.sizeBounds.width !== null) {
			childX = input.sizeBounds.width - childX - childWidth;
		}
		bounds.x = childX + (rowIndex === 0 ? 0 : TAB_PADDING);

		if (!child.drag?.shouldFloat) {
			if (rowIndex !== 0) {
				x += TAB_PADDING;
			}
			x += childWidth;
			totalWidth = Math.max(totalWidth, x);
			totalHeight = Math.max(totalHeight, y + bounds.height);
			rowIndex += 1;
			if (rowIndex >= childrenOnRow) {
				y = totalHeight + TAB_PADDING;
				rowIndex = 0;
				x = 0;
			}
		}

		return { id: child.id, visible: true, bounds };
	});

	return {
		hostSize: { width: input.sizeBounds.width ?? totalWidth, height: totalHeight },
		children,
	};
}

export function calculatePinnedTabContainerMinimumSize(
	childCount: number,
	collapseState: VerticalTabStripCollapseState,
): Size {
	const minimumRows = Math.min(
		collapseState === VerticalTabStripCollapseState.Expanded ? 1 : 1.5,
		childCount,
	);
	return {
		width: LAYOUT_CONSTANTS.verticalTabMinWidth,
		height:
			Math.ceil(LAYOUT_CONSTANTS.verticalTabHeight * minimumRows) +
			(childCount > 1 ? TAB_PADDING : 0),
	};
}
