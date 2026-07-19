import { describe, expect, it, vi } from 'vitest';

import {
	readWorkbenchSidebarVisibility,
	writeWorkbenchSidebarVisibility,
} from '../workbench-sidebar-visibility-storage';

describe('Workbench sidebar visibility storage', () => {
	it('restores only a valid versioned visibility record', () => {
		for (const [storedValue, expectedVisibility] of [
			[null, true],
			['not-json', true],
			['{"visible":"false"}', true],
			['{"visible":false,"extra":true}', true],
			['{"visible":false}', false],
		] as const) {
			expect(
				readWorkbenchSidebarVisibility({
					getItem: () => storedValue,
					setItem: () => undefined,
				}),
			).toBe(expectedVisibility);
		}
	});

	it('writes the visibility under the canonical key', () => {
		const setItem = vi.fn<(key: string, value: string) => void>();

		writeWorkbenchSidebarVisibility({ getItem: () => null, setItem }, false);

		expect(setItem).toHaveBeenCalledWith(
			'orchbee:workbench:left-panel-visibility:v1',
			'{"visible":false}',
		);
	});

	it('treats unavailable storage as a recoverable boundary failure', () => {
		const unavailableStorage = {
			getItem: () => {
				throw new Error('Storage is unavailable');
			},
			setItem: () => {
				throw new Error('Storage is unavailable');
			},
		};

		expect(readWorkbenchSidebarVisibility(unavailableStorage)).toBe(true);
		expect(() => writeWorkbenchSidebarVisibility(unavailableStorage, false)).not.toThrow();
	});
});
