export const TAB_STRIP_NUDGE_CORNER_RADIUS = 10;
export const TAB_STRIP_NUDGE_FLAT_CORNER_RADIUS = 4;
export const TAB_STRIP_NUDGE_ICON_MARGIN = 6;
export const TAB_STRIP_NUDGE_LABEL_LEFT_MARGIN = 4;
export const TAB_STRIP_NUDGE_LABEL_RIGHT_MARGIN = 12;
export const TAB_STRIP_NUDGE_CLOSE_BUTTON_MARGIN = 8;
export const TAB_STRIP_NUDGE_CLOSE_BUTTON_SIZE = 16;

export interface TabStripNudgeButtonLayoutInput {
	iconWidth: number;
	labelWidth: number;
	showCloseButton: boolean;
	widthFactor: number;
	controlHeight?: number;
}

export interface TabStripNudgeButtonLayout {
	fullWidth: number;
	width: number;
	height: number;
	labelInsets: Readonly<{ left: number; right: number }>;
	closeButtonContribution: number;
}

const clamp01 = (value: number) => Math.min(1, Math.max(0, value));

/** Ports TabStripNudgeButton's BoxLayout preferred-size arithmetic. */
export function calculateTabStripNudgeButtonLayout(
	input: Readonly<TabStripNudgeButtonLayoutInput>,
): TabStripNudgeButtonLayout {
	const labelInsets = {
		left: TAB_STRIP_NUDGE_LABEL_LEFT_MARGIN,
		right: input.showCloseButton ? 0 : TAB_STRIP_NUDGE_LABEL_RIGHT_MARGIN,
	};
	const iconContribution = input.iconWidth > 0 ? TAB_STRIP_NUDGE_ICON_MARGIN + input.iconWidth : 0;
	const closeButtonContribution = input.showCloseButton
		? TAB_STRIP_NUDGE_CLOSE_BUTTON_SIZE + 2 * TAB_STRIP_NUDGE_CLOSE_BUTTON_MARGIN
		: 0;
	const fullWidth =
		iconContribution +
		labelInsets.left +
		Math.max(0, input.labelWidth) +
		labelInsets.right +
		closeButtonContribution;
	return {
		fullWidth,
		width: Math.trunc(fullWidth * clamp01(input.widthFactor)),
		height: input.controlHeight ?? 28,
		labelInsets,
		closeButtonContribution,
	};
}
