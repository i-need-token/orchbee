import { describe, expect, it, vi } from 'vitest';

import {
	readWorkbenchLeftPanelWidth,
	writeWorkbenchLeftPanelWidth,
} from '../workbench-layout-storage';

const DEFAULT_LEFT_PANEL_WIDTH = 250;

describe('workbench layout storage', () => {
	it('restores only a valid versioned pixel width', () => {
		for (const [storedValue, expectedWidth] of [
			[null, DEFAULT_LEFT_PANEL_WIDTH],
			['not-json', DEFAULT_LEFT_PANEL_WIDTH],
			['318', DEFAULT_LEFT_PANEL_WIDTH],
			['null', DEFAULT_LEFT_PANEL_WIDTH],
			['{"width":199}', DEFAULT_LEFT_PANEL_WIDTH],
			['{"width":401}', DEFAULT_LEFT_PANEL_WIDTH],
			['{"width":250,"extra":true}', DEFAULT_LEFT_PANEL_WIDTH],
			['{"width":318}', 318],
		] as const) {
			expect(
				readWorkbenchLeftPanelWidth({
					getItem: () => storedValue,
					setItem: () => undefined,
				}),
			).toBe(expectedWidth);
		}
	});

	it('writes a rounded width under the canonical key', () => {
		const setItem = vi.fn<(key: string, value: string) => void>();

		writeWorkbenchLeftPanelWidth({ getItem: () => null, setItem }, 318.4);

		expect(setItem).toHaveBeenCalledWith('orchbee:workbench:left-panel-width:v1', '{"width":318}');
	});

	it('ignores unavailable storage and out-of-range writes', () => {
		const unavailableStorage = {
			getItem: () => {
				throw new Error('Storage is unavailable');
			},
			setItem: () => {
				throw new Error('Storage is unavailable');
			},
		};

		expect(readWorkbenchLeftPanelWidth(unavailableStorage)).toBe(DEFAULT_LEFT_PANEL_WIDTH);
		expect(() => writeWorkbenchLeftPanelWidth(unavailableStorage, 320)).not.toThrow();
		expect(() => writeWorkbenchLeftPanelWidth(unavailableStorage, 500)).not.toThrow();
	});
});
