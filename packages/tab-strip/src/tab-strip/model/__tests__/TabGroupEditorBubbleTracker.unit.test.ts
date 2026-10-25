import { describe, expect, it, vi } from 'vitest';
import { TabGroupEditorBubbleTracker } from '../TabGroupEditorBubbleTracker';

describe('TabGroupEditorBubbleTracker', () => {
	it('ignores opening-time auto-scroll and closes at the 250ms boundary', () => {
		const tracker = new TabGroupEditorBubbleTracker();
		const reasons: string[] = [];
		tracker.registerOnClosed((reason) => reasons.push(reason));
		tracker.open(1_000);

		expect(tracker.onContentsScrolled(1_000)).toBe(false);
		expect(tracker.onContentsScrolled(1_000 + 250 - 1)).toBe(false);
		expect(tracker.isOpen).toBe(true);
		expect(tracker.onContentsScrolled(1_000 + 250)).toBe(true);
		expect(tracker.isOpen).toBe(false);
		expect(reasons).toEqual(['scroll']);
	});

	it('notifies opens and closes synchronously and closes for a mode change', () => {
		const tracker = new TabGroupEditorBubbleTracker();
		const opened = vi.fn<() => void>();
		const closed = vi.fn<() => void>();
		tracker.registerOnOpened(opened);
		tracker.registerOnClosed(closed);

		tracker.open();
		expect(opened).toHaveBeenCalledOnce();
		expect(tracker.onVerticalTabStripModeWillChange()).toBe(true);
		expect(closed).toHaveBeenCalledWith('vertical-tab-mode-change');
		expect(tracker.onVerticalTabStripModeWillChange()).toBe(false);
	});
});
