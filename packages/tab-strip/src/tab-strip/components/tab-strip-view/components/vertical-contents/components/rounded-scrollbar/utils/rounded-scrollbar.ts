const ROUNDED_SCROLLBAR_THUMB_THICKNESS = 5;
const ROUNDED_SCROLLBAR_TRAILING_PADDING = 4;
export const ROUNDED_SCROLLBAR_FADE_DELAY_MS = 500;

export interface RoundedScrollbarThumbGeometry {
	visible: boolean;
	top: number;
	height: number;
	maximumScroll: number;
}

export function roundedScrollbarThickness(shouldHaveRightMargin = true): number {
	return (
		ROUNDED_SCROLLBAR_THUMB_THICKNESS +
		(shouldHaveRightMargin ? ROUNDED_SCROLLBAR_TRAILING_PADDING : 0)
	);
}

/** Browser-independent overlay thumb geometry used by RoundedScrollBar. */
export function calculateRoundedScrollbarThumb(input: {
	viewportHeight: number;
	contentHeight: number;
	scrollTop: number;
	minimumThumbHeight?: number;
}): RoundedScrollbarThumbGeometry {
	const viewportHeight = Math.max(0, input.viewportHeight);
	const contentHeight = Math.max(0, input.contentHeight);
	const maximumScroll = Math.max(0, contentHeight - viewportHeight);
	if (viewportHeight === 0 || maximumScroll === 0) {
		return { visible: false, top: 0, height: viewportHeight, maximumScroll };
	}
	const minimum = Math.min(viewportHeight, input.minimumThumbHeight ?? 20);
	const height = Math.max(minimum, Math.round((viewportHeight * viewportHeight) / contentHeight));
	const clampedScrollTop = Math.min(maximumScroll, Math.max(0, input.scrollTop));
	const top = Math.round((clampedScrollTop / maximumScroll) * (viewportHeight - height));
	return { visible: true, top, height, maximumScroll };
}
