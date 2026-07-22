export const TAB_STRIP_CONTROL_BUTTON_SIZE = 28;
export const TAB_STRIP_CONTROL_BUTTON_ICON_SIZE = 16;
export const TAB_STRIP_CONTROL_BUTTON_CORNER_RADIUS = 14;
export const TAB_STRIP_CONTROL_BUTTON_FLAT_CORNER_RADIUS = 0;

export const TabStripControlButtonEdge = {
	None: 'none',
	Left: 'left',
	Right: 'right',
} as const;

export type TabStripControlButtonEdge =
	(typeof TabStripControlButtonEdge)[keyof typeof TabStripControlButtonEdge];

export interface TabStripControlButtonRadii {
	topLeft: number;
	topRight: number;
	bottomRight: number;
	bottomLeft: number;
}

export interface TabStripControlButtonInsets {
	top: number;
	right: number;
	bottom: number;
	left: number;
}

export interface TabStripControlButtonShapeInput {
	fixedFlatEdge?: TabStripControlButtonEdge;
	animatedFlatEdge?: TabStripControlButtonEdge;
	flatEdgeFactor?: number;
	leftCornerRadius?: number;
	rightCornerRadius?: number;
	flatCornerRadius?: number;
	frameCondensed?: boolean;
}

const clamp01 = (value: number) => Math.min(1, Math.max(0, value));

/** Direct port of TabStripControlButton::GetScaledCornerRadius. */
export function getScaledTabStripControlButtonRadius(
	initialRadius: number,
	edge: TabStripControlButtonEdge,
	input: TabStripControlButtonShapeInput = {},
): number {
	const flatRadius = input.flatCornerRadius ?? TAB_STRIP_CONTROL_BUTTON_FLAT_CORNER_RADIUS;
	if ((input.fixedFlatEdge ?? TabStripControlButtonEdge.None) === edge) return flatRadius;
	if ((input.animatedFlatEdge ?? TabStripControlButtonEdge.None) === edge) {
		return (initialRadius - flatRadius) * clamp01(input.flatEdgeFactor ?? 1) + flatRadius;
	}
	return initialRadius;
}

/** Radii used by both the painted background and Chromium's hit-test mask. */
export function getTabStripControlButtonRadii(
	input: TabStripControlButtonShapeInput = {},
): TabStripControlButtonRadii {
	const left = input.leftCornerRadius ?? TAB_STRIP_CONTROL_BUTTON_CORNER_RADIUS;
	const right = input.rightCornerRadius ?? TAB_STRIP_CONTROL_BUTTON_CORNER_RADIUS;
	const topLeft = input.frameCondensed ? 0 : left;
	const topRight = input.frameCondensed ? 0 : right;
	return {
		topLeft: getScaledTabStripControlButtonRadius(topLeft, TabStripControlButtonEdge.Left, input),
		topRight: getScaledTabStripControlButtonRadius(
			topRight,
			TabStripControlButtonEdge.Right,
			input,
		),
		bottomRight: getScaledTabStripControlButtonRadius(
			right,
			TabStripControlButtonEdge.Right,
			input,
		),
		bottomLeft: getScaledTabStripControlButtonRadius(left, TabStripControlButtonEdge.Left, input),
	};
}

/** Port of CalculatePreferredSize: the fixed content size plus border insets. */
export function getTabStripControlButtonPreferredSize(
	insets: Partial<TabStripControlButtonInsets> = {},
): Readonly<{ width: number; height: number }> {
	return {
		width: TAB_STRIP_CONTROL_BUTTON_SIZE + (insets.left ?? 0) + (insets.right ?? 0),
		height: TAB_STRIP_CONTROL_BUTTON_SIZE + (insets.top ?? 0) + (insets.bottom ?? 0),
	};
}
