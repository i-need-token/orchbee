import { describe, expect, it } from 'vitest';
import {
	TAB_STRIP_NUDGE_CLOSE_BUTTON_SIZE,
	TAB_STRIP_NUDGE_CORNER_RADIUS,
	TAB_STRIP_NUDGE_FLAT_CORNER_RADIUS,
	calculateTabStripNudgeButtonLayout,
} from '../tab-strip-nudge-button';

describe('TabStripNudgeButton layout', () => {
	it('matches Chromium radii and close-button size', () => {
		expect(TAB_STRIP_NUDGE_CORNER_RADIUS).toBe(10);
		expect(TAB_STRIP_NUDGE_FLAT_CORNER_RADIUS).toBe(4);
		expect(TAB_STRIP_NUDGE_CLOSE_BUTTON_SIZE).toBe(16);
	});

	it('uses a 4/12 label inset when the close button is hidden', () => {
		expect(
			calculateTabStripNudgeButtonLayout({
				iconWidth: 16,
				labelWidth: 60,
				showCloseButton: false,
				widthFactor: 1,
			}),
		).toEqual({
			fullWidth: 98,
			width: 98,
			height: 28,
			labelInsets: { left: 4, right: 12 },
			closeButtonContribution: 0,
		});
	});

	it('reserves 8dp on either side of the 16dp close button and truncates the animated width', () => {
		expect(
			calculateTabStripNudgeButtonLayout({
				iconWidth: 16,
				labelWidth: 61,
				showCloseButton: true,
				widthFactor: 0.5,
			}),
		).toEqual({
			fullWidth: 119,
			width: 59,
			height: 28,
			labelInsets: { left: 4, right: 0 },
			closeButtonContribution: 32,
		});
	});
});
