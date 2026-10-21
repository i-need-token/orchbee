import { describe, expect, it } from 'vitest';
import {
	getTabCloseButtonMetrics,
	isPointInTabCloseTarget,
	shouldTabCloseButtonHandlePointer,
} from '../tab-close-button';

describe('TabCloseButton source port', () => {
	it("uses Chromium's 36dp button around the 24dp icon in touch UI", () => {
		expect(getTabCloseButtonMetrics(true)).toEqual({
			iconSize: 24,
			buttonSize: 36,
			nonVisiblePadding: 6,
		});
		expect(isPointInTabCloseTarget({ x: 29, y: 18, touchUi: true })).toBe(true);
		expect(isPointInTabCloseTarget({ x: 30, y: 18, touchUi: true })).toBe(false);
		expect(isPointInTabCloseTarget({ x: 35, y: 18, touchUi: true, touchDown: true })).toBe(true);
	});

	it('ignores mouse padding but includes it for touch targeting', () => {
		expect(isPointInTabCloseTarget({ x: 5, y: 14 })).toBe(false);
		expect(isPointInTabCloseTarget({ x: 6, y: 6 })).toBe(true);
		expect(isPointInTabCloseTarget({ x: 5, y: 14, touchDown: true })).toBe(true);
	});

	it('forwards middle clicks to the parent tab', () => {
		expect(shouldTabCloseButtonHandlePointer({ button: 1, x: 14, y: 14 })).toBe(false);
		expect(shouldTabCloseButtonHandlePointer({ button: 0, x: 14, y: 14 })).toBe(true);
	});
});
