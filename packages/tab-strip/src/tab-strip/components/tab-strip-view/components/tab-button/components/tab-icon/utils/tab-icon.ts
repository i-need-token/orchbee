import type { TabData } from '../../../../../../../model/tab-model';

export const TAB_ICON_SIZE = 16;
export const TAB_ICON_ATTENTION_RADIUS = 3;
export const TAB_ICON_LOADING_STROKE_WIDTH = 2;
export const TAB_ICON_FAVICON_TRANSITION_MS = 250;
export const TAB_ICON_DISCARD_FADE_MS = 1_000;
export const TAB_ICON_CRASH_ANIMATION_MS = 1_000;
export const TAB_ICON_CRASH_FRAMES_PER_SECOND = 25;
export const TAB_ICON_THROBBER_ROTATION_MS = 1_568;
export const TAB_ICON_INCREASED_DISCARD_RADIUS = 2;

export type TabIconNetworkState = NonNullable<TabData['networkState']>;

export interface TabIconVisualState {
	networkState: TabIconNetworkState;
	showingLoadingAnimation: boolean;
	showingFavicon: boolean;
	showingCrashedFavicon: boolean;
	showingAttentionIndicator: boolean;
	showingDiscardIndicator: boolean;
	hasCustomFavicon: boolean;
	increasedDiscardIndicatorRadius: number;
}

export interface TabIconCrashFrame {
	showingCrashedFavicon: boolean;
	hidingFraction: number;
}

const clamp01 = (value: number) => Math.min(1, Math.max(0, value));

/** Mirrors TabIcon's definition: waiting/loading animate, none/error do not. */
export function tabIconNetworkStateIsAnimated(networkState: TabIconNetworkState): boolean {
	return networkState !== 'none' && networkState !== 'error';
}

export function resolveTabIconNetworkState(tab: Readonly<TabData>): TabIconNetworkState {
	return tab.networkState ?? (tab.loading ? 'loading' : 'none');
}

/**
 * Ports ResizeDiscardIndicatorRadiusForWidth(). The ring grows by 2dp only
 * when a 20dp diameter still fits around Chromium's 16dp favicon.
 */
export function getIncreasedDiscardIndicatorRadius(width: number): number {
	return width >= TAB_ICON_SIZE + 2 * TAB_ICON_INCREASED_DISCARD_RADIUS
		? TAB_ICON_INCREASED_DISCARD_RADIUS
		: 0;
}

/** Ports CrashAnimation::AnimateToState(). */
export function getTabIconCrashFrame(progress: number): TabIconCrashFrame {
	const state = clamp01(progress);
	if (state < 0.5) {
		return { showingCrashedFavicon: false, hidingFraction: state * 2 };
	}
	return {
		showingCrashedFavicon: true,
		hidingFraction: 1 - (state - 0.5) * 2,
	};
}

/**
 * Ports the circular favicon clip calculation used while loading/discarded.
 * Chromium applies EASE_IN before interpolating from the inset diameter to a
 * full-square circumscribed circle.
 */
export function getTabIconClipDiameter(
	animationValue: number,
	showingDiscardIndicator: boolean,
	increasedDiscardIndicatorRadius: number,
): number {
	const initialInset = 1 + TAB_ICON_LOADING_STROKE_WIDTH;
	const initialDiameter =
		TAB_ICON_SIZE -
		2 * initialInset +
		(showingDiscardIndicator ? 2 * increasedDiscardIndicatorRadius : 0);
	const finalDiameter = Math.SQRT2 * TAB_ICON_SIZE;
	const eased = clamp01(animationValue) ** 2;
	return initialDiameter + (finalDiameter - initialDiameter) * eased;
}

/** Browser-independent rendering decisions from TabIcon::OnPaint(). */
export function getTabIconVisualState(
	tab: Readonly<TabData>,
	availableWidth = TAB_ICON_SIZE + 2 * TAB_ICON_INCREASED_DISCARD_RADIUS,
	discardRingTreatmentEnabled = true,
): TabIconVisualState {
	const networkState = resolveTabIconNetworkState(tab);
	const showingLoadingAnimation =
		!tab.shouldHideThrobber && tabIconNetworkStateIsAnimated(networkState);
	const showingCrashedFavicon = Boolean(tab.crashed);
	const hasCustomFavicon = Boolean(tab.faviconUrl);
	const showingDiscardIndicator = Boolean(tab.discarded) && discardRingTreatmentEnabled;
	const attention = Boolean(tab.blocked || tab.needsAttention);

	return {
		networkState,
		showingLoadingAnimation,
		showingFavicon:
			showingCrashedFavicon ||
			!showingLoadingAnimation ||
			(networkState !== 'waiting' && hasCustomFavicon),
		showingCrashedFavicon,
		showingAttentionIndicator: attention && !showingLoadingAnimation && !showingCrashedFavicon,
		showingDiscardIndicator,
		hasCustomFavicon,
		increasedDiscardIndicatorRadius: getIncreasedDiscardIndicatorRadius(availableWidth),
	};
}
