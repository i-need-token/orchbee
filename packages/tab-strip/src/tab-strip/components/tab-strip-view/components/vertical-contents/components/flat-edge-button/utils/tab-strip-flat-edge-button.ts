import { TabStripOrientation } from '../../../../../../../types/tab-strip-types';

export const TAB_STRIP_FLAT_EDGE_BUTTON_SIZE = 28;
const TAB_STRIP_FLAT_EDGE_RADIUS = 2;
const TAB_STRIP_ROUNDED_EDGE_RADIUS = 10;

export const TabStripFlatEdge = {
	None: 'none',
	Top: 'top',
	Left: 'left',
	Bottom: 'bottom',
	Right: 'right',
} as const;

export type TabStripFlatEdge = (typeof TabStripFlatEdge)[keyof typeof TabStripFlatEdge];

interface CornerRadii {
	topLeft: number;
	topRight: number;
	bottomRight: number;
	bottomLeft: number;
}

interface FlatEdgeButtonSize {
	width: number;
	height: number;
}

/** Port of TabStripFlatEdgeButton::GetButtonCornerRadii. */
export function getFlatEdgeButtonCornerRadii(
	flatEdge: TabStripFlatEdge,
	flatEdgeFactor = 1,
): CornerRadii {
	const rounded = TAB_STRIP_ROUNDED_EDGE_RADIUS;
	const flat =
		TAB_STRIP_FLAT_EDGE_RADIUS + (rounded - TAB_STRIP_FLAT_EDGE_RADIUS) * (1 - flatEdgeFactor);
	switch (flatEdge) {
		case TabStripFlatEdge.Top:
			return { topLeft: flat, topRight: flat, bottomRight: rounded, bottomLeft: rounded };
		case TabStripFlatEdge.Left:
			return { topLeft: flat, topRight: rounded, bottomRight: rounded, bottomLeft: flat };
		case TabStripFlatEdge.Bottom:
			return { topLeft: rounded, topRight: rounded, bottomRight: flat, bottomLeft: flat };
		case TabStripFlatEdge.Right:
			return { topLeft: rounded, topRight: flat, bottomRight: flat, bottomLeft: rounded };
		case TabStripFlatEdge.None:
			return { topLeft: rounded, topRight: rounded, bottomRight: rounded, bottomLeft: rounded };
	}
}

/** Port of TabStripFlatEdgeButton::CalculatePreferredSize. */
export function getFlatEdgeButtonPreferredSize(
	expansionFactor: number,
	orientation: TabStripOrientation,
	rawButtonSize = TAB_STRIP_FLAT_EDGE_BUTTON_SIZE,
): FlatEdgeButtonSize {
	return orientation === TabStripOrientation.Horizontal
		? { width: Math.trunc(rawButtonSize * expansionFactor), height: rawButtonSize }
		: { width: rawButtonSize, height: Math.trunc(rawButtonSize * expansionFactor) };
}

/** Port of TabStripComboButton::UpdateStyles. */
export function getComboButtonFlatEdges(
	orientation: TabStripOrientation,
	bothVisible: boolean,
	rightToLeft = false,
): Readonly<{ start: TabStripFlatEdge; end: TabStripFlatEdge }> {
	if (!bothVisible) return { start: TabStripFlatEdge.None, end: TabStripFlatEdge.None };
	if (orientation === TabStripOrientation.Vertical) {
		return { start: TabStripFlatEdge.Bottom, end: TabStripFlatEdge.Top };
	}
	return rightToLeft
		? { start: TabStripFlatEdge.Left, end: TabStripFlatEdge.Right }
		: { start: TabStripFlatEdge.Right, end: TabStripFlatEdge.Left };
}
