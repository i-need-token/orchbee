import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { TabStripOrientation } from '../../types/tab-strip-types';
import { TabStripComboButtonController } from '../TabStripComboButtonController';

describe('TabStripComboButtonController', () => {
	beforeEach(() => vi.useFakeTimers());
	afterEach(() => vi.useRealTimers());

	it('keeps hiding buttons alive until the configured animation ends', () => {
		const controller = new TabStripComboButtonController({
			tabSearchPinned: true,
		});
		controller.setPinned('tab-search', false);
		expect(controller.getSnapshot()).toMatchObject({
			tabSearchVisible: true,
			tabSearchTargetVisible: false,
			tabSearchAnimating: true,
		});
		vi.advanceTimersByTime(300);
		expect(controller.getSnapshot()).toMatchObject({
			tabSearchVisible: false,
			tabSearchAnimating: false,
		});
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
		vi.advanceTimersByTime(2_000);
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

	it('tracks orientation independently of Tab Search visibility', () => {
		const controller = new TabStripComboButtonController({ richAnimations: false });
		controller.setOrientation(TabStripOrientation.Vertical);
		expect(controller.getSnapshot()).toMatchObject({
			orientation: 'vertical',
			tabSearchVisible: false,
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
		vi.advanceTimersByTime(2_000 - 1);
		expect(controller.getSnapshot().tabSearchVisible).toBe(true);
		vi.advanceTimersByTime(1);
		expect(controller.getSnapshot().tabSearchVisible).toBe(false);
	});
});
