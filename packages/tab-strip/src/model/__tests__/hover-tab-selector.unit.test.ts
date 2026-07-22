import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { TabModel } from '../tab-model';
import { HOVER_TAB_TRANSITION_DELAY_MS, HoverTabSelector } from '../hover-tab-selector';

function model(): TabModel {
	return new TabModel({
		tabs: [
			{ id: 'a', title: 'A', pinned: false, group: null, split: null },
			{ id: 'b', title: 'B', pinned: false, group: null, split: null },
			{ id: 'c', title: 'C', pinned: false, group: null, split: null },
		],
		activeTabId: 'a',
	});
}

describe('HoverTabSelector', () => {
	beforeEach(() => vi.useFakeTimers());
	afterEach(() => vi.useRealTimers());

	it("activates a hovered tab after Chromium's 500ms delay", () => {
		const tabs = model();
		const selector = new HoverTabSelector(tabs);
		selector.startTabTransition(1);
		vi.advanceTimersByTime(HOVER_TAB_TRANSITION_DELAY_MS - 1);
		expect(tabs.getSnapshot().activeTabId).toBe('a');
		vi.advanceTimersByTime(1);
		expect(tabs.getSnapshot().activeTabId).toBe('b');
	});

	it('keeps the same timer, retargets another tab, and supports cancellation', () => {
		const tabs = model();
		const selector = new HoverTabSelector(tabs);
		selector.startTabTransition(1);
		vi.advanceTimersByTime(300);
		selector.startTabTransition(1);
		vi.advanceTimersByTime(200);
		expect(tabs.getSnapshot().activeTabId).toBe('b');

		selector.startTabTransition(2);
		vi.advanceTimersByTime(300);
		selector.startTabTransition(0);
		vi.advanceTimersByTime(300);
		expect(tabs.getSnapshot().activeTabId).toBe('b');
		selector.cancelTabTransition();
		vi.runAllTimers();
		expect(tabs.getSnapshot().activeTabId).toBe('b');
	});
});
