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
