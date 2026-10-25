import type { TabData } from '../../../../../../../model/tab-model';

const TAB_ICON_SIZE = 16;

export const TAB_ICON_CRASH_ANIMATION_MS = 1_000;

const TAB_ICON_INCREASED_DISCARD_RADIUS = 2;

type TabIconNetworkState = NonNullable<TabData['networkState']>;

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
function getIncreasedDiscardIndicatorRadius(width: number): number {
	return width >= TAB_ICON_SIZE + 2 * TAB_ICON_INCREASED_DISCARD_RADIUS
		? TAB_ICON_INCREASED_DISCARD_RADIUS
		: 0;
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
