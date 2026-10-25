import { describe, expect, it } from 'vitest';
import { shouldTabCloseButtonHandlePointer } from '../shouldTabCloseButtonHandlePointer';

describe('TabCloseButton source port', () => {
	it("uses Chromium's 36dp button around the 24dp icon in touch UI", () => {
		expect(shouldTabCloseButtonHandlePointer({ button: 0, x: 29, y: 18, touchUi: true })).toBe(
			true,
		);
		expect(shouldTabCloseButtonHandlePointer({ button: 0, x: 30, y: 18, touchUi: true })).toBe(
			false,
		);
		expect(
			shouldTabCloseButtonHandlePointer({
				button: 0,
				x: 35,
				y: 18,
				touchUi: true,
				pointerType: 'touch',
			}),
		).toBe(true);
	});

	it('ignores mouse padding but includes it for touch targeting', () => {
		expect(shouldTabCloseButtonHandlePointer({ button: 0, x: 5, y: 14 })).toBe(false);
		expect(shouldTabCloseButtonHandlePointer({ button: 0, x: 6, y: 6 })).toBe(true);
		expect(
			shouldTabCloseButtonHandlePointer({ button: 0, x: 5, y: 14, pointerType: 'touch' }),
		).toBe(true);
	});

	it('forwards middle clicks to the parent tab', () => {
		expect(shouldTabCloseButtonHandlePointer({ button: 1, x: 14, y: 14 })).toBe(false);
		expect(shouldTabCloseButtonHandlePointer({ button: 0, x: 14, y: 14 })).toBe(true);
	});
});
