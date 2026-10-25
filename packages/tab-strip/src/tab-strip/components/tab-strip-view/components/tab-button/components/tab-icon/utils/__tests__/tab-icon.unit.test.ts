import { describe, expect, it } from 'vitest';
import {
	TAB_ICON_CRASH_ANIMATION_MS,
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
		expect(TAB_ICON_CRASH_ANIMATION_MS).toBe(1_000);
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
		expect(getTabIconVisualState(tab, 19).increasedDiscardIndicatorRadius).toBe(0);
		expect(getTabIconVisualState(tab, 20).increasedDiscardIndicatorRadius).toBe(2);
	});
});
