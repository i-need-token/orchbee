import { describe, expect, it } from 'vitest';

import { takesCreatedPanelFocus } from '../takesCreatedPanelFocus';

const ALLOWED = {
	focusNewPanel: true,
	handoverHeld: true,
	sameActiveTab: true,
	sameLocation: true,
	signalAborted: false,
};

describe('takesCreatedPanelFocus', () => {
	it('leaves the focus where it was when the split preference is off', () => {
		expect(takesCreatedPanelFocus({ ...ALLOWED, focusNewPanel: false })).toBe(false);
	});

	it('gives the focus to the new Panel only while nothing has moved on', () => {
		expect(takesCreatedPanelFocus(ALLOWED)).toBe(true);
		expect(takesCreatedPanelFocus({ ...ALLOWED, handoverHeld: false })).toBe(false);
		expect(takesCreatedPanelFocus({ ...ALLOWED, signalAborted: true })).toBe(false);
	});
});
