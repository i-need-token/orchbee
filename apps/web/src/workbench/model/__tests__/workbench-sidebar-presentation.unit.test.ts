import { describe, expect, it } from 'vitest';

import {
	resolveWorkbenchSidebarPresentation,
	transitionWorkbenchSidebarPresentation,
	type WorkbenchSidebarPresentationEvent,
	type WorkbenchSidebarPresentationState,
} from '../workbench-sidebar-presentation';

function run(
	state: WorkbenchSidebarPresentationState,
	...events: readonly WorkbenchSidebarPresentationEvent[]
): WorkbenchSidebarPresentationState {
	return events.reduce(transitionWorkbenchSidebarPresentation, state);
}

const WIDE_DOCKED: WorkbenchSidebarPresentationState = {
	docked: true,
	narrow: false,
	overlayOpen: false,
};

describe('workbench sidebar presentation', () => {
	it('docks or hides as chosen while the window is wide', () => {
		expect(resolveWorkbenchSidebarPresentation(WIDE_DOCKED)).toBe('docked');
		const hidden = run(WIDE_DOCKED, { type: 'toggled' });
		expect(hidden).toEqual({ ...WIDE_DOCKED, docked: false });
		expect(resolveWorkbenchSidebarPresentation(hidden)).toBe('hidden');
	});

	it('steps aside in a narrow window, and the toggle opens it over the content', () => {
		const narrow = run(WIDE_DOCKED, { narrow: true, type: 'window-resized' });
		expect(resolveWorkbenchSidebarPresentation(narrow)).toBe('hidden');
		const opened = run(narrow, { type: 'toggled' });
		expect(resolveWorkbenchSidebarPresentation(opened)).toBe('overlay');
		// The docked choice is untouched while narrow.
		expect(opened.docked).toBe(true);
		expect(resolveWorkbenchSidebarPresentation(run(opened, { type: 'toggled' }))).toBe('hidden');
	});

	it('closes the overlay when dismissed, and dismissing a closed overlay changes nothing', () => {
		const opened = run(WIDE_DOCKED, { narrow: true, type: 'window-resized' }, { type: 'toggled' });
		const dismissed = run(opened, { type: 'overlay-dismissed' });
		expect(resolveWorkbenchSidebarPresentation(dismissed)).toBe('hidden');
		expect(run(dismissed, { type: 'overlay-dismissed' })).toBe(dismissed);
		expect(run(WIDE_DOCKED, { type: 'overlay-dismissed' })).toBe(WIDE_DOCKED);
	});

	it('docks again as it was left once the window widens, closing any overlay', () => {
		const opened = run(WIDE_DOCKED, { narrow: true, type: 'window-resized' }, { type: 'toggled' });
		expect(run(opened, { narrow: false, type: 'window-resized' })).toEqual(WIDE_DOCKED);
		const hiddenWide = { ...WIDE_DOCKED, docked: false };
		const narrowed = run(hiddenWide, { narrow: true, type: 'window-resized' }, { type: 'toggled' });
		expect(
			resolveWorkbenchSidebarPresentation(run(narrowed, { narrow: false, type: 'window-resized' })),
		).toBe('hidden');
	});
});
