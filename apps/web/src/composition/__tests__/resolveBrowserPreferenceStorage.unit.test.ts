import { describe, expect, it, vi } from 'vitest';

import { resolveBrowserPreferenceStorage } from '../resolveBrowserPreferenceStorage';

describe('resolveBrowserPreferenceStorage', () => {
	it('returns the browser-owned storage when it is available', () => {
		const storage = {
			getItem: vi.fn<(key: string) => string | null>(() => null),
			setItem: vi.fn<(key: string, value: string) => void>(),
		};

		expect(resolveBrowserPreferenceStorage(() => storage)).toBe(storage);
	});

	it('falls back to ephemeral storage when the browser getter throws', () => {
		const storage = resolveBrowserPreferenceStorage(() => {
			throw new Error('Browser storage is unavailable.');
		});

		expect(storage.getItem('orchbee:test:preference:v1')).toBeNull();
		storage.setItem('orchbee:test:preference:v1', '{"enabled":true}');
		expect(storage.getItem('orchbee:test:preference:v1')).toBe('{"enabled":true}');
	});
});
