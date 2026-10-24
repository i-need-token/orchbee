import type { Rect } from '../utils/rectRight';
import { LayoutDomain } from './tab-strip-layout-types';

export const TAB_CLOSE_TOUCH_RESIZE_DELAY_MS = 2_000;
const TAB_CLOSE_WATCH_VERTICAL_SLOP = 40;
const TAB_CLOSE_WATCH_TRAILING_SLOP = 60;

export type TabCloseSource = 'mouse' | 'touch' | 'non-ui';

interface TabClosingWidthInput {
	currentOverride: number | null;
	containerWidth: number;
	modelCountAfterRemoval: number;
	removedModelIndex: number;
	removedWidth: number;
	removedWasActive: boolean;
	removedWasPinned: boolean;
	layoutDomain: LayoutDomain;
	nextActiveWidth?: number;
	nextActivePinned?: boolean;
	trailingIdealRight: number;
	tabOverlap: number;
}

/**
 * Port of TabContainerImpl::EnterTabClosingMode and
 * UpdateClosingModeOnRemovedTab's width arithmetic.
 */
export function calculateTabClosingModeWidth(input: Readonly<TabClosingWidthInput>): number | null {
	if (input.modelCountAfterRemoval <= 0) return null;

	const initialOverride = input.currentOverride ?? input.containerWidth;
	if (input.removedModelIndex === input.modelCountAfterRemoval) {
		return initialOverride;
	}

	let sizeDelta = input.removedWidth;
	if (
		input.removedWasActive &&
		!input.removedWasPinned &&
		input.layoutDomain === LayoutDomain.InactiveWidthBelowActiveWidth &&
		input.nextActiveWidth !== undefined &&
		input.nextActivePinned !== true
	) {
		sizeDelta = input.nextActiveWidth;
	}

	return Math.max(0, input.trailingIdealRight - sizeDelta + input.tabOverlap);
}

/** Region watched by Chromium while mouse-based closing mode is active. */
export function getTabClosingModeWatchBounds(host: Readonly<Rect>, rtl = false): Rect {
	return {
		x: host.x - (rtl ? TAB_CLOSE_WATCH_TRAILING_SLOP : 0),
		y: host.y,
		width: host.width + TAB_CLOSE_WATCH_TRAILING_SLOP,
		height: host.height + TAB_CLOSE_WATCH_VERTICAL_SLOP,
	};
}

export function tabClosingModeContainsPoint(
	bounds: Readonly<Rect>,
	point: Readonly<{ x: number; y: number }>,
): boolean {
	return (
		point.x >= bounds.x &&
		point.x < bounds.x + bounds.width &&
		point.y >= bounds.y &&
		point.y < bounds.y + bounds.height
	);
}

/**
 * Chromium drops the width override as soon as it is wider than the ideal
 * trailing edge: at that point the override is no longer constraining tabs.
 */
export function shouldExitTabClosingMode(
	overrideWidth: number,
	trailingIdealRight: number,
): boolean {
	return overrideWidth > trailingIdealRight;
}
