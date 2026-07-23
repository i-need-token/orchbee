import { describe, expect, it } from 'vitest';
import {
	TAB_ICON_CRASH_ANIMATION_MS,
	TAB_ICON_CRASH_FRAMES_PER_SECOND,
	TAB_ICON_DISCARD_FADE_MS,
	TAB_ICON_FAVICON_TRANSITION_MS,
	TAB_ICON_THROBBER_ROTATION_MS,
	getIncreasedDiscardIndicatorRadius,
	getTabIconClipDiameter,
	getTabIconCrashFrame,
	getTabIconVisualState,
	tabIconNetworkStateIsAnimated,
} from '../tab-icon';

const tab = {
	id: 'a',
	title: 'A',
	pinned: false,
	group: null,
} as const;

describe('TabIcon source port', () => {
	it('matches Chromium animation constants', () => {
		expect(TAB_ICON_FAVICON_TRANSITION_MS).toBe(250);
		expect(TAB_ICON_DISCARD_FADE_MS).toBe(1_000);
		expect(TAB_ICON_CRASH_ANIMATION_MS).toBe(1_000);
		expect(TAB_ICON_CRASH_FRAMES_PER_SECOND).toBe(25);
		expect(TAB_ICON_THROBBER_ROTATION_MS).toBe(1_568);
	});

	it('animates waiting and loading but not none or error', () => {
		expect(tabIconNetworkStateIsAnimated('waiting')).toBe(true);
		expect(tabIconNetworkStateIsAnimated('loading')).toBe(true);
		expect(tabIconNetworkStateIsAnimated('none')).toBe(false);
		expect(tabIconNetworkStateIsAnimated('error')).toBe(false);
	});

	it('hides every favicon while waiting and only keeps custom favicons while loading', () => {
		expect(
			getTabIconVisualState({ ...tab, networkState: 'waiting', faviconUrl: '/custom.png' })
				.showingFavicon,
		).toBe(false);
		expect(getTabIconVisualState({ ...tab, networkState: 'loading' }).showingFavicon).toBe(false);
		expect(
			getTabIconVisualState({ ...tab, networkState: 'loading', faviconUrl: '/custom.png' })
				.showingFavicon,
		).toBe(true);
		expect(
			getTabIconVisualState({
				...tab,
				networkState: 'loading',
				faviconUrl: '/custom.png',
				shouldHideThrobber: true,
			}).showingLoadingAnimation,
		).toBe(false);
	});

	it('gives loading/crash precedence over attention and retains the discard ring preference', () => {
		expect(getTabIconVisualState({ ...tab, needsAttention: true }).showingAttentionIndicator).toBe(
			true,
		);
		expect(
			getTabIconVisualState({ ...tab, needsAttention: true, loading: true })
				.showingAttentionIndicator,
		).toBe(false);
		expect(
			getTabIconVisualState({ ...tab, needsAttention: true, crashed: true })
				.showingAttentionIndicator,
		).toBe(false);
		expect(getTabIconVisualState({ ...tab, discarded: true }).showingDiscardIndicator).toBe(true);
		expect(
			getTabIconVisualState({ ...tab, discarded: true }, 20, false).showingDiscardIndicator,
		).toBe(false);
	});

	it('increases the discard ring only when its 20dp diameter fits', () => {
		expect(getIncreasedDiscardIndicatorRadius(19)).toBe(0);
		expect(getIncreasedDiscardIndicatorRadius(20)).toBe(2);
		expect(getTabIconClipDiameter(0, true, 2)).toBe(14);
		expect(getTabIconClipDiameter(0, false, 0)).toBe(10);
		expect(getTabIconClipDiameter(1, false, 0)).toBeCloseTo(Math.SQRT2 * 16);
	});

	it('moves the normal icon down, then brings the crash icon up', () => {
		expect(getTabIconCrashFrame(0)).toEqual({ showingCrashedFavicon: false, hidingFraction: 0 });
		expect(getTabIconCrashFrame(0.25)).toEqual({
			showingCrashedFavicon: false,
			hidingFraction: 0.5,
		});
		expect(getTabIconCrashFrame(0.5)).toEqual({ showingCrashedFavicon: true, hidingFraction: 1 });
		expect(getTabIconCrashFrame(1)).toEqual({ showingCrashedFavicon: true, hidingFraction: 0 });
	});
});
