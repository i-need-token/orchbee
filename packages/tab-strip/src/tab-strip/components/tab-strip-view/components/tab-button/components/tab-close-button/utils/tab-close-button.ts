export const TAB_CLOSE_ICON_SIZE = 16;
export const TAB_CLOSE_TOUCH_ICON_SIZE = 24;
export const TAB_CLOSE_BUTTON_PADDING = 12;
export const TAB_CLOSE_BUTTON_SIZE = TAB_CLOSE_ICON_SIZE + TAB_CLOSE_BUTTON_PADDING;
export const TAB_CLOSE_TOUCH_BUTTON_SIZE = TAB_CLOSE_TOUCH_ICON_SIZE + TAB_CLOSE_BUTTON_PADDING;
export const TAB_CLOSE_HOVER_OPACITY = 0.16;
export const TAB_CLOSE_PRESSED_OPACITY = 0.14;

export interface TabCloseButtonMetrics {
	iconSize: number;
	buttonSize: number;
	nonVisiblePadding: number;
}

/** Port of TabCloseButton::GetButtonSize plus kTabCloseButtonSize. */
export function getTabCloseButtonMetrics(touchUi = false): TabCloseButtonMetrics {
	const iconSize = touchUi ? TAB_CLOSE_TOUCH_ICON_SIZE : TAB_CLOSE_ICON_SIZE;
	return {
		iconSize,
		buttonSize: iconSize + TAB_CLOSE_BUTTON_PADDING,
		nonVisiblePadding: TAB_CLOSE_BUTTON_PADDING / 2,
	};
}

export interface TabCloseTargetInput {
	x: number;
	y: number;
	touchDown?: boolean;
	touchUi?: boolean;
}

/**
 * Mirrors TabCloseButton::TargetForRect(): mouse point targeting ignores the
 * 6dp padding around the icon, while touch can use the whole 28dp button.
 */
export function isPointInTabCloseTarget(input: Readonly<TabCloseTargetInput>): boolean {
	const metrics = getTabCloseButtonMetrics(input.touchUi);
	const inset = input.touchDown ? 0 : metrics.nonVisiblePadding;
	return (
		input.x >= inset &&
		input.x < metrics.buttonSize - inset &&
		input.y >= inset &&
		input.y < metrics.buttonSize - inset
	);
}

export function shouldTabCloseButtonHandlePointer(input: {
	button: number;
	pointerType?: string;
	x: number;
	y: number;
	touchUi?: boolean;
}): boolean {
	if (input.button === 1) return false;
	return isPointInTabCloseTarget({
		x: input.x,
		y: input.y,
		touchDown: input.pointerType === 'touch',
		...(input.touchUi === undefined ? {} : { touchUi: input.touchUi }),
	});
}
