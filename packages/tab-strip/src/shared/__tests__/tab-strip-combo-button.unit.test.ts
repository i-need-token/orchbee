import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { TabStripOrientation } from '../../common';
import {
	getTabStripComboAnimationFactors,
	TAB_SEARCH_EPHEMERAL_HIDE_DELAY_MS,
	TAB_STRIP_COMBO_ANIMATION_DURATION_MS,
	TabStripComboButtonController,
	TabStripComboContext,
	TabStripComboStartKind,
} from '../tab-strip-combo-button';

describe('TabStripComboButtonController', () => {
	beforeEach(() => vi.useFakeTimers());
	afterEach(() => vi.useRealTimers());

	it('maps Chromium expansion, opacity, and radius stages', () => {
		expect(getTabStripComboAnimationFactors(0)).toEqual({ expansion: 0, opacity: 0, radius: 0 });
		expect(getTabStripComboAnimationFactors(0.5)).toEqual({
			expansion: 1,
			opacity: 0.5,
			radius: 0,
		});
		expect(getTabStripComboAnimationFactors(0.75)).toEqual({
			expansion: 1,
			opacity: 1,
			radius: 0.5,
		});
		expect(getTabStripComboAnimationFactors(1)).toEqual({ expansion: 1, opacity: 1, radius: 1 });
	});

	it('keeps hiding buttons alive until the 300ms animation ends', () => {
		const controller = new TabStripComboButtonController({
			startPinned: true,
			tabSearchPinned: true,
		});
		controller.setPinned('start', false);
		expect(controller.getSnapshot()).toMatchObject({
			startVisible: true,
			startTargetVisible: false,
			startAnimating: true,
		});
		vi.advanceTimersByTime(TAB_STRIP_COMBO_ANIMATION_DURATION_MS);
		expect(controller.getSnapshot()).toMatchObject({ startVisible: false, startAnimating: false });
	});

	it('shows unpinned Tab Search ephemerally and hides it two seconds after bubble close', () => {
		const controller = new TabStripComboButtonController({ richAnimations: false });
		controller.onBubbleInitializing();
		expect(controller.getSnapshot()).toMatchObject({
			tabSearchVisible: true,
			tabSearchEphemeral: true,
			keepCurrentStateLock: true,
		});
		controller.onBubbleDestroying();
		expect(controller.getSnapshot().hideTabSearchPending).toBe(true);
		vi.advanceTimersByTime(TAB_SEARCH_EPHEMERAL_HIDE_DELAY_MS);
		expect(controller.getSnapshot()).toMatchObject({
			tabSearchVisible: false,
			tabSearchEphemeral: false,
			keepCurrentStateLock: false,
		});
	});

	it('pins an ephemeral search button and records exact action names', () => {
		const actions: string[] = [];
		const controller = new TabStripComboButtonController({
			richAnimations: false,
			onUserAction: (action) => actions.push(action),
		});
		controller.onBubbleInitializing();
		controller.executeTogglePinned('tab-search');
		controller.onBubbleDestroying();
		vi.runAllTimers();
		expect(controller.getSnapshot()).toMatchObject({
			tabSearchPinned: true,
			tabSearchVisible: true,
			tabSearchEphemeral: false,
		});
		expect(actions).toEqual(['TabStripComboButton.TabSearch.Pinned']);
	});

	it('omits the start action horizontally and tracks Projects/Everything pin metrics', () => {
		const horizontal = new TabStripComboButtonController({
			context: TabStripComboContext.HorizontalTabStrip,
			startPinned: true,
			tabSearchPinned: true,
			richAnimations: false,
		});
		expect(horizontal.getSnapshot()).toMatchObject({
			startKind: null,
			startVisible: false,
			tabSearchVisible: true,
		});

		const actions: string[] = [];
		const everything = new TabStripComboButtonController({
			startKind: TabStripComboStartKind.EverythingMenu,
			startPinned: true,
			richAnimations: false,
			onUserAction: (action) => actions.push(action),
		});
		everything.setOrientation(TabStripOrientation.Vertical);
		everything.executeTogglePinned('start');
		expect(actions).toEqual(['TabStripComboButton.EverythingMenu.Unpinned']);
		expect(everything.getSnapshot()).toMatchObject({
			orientation: 'vertical',
			startVisible: false,
		});
	});

	it('holds expand-on-hover during bubbles and context menus and gates IPH', () => {
		const controller = new TabStripComboButtonController({ tabSearchPinned: true });
		expect(controller.isIphEligible(false)).toBe(false);
		expect(controller.isIphEligible(true)).toBe(true);
		controller.onMenuOpened('tab-search');
		expect(controller.getSnapshot()).toMatchObject({
			menuOpenFor: 'tab-search',
			keepCurrentStateLock: true,
		});
		controller.onMenuClosed();
		expect(controller.getSnapshot().keepCurrentStateLock).toBe(false);
	});

	it('restarts the ephemeral hide grace period when its context menu closes', () => {
		const controller = new TabStripComboButtonController({ richAnimations: false });
		controller.onBubbleInitializing();
		controller.onBubbleDestroying();
		vi.advanceTimersByTime(1_500);
		controller.onMenuOpened('tab-search');
		vi.advanceTimersByTime(500);
		expect(controller.getSnapshot().tabSearchVisible).toBe(true);

		controller.onMenuClosed();
		vi.advanceTimersByTime(TAB_SEARCH_EPHEMERAL_HIDE_DELAY_MS - 1);
		expect(controller.getSnapshot().tabSearchVisible).toBe(true);
		vi.advanceTimersByTime(1);
		expect(controller.getSnapshot().tabSearchVisible).toBe(false);
	});
});
